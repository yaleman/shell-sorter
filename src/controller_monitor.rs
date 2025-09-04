//! ESPHome controller monitoring module.
//!
//! This module provides a separate thread for monitoring the ESPHome remote controller
//! and communicates with the web server using oneshot channels for request/response patterns.

use reqwest::Method;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::sync::broadcast;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{interval, sleep};
use tracing::{debug, error, info, warn};

use crate::OurResult;
use crate::config::Settings;
use crate::protocol::GlobalMessage;

/// Controller status information
#[derive(Debug, Clone, Default)]
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
    UpdateConfig { new_settings: Box<Settings> },
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
    #[allow(dead_code)]
    global_tx: broadcast::Sender<GlobalMessage>,
    global_rx: broadcast::Receiver<GlobalMessage>,
    status: Arc<RwLock<ControllerStatus>>,
    client: reqwest::Client,
}

/// Handle for communicating with the controller monitor
#[derive(Clone)]
pub struct ControllerHandle {
    request_sender: mpsc::UnboundedSender<ControllerRequest>,
    status: Arc<RwLock<ControllerStatus>>,
}

impl ControllerHandle {
    /// Safely lock the status for reading
    async fn lock_status(&self) -> tokio::sync::RwLockReadGuard<ControllerStatus> {
        self.status.read().await
    }

    /// Update the controller configuration
    pub async fn update_config(&self, new_settings: Settings) -> Result<(), String> {
        let (response_sender, response_receiver) = oneshot::channel();

        let request = ControllerRequest {
            command: ControllerCommand::UpdateConfig {
                new_settings: Box::new(new_settings),
            },
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
    pub async fn get_status(&self) -> ControllerStatus {
        self.lock_status().await.clone()
    }
}

impl ControllerMonitor {
    /// Safely lock the status for reading
    async fn lock_status(&self) -> tokio::sync::RwLockReadGuard<ControllerStatus> {
        self.status.read().await
    }

    /// Safely lock the status for writing
    async fn lock_status_write(&self) -> tokio::sync::RwLockWriteGuard<ControllerStatus> {
        self.status.write().await
    }

    /// Create a new controller monitor and return a handle for communication
    pub fn new(
        settings: Settings,
        global_tx: broadcast::Sender<GlobalMessage>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let status = Arc::new(RwLock::new(ControllerStatus {
            online: false,
            hostname: settings.esphome_hostname.clone(),
            last_seen: None,
            response_time_ms: None,
            error_count: 0,
            uptime_seconds: None,
        }));

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()?;

        let monitor = Self {
            global_tx: global_tx.clone(),

            global_rx: global_tx.subscribe(),
            status: status.clone(),
            client,
        };

        Ok(monitor)
    }

    /// Start the controller monitoring loop
    pub async fn run(mut self) -> OurResult<()> {
        let hostname = self.get_hostname().await;

        info!("Starting ESPHome controller monitor for {}", hostname);

        // Start periodic health check
        let health_check_status = self.status.clone();
        let health_check_client = self.client.clone();
        // let health_check_settings = self.status.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                let hostname = health_check_status.read().await.hostname.clone();
                Self::perform_health_check(&health_check_client, &hostname, &health_check_status)
                    .await;
            }
        });

        // Main request processing loop
        loop {
            if let Ok(req) = self.global_rx.recv().await {
                debug!("Received global message: {:?}", req);
                self.handle_request(req).await;
            } else {
                error!("Controller monitor global receiver closed, exiting loop");
                break;
            }
        }

        Ok(())
    }

    /// Handle a single request from the web server
    async fn handle_request(&self, request: GlobalMessage) {
        debug!("Handling request: {:?}", request);
        let _response = match request {
            GlobalMessage::NextCase => {
                self.trigger_next_case().await;
            }
            GlobalMessage::NewConfig(settings) => {
                self.update_config(settings).await;
            }
            GlobalMessage::ControllerStatus { responder } => {
                if let Err(err) = responder.send(self.get_machine_status().await) {
                    error!("Failed to send controller status response: {err:?}");
                }
            }
            _ => {}
        };
        //     ControllerCommand::GetSensors => self.get_sensor_readings().await,
        //     ControllerCommand::GetHardwareStatus => self.get_hardware_status().await,
        //     ControllerCommand::TriggerVibration => self.trigger_vibration().await,
        //     ControllerCommand::SetServoPosition { servo, position } => {
        //         self.set_servo_position(&servo, position).await
        //     }
        //     ControllerCommand::UpdateConfig { new_settings } => {
        //         self.update_config(*new_settings).await
        //     }
        // };

        // if let Err(err) = request.response_sender.send(response) {
        //     debug!(
        //         "Failed to send response back to web server (receiver dropped - likely due to client timeout or connection closed): {err:?}",
        //     );
        // }
    }

    /// Update controller configuration
    async fn update_config(&self, new_settings: Settings) -> ControllerResponse {
        // TODO: Implement proper configuration update logic

        let new_hostname = new_settings.esphome_hostname.clone();

        let mut status_writer = self.status.write().await;

        if new_hostname != status_writer.hostname {
            debug!(
                "Controller hostname change detected: '{}' -> '{}'",
                status_writer.hostname, new_hostname
            );
            status_writer.hostname = new_hostname.clone();
            status_writer.online = false; // Reset online status until next health check
            status_writer.last_seen = None;
            status_writer.response_time_ms = None;
            status_writer.error_count = 0;
        }

        ControllerResponse::ConfigUpdated
    }

    /// Trigger the next case sequence on the controller
    async fn trigger_next_case(&self) -> ControllerResponse {
        let hostname = self.lock_status().await.hostname.clone();

        let url = format!("http://{hostname}/button/trigger_next_case/press");

        match self.make_request(&url, Method::POST).await {
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
    async fn get_machine_status(&self) -> ControllerStatus {
        todo!()
        // ControllerStatus {
        //     status: if self.is_online().await {
        //         "Ready".to_string()
        //     } else {
        //         "Offline".to_string()
        //     },
        //     ready: self.is_online().await,
        //     active_jobs: 0,
        //     last_update: chrono::Utc::now(),
        // }
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

    async fn get_hostname(&self) -> String {
        self.status.read().await.hostname.clone()
    }

    /// Get hardware status from the controller
    async fn get_hardware_status(&self) -> ControllerResponse {
        let mut status = HashMap::new();

        if self.is_online().await {
            status.insert("controller".to_string(), "Connected".to_string());
            let hostname = self.get_hostname().await;
            status.insert("esphome_hostname".to_string(), hostname);

            // Try to get additional status info
            if let Ok(info) = self.get_device_info().await {
                status.extend(info);
            }
        } else {
            status.insert("controller".to_string(), "Disconnected".to_string());
            let hostname = self.get_hostname().await;
            status.insert("esphome_hostname".to_string(), hostname);
        }

        ControllerResponse::HardwareData(status)
    }

    /// Trigger vibration motor
    async fn trigger_vibration(&self) -> ControllerResponse {
        let hostname = self.get_hostname().await;
        let url = format!("http://{hostname}/switch/vibration_motor/turn_on");

        match self.make_request(&url, Method::POST).await {
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
        let hostname = self.get_hostname().await;
        let url = format!("http://{hostname}/number/{servo}/set?value={position}");

        match self.make_request(&url, Method::POST).await {
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
        self.lock_status().await.online
    }

    /// Get binary sensor state from ESPHome
    async fn get_binary_sensor(
        &self,
        sensor_name: &str,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let hostname = self.get_hostname().await;
        let url = format!("http://{hostname}/binary_sensor/{sensor_name}/state");
        let response = self.make_request(&url, Method::GET).await?;

        // ESPHome returns "ON" or "OFF" for binary sensors
        Ok(response.trim().to_uppercase() == "ON")
    }

    /// Get device information from ESPHome
    async fn get_device_info(&self) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
        let hostname = self.get_hostname().await;
        let url = format!("http://{hostname}/text_sensor/device_info/state");

        let mut info = HashMap::new();
        match self.make_request(&url, Method::GET).await {
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
        method: reqwest::Method,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let start_time = Instant::now();
        let response = self
            .client
            .request(method.clone(), url)
            .basic_auth("admin", Some("shellsorter")) // TODO: store the creds
            .send()
            .await?;

        let elapsed = start_time.elapsed();

        if response.status().is_success() {
            let text = response.text().await?;

            // Update response time in status
            {
                let mut status = self.lock_status_write().await;
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
        status: &Arc<RwLock<ControllerStatus>>,
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
                    let mut status_lock = status.write().await;
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
                    let mut status_lock = status.write().await;
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
