//! ESPHome controller monitoring module.
//!
//! This module provides a separate thread for monitoring the ESPHome remote controller
//! and communicates with the web server using oneshot channels for request/response patterns.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::{Mutex, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};
use tokio::time::{interval, sleep};
use tracing::{debug, error, info, warn};

use crate::OurResult;
use crate::config::Settings;

/// Controller status information
#[derive(Debug, Clone)]
pub struct ControllerStatus {
    pub online: bool,
    pub hostname: String,
    pub last_seen: Option<Instant>,
    pub response_time_ms: Option<u64>,
    pub error_count: u32,
    pub uptime_seconds: Option<u64>,
}

/// Sensor readings from the controller
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorReadings {
    pub case_ready: bool,
    pub case_in_view: bool,
    pub timestamp: u64,
}

/// Machine status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineStatus {
    pub status: String,
    pub ready: bool,
    pub active_jobs: u32,
    pub last_update: chrono::DateTime<chrono::Utc>,
}

/// Commands that can be sent to the controller
#[derive(Debug, Clone)]
pub enum ControllerCommand {
    NextCase,
    GetStatus,
    GetSensors,
    GetHardwareStatus,
    TriggerVibration,
    SetServoPosition { servo: String, position: u8 },
    UpdateConfig { new_settings: Settings },
}

/// Responses from controller operations
#[derive(Debug, Clone)]
pub enum ControllerResponse {
    Success(String),
    SensorData(SensorReadings),
    StatusData(MachineStatus),
    HardwareData(HashMap<String, String>),
    Error(String),
    ConfigUpdated,
}

/// Request structure for communication with the controller monitor
#[derive(Debug)]
pub struct ControllerRequest {
    pub command: ControllerCommand,
    pub response_sender: oneshot::Sender<ControllerResponse>,
}

/// Controller monitor that runs in a separate thread
pub struct ControllerMonitor {
    settings: Arc<RwLock<Settings>>,
    status: Arc<Mutex<ControllerStatus>>,
    request_receiver: mpsc::UnboundedReceiver<ControllerRequest>,
    client: reqwest::Client,
}

/// Handle for communicating with the controller monitor
#[derive(Clone)]
pub struct ControllerHandle {
    request_sender: mpsc::UnboundedSender<ControllerRequest>,
    status: Arc<Mutex<ControllerStatus>>,
}

impl ControllerHandle {
    /// Update the controller configuration
    pub async fn update_config(&self, new_settings: Settings) -> Result<(), String> {
        let (response_sender, response_receiver) = oneshot::channel();

        let request = ControllerRequest {
            command: ControllerCommand::UpdateConfig { new_settings },
            response_sender,
        };

        if self.request_sender.send(request).is_err() {
            return Err("Controller monitor is not running".to_string());
        }

        match response_receiver.await {
            Ok(ControllerResponse::ConfigUpdated) => Ok(()),
            Ok(ControllerResponse::Error(e)) => Err(e),
            Ok(_) => Err("Unexpected response from controller monitor".to_string()),
            Err(_) => Err("Failed to receive response from controller monitor".to_string()),
        }
    }

    /// Get current controller status
    pub fn get_status(&self) -> ControllerStatus {
        self.status.lock().expect("Status lock poisoned").clone()
    }
}

impl ControllerMonitor {
    /// Create a new controller monitor and return a handle for communication
    pub fn new(settings: Settings) -> Result<(Self, ControllerHandle), Box<dyn std::error::Error>> {
        let (request_sender, request_receiver) = mpsc::unbounded_channel();

        let settings = Arc::new(RwLock::new(settings.clone()));
        let status = Arc::new(Mutex::new(ControllerStatus {
            online: false,
            hostname: settings
                .read()
                .expect("Settings lock poisoned")
                .esphome_hostname
                .clone(),
            last_seen: None,
            response_time_ms: None,
            error_count: 0,
            uptime_seconds: None,
        }));

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()?;

        let monitor = Self {
            settings,
            status: status.clone(),
            request_receiver,
            client,
        };

        let handle = ControllerHandle {
            request_sender,
            status,
        };

        Ok((monitor, handle))
    }

    /// Start the controller monitoring loop
    pub async fn run(mut self) -> OurResult<()> {
        let hostname = {
            let settings = self.settings.read().expect("Settings lock poisoned");
            settings.esphome_hostname.clone()
        };

        info!("Starting ESPHome controller monitor for {}", hostname);

        // Start periodic health check
        let health_check_status = self.status.clone();
        let health_check_client = self.client.clone();
        let health_check_settings = self.settings.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                let hostname = {
                    let settings = health_check_settings
                        .read()
                        .expect("Settings lock poisoned");
                    settings.esphome_hostname.clone()
                };
                Self::perform_health_check(&health_check_client, &hostname, &health_check_status)
                    .await;
            }
        });

        // Main request processing loop
        loop {
            tokio::select! {
                Some(request) = self.request_receiver.recv() => {
                    self.handle_request(request).await;
                }
                else => {
                    warn!("Controller monitor request channel closed, shutting down");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Handle a single request from the web server
    async fn handle_request(&self, request: ControllerRequest) {
        let response = match request.command {
            ControllerCommand::NextCase => self.trigger_next_case().await,
            ControllerCommand::GetStatus => self.get_machine_status().await,
            ControllerCommand::GetSensors => self.get_sensor_readings().await,
            ControllerCommand::GetHardwareStatus => self.get_hardware_status().await,
            ControllerCommand::TriggerVibration => self.trigger_vibration().await,
            ControllerCommand::SetServoPosition { servo, position } => {
                self.set_servo_position(&servo, position).await
            }
            ControllerCommand::UpdateConfig { new_settings } => {
                self.update_config(new_settings).await
            }
        };

        if request.response_sender.send(response).is_err() {
            warn!("Failed to send response back to web server (receiver dropped)");
        }
    }

    /// Update controller configuration
    async fn update_config(&self, new_settings: Settings) -> ControllerResponse {
        let old_hostname = {
            let settings = self.settings.read().expect("Settings lock poisoned");
            settings.esphome_hostname.clone()
        };

        // Update settings
        {
            let mut settings = self.settings.write().expect("Settings lock poisoned");
            *settings = new_settings;
        }

        let new_hostname = {
            let settings = self.settings.read().expect("Settings lock poisoned");
            settings.esphome_hostname.clone()
        };

        // Update status with new hostname if it changed
        if old_hostname != new_hostname {
            let mut status = self.status.lock().expect("Status lock poisoned");
            status.hostname = new_hostname.clone();
            status.online = false; // Reset online status until next health check
            status.last_seen = None;
            status.response_time_ms = None;
            status.error_count = 0;

            info!(
                "Controller hostname updated from '{}' to '{}'",
                old_hostname, new_hostname
            );
        }

        ControllerResponse::ConfigUpdated
    }

    /// Trigger the next case sequence on the controller
    async fn trigger_next_case(&self) -> ControllerResponse {
        let hostname = {
            let settings = self.settings.read().expect("Settings lock poisoned");
            settings.esphome_hostname.clone()
        };

        let url = format!("http://{}/button/trigger_next_case/press", hostname);

        match self.make_request(&url, "POST").await {
            Ok(_) => {
                info!("Successfully triggered next case sequence");
                ControllerResponse::Success("Next case sequence triggered".to_string())
            }
            Err(e) => {
                error!("Failed to trigger next case: {e}");
                ControllerResponse::Error(format!("Failed to trigger next case: {e}"))
            }
        }
    }

    /// Get machine status from the controller
    async fn get_machine_status(&self) -> ControllerResponse {
        let status = MachineStatus {
            status: if self.is_online().await {
                "Ready".to_string()
            } else {
                "Offline".to_string()
            },
            ready: self.is_online().await,
            active_jobs: 0,
            last_update: chrono::Utc::now(),
        };

        ControllerResponse::StatusData(status)
    }

    /// Get sensor readings from the controller
    async fn get_sensor_readings(&self) -> ControllerResponse {
        // Try to get sensor data from ESPHome API
        let case_ready = self
            .get_binary_sensor("case_ready_to_feed")
            .await
            .unwrap_or(false);
        let case_in_view = self
            .get_binary_sensor("case_in_camera_view")
            .await
            .unwrap_or(false);

        let readings = SensorReadings {
            case_ready,
            case_in_view,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        };

        ControllerResponse::SensorData(readings)
    }

    /// Get hardware status from the controller
    async fn get_hardware_status(&self) -> ControllerResponse {
        let mut status = HashMap::new();

        if self.is_online().await {
            status.insert("controller".to_string(), "Connected".to_string());
            status.insert("esphome_hostname".to_string(), {
                let settings = self.settings.read().expect("Settings lock poisoned");
                settings.esphome_hostname.clone()
            });

            // Try to get additional status info
            if let Ok(info) = self.get_device_info().await {
                status.extend(info);
            }
        } else {
            status.insert("controller".to_string(), "Disconnected".to_string());
            status.insert("esphome_hostname".to_string(), {
                let settings = self.settings.read().expect("Settings lock poisoned");
                settings.esphome_hostname.clone()
            });
        }

        ControllerResponse::HardwareData(status)
    }

    /// Trigger vibration motor
    async fn trigger_vibration(&self) -> ControllerResponse {
        let url = format!("http://{}/switch/vibration_motor/turn_on", {
            let settings = self.settings.read().expect("Settings lock poisoned");
            settings.esphome_hostname.clone()
        });

        match self.make_request(&url, "POST").await {
            Ok(_) => {
                // ESPHome will automatically turn off after configured time
                info!("Successfully triggered vibration motor");
                ControllerResponse::Success("Vibration motor triggered".to_string())
            }
            Err(e) => {
                error!("Failed to trigger vibration: {e}");
                ControllerResponse::Error(format!("Failed to trigger vibration: {e}"))
            }
        }
    }

    /// Set servo position
    async fn set_servo_position(&self, servo: &str, position: u8) -> ControllerResponse {
        let url = format!("http://{}/number/{servo}/set?value={position}", {
            let settings = self.settings.read().expect("Settings lock poisoned");
            settings.esphome_hostname.clone()
        });

        match self.make_request(&url, "POST").await {
            Ok(_) => {
                info!("Successfully set {servo} servo to position {position}");
                ControllerResponse::Success(format!("Servo {servo} set to position {position}"))
            }
            Err(e) => {
                error!("Failed to set servo position: {e}");
                ControllerResponse::Error(format!("Failed to set servo position: {e}"))
            }
        }
    }

    /// Check if the controller is online
    async fn is_online(&self) -> bool {
        let status = self.status.lock().expect("Status lock poisoned");
        status.online
    }

    /// Get binary sensor state from ESPHome
    async fn get_binary_sensor(
        &self,
        sensor_name: &str,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let url = format!(
            "http://{}/binary_sensor/{}/state",
            {
                let settings = self.settings.read().expect("Settings lock poisoned");
                settings.esphome_hostname.clone()
            },
            sensor_name
        );
        let response = self.make_request(&url, "GET").await?;

        // ESPHome returns "ON" or "OFF" for binary sensors
        Ok(response.trim().to_uppercase() == "ON")
    }

    /// Get device information from ESPHome
    async fn get_device_info(&self) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
        let url = format!("http://{}/text_sensor/device_info/state", {
            let settings = self.settings.read().expect("Settings lock poisoned");
            settings.esphome_hostname.clone()
        });

        let mut info = HashMap::new();
        match self.make_request(&url, "GET").await {
            Ok(response) => {
                info.insert("device_info".to_string(), response);
            }
            Err(_) => {
                // If specific endpoint doesn't exist, just return basic info
                info.insert("status".to_string(), "online".to_string());
            }
        }

        Ok(info)
    }

    /// Make HTTP request to the controller
    async fn make_request(
        &self,
        url: &str,
        method: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let start_time = Instant::now();

        let response = match method {
            "GET" => {
                self.client
                    .get(url)
                    .basic_auth("admin", Some("shellsorter"))
                    .send()
                    .await?
            }
            "POST" => {
                self.client
                    .post(url)
                    .basic_auth("admin", Some("shellsorter"))
                    .send()
                    .await?
            }
            _ => return Err("Unsupported HTTP method".into()),
        };

        let elapsed = start_time.elapsed();

        if response.status().is_success() {
            let text = response.text().await?;

            // Update response time in status
            {
                let mut status = self.status.lock().expect("Status lock poisoned");
                status.response_time_ms = Some(elapsed.as_millis() as u64);
                status.last_seen = Some(Instant::now());
            }

            Ok(text)
        } else {
            Err(format!("HTTP error: {}", response.status()).into())
        }
    }

    /// Perform periodic health check
    async fn perform_health_check(
        client: &reqwest::Client,
        hostname: &str,
        status: &Arc<Mutex<ControllerStatus>>,
    ) {
        let url = format!("http://{hostname}/");
        let start_time = Instant::now();

        debug!("Performing health check for {hostname}");

        let is_online = match client
            .get(&url)
            .basic_auth("admin", Some("shellsorter"))
            .send()
            .await
        {
            Ok(response) => {
                let elapsed = start_time.elapsed();
                let success = response.status().is_success();

                if success {
                    debug!(
                        "Health check successful, response time: {}ms",
                        elapsed.as_millis()
                    );
                } else {
                    warn!("Health check failed with status: {}", response.status());
                }

                // Update status
                {
                    let mut status_lock = status.lock().expect("Status lock poisoned");
                    status_lock.online = success;
                    status_lock.last_seen = Some(Instant::now());
                    status_lock.response_time_ms = Some(elapsed.as_millis() as u64);

                    if success {
                        status_lock.error_count = 0;
                    } else {
                        status_lock.error_count += 1;
                    }
                }

                success
            }
            Err(e) => {
                warn!("Health check failed: {e}");

                // Update status
                {
                    let mut status_lock = status.lock().expect("Status lock poisoned");
                    status_lock.online = false;
                    status_lock.error_count += 1;
                    status_lock.response_time_ms = None;
                }

                false
            }
        };

        if !is_online {
            // Wait a bit before next attempt to avoid spam
            sleep(Duration::from_secs(5)).await;
        }
    }
}

impl ControllerHandle {
    /// Send a command to the controller and wait for response
    pub async fn send_command(
        &self,
        command: ControllerCommand,
    ) -> Result<ControllerResponse, Box<dyn std::error::Error>> {
        let (response_sender, response_receiver) = oneshot::channel();

        let request = ControllerRequest {
            command,
            response_sender,
        };

        self.request_sender.send(request)?;

        match response_receiver.await {
            Ok(response) => Ok(response),
            Err(_) => Err("Controller monitor did not respond".into()),
        }
    }
}
