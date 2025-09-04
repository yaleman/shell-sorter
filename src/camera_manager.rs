use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use reqwest::Url;
use serde::{Deserialize, Serialize};

use tokio::sync::oneshot;
use tracing::{debug, info, warn};

use crate::protocol::GlobalMessage;
use crate::{OurError, OurResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde_with::serde_as]
pub struct CameraInfo {
    pub hostname: String,
    #[serde_as(as = "DisplayFromStr")]
    pub stream_url: Url,
    #[serde_as(as = "DisplayFromStr")]
    pub snapshot_url: Url,
    pub online: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CameraStatus {
    pub cameras: HashMap<String, CameraInfo>,
    pub streaming: bool,
}

impl From<String> for CameraInfo {
    fn from(camera_id: String) -> Self {
        let camera_name = camera_id.clone();
        let hostname = format!("http://{camera_name}");

        CameraInfo {
            stream_url: Url::from_str(&format!("{hostname}/camera/stream")).unwrap(),
            snapshot_url: Url::from_str(&format!("{hostname}/camera/snapshot")).unwrap(),
            hostname,
            online: true,
        }
    }
}

impl From<Vec<String>> for CameraStatus {
    fn from(selected_cameras: Vec<String>) -> Self {
        let mut cameras = HashMap::new();

        for camera_id in selected_cameras {
            cameras.insert(camera_id.clone(), CameraInfo::from(camera_id));
        }

        CameraStatus {
            cameras,
            streaming: false,
        }
    }
}

#[derive(Debug)]
pub enum CameraRequest {
    DetectCameras,
    ListCameras {
        respond_to: oneshot::Sender<OurResult<Vec<CameraInfo>>>,
    },
    SelectCameras {
        camera_ids: Vec<String>,
        respond_to: oneshot::Sender<OurResult<()>>,
    },
    StartStreaming {
        respond_to: oneshot::Sender<OurResult<()>>,
    },
    StopStreaming {
        respond_to: oneshot::Sender<OurResult<()>>,
    },
    CaptureImage {
        camera_id: String,
        respond_to: oneshot::Sender<OurResult<Vec<u8>>>,
    },
    GetStatus {
        respond_to: oneshot::Sender<OurResult<CameraStatus>>,
    },
}

pub struct EspCameraManager {
    status: Arc<RwLock<CameraStatus>>,

    client: reqwest::Client,

    #[allow(dead_code)]
    global_tx: tokio::sync::broadcast::Sender<GlobalMessage>,
    global_rx: tokio::sync::broadcast::Receiver<GlobalMessage>,
}

impl EspCameraManager {
    async fn status_read(&self) -> tokio::sync::RwLockReadGuard<CameraStatus> {
        self.status.read().await
    }

    async fn status_write(&self) -> tokio::sync::RwLockWriteGuard<CameraStatus> {
        self.status.write().await
    }

    pub fn new(
        global_tx: tokio::sync::broadcast::Sender<GlobalMessage>,
        global_rx: tokio::sync::broadcast::Receiver<GlobalMessage>,
        network_camera_hostnames: Vec<String>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let status = Arc::new(RwLock::new(CameraStatus::from(network_camera_hostnames)));

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;

        Ok(Self {
            status: status.clone(),
            client,
            global_tx: global_tx.clone(),
            global_rx,
        })
    }

    pub async fn run(mut self) -> OurResult<()> {
        info!("Starting camera manager");
        while let Ok(request) = self.global_rx.recv().await {
            debug!("EspCameraManager Received global message: {:?}", request);
            match request {
                GlobalMessage::NewConfig(_settings) => {
                    // Handle new configuration if needed
                    debug!("EspCameraManager Received NewConfig message, but not implemented yet");
                }
                GlobalMessage::Shutdown => return Ok(()),
                GlobalMessage::NextCase => {}
                GlobalMessage::GetSensors(_) => {}
                GlobalMessage::ControllerStatus { .. } => {}
                GlobalMessage::MachineStatus(_sender) => {}
                GlobalMessage::SelectCameras(_camera_types) => {}

                GlobalMessage::StartCameras(_camera_types) => {
                    self.start_streaming().await?;
                }
                GlobalMessage::DetectCameras => self.detect_cameras().await?,
                GlobalMessage::StopCameras => self.stop_streaming().await?,
                GlobalMessage::SetCameraBrightness { .. } => {
                    debug!("Received SetCameraBrightness request, but not implemented yet");
                }
            }
        }

        info!("Camera manager stopped");
        Ok(())
    }

    async fn detect_cameras(&mut self) -> OurResult<()> {
        debug!("Detecting ESPHome cameras");

        let mut camera_writer = self.status.write().await;

        for (_camera_id, camera) in &mut camera_writer.cameras {
            match self.probe_esphome_camera(&camera.hostname).await {
                Ok(_camera_info) => {
                    camera.online = true;
                    info!("Detected camera at {}", camera.hostname);
                }
                Err(e) => {
                    camera.online = false;
                    warn!("Failed to detect camera at {}: {e}", camera.hostname);
                }
            }
        }

        Ok(())
    }

    #[allow(dead_code)]
    async fn list_cameras(&self) -> OurResult<Vec<CameraInfo>> {
        let status = self.status_read().await;
        Ok(status.cameras.values().cloned().collect())
    }
    #[allow(dead_code)]
    async fn select_cameras(&mut self, camera_ids: Vec<String>) -> OurResult<()> {
        let status = self.status_write().await;

        // Validate all camera IDs exist
        for id in &camera_ids {
            if !status.cameras.contains_key(id) {
                return Err(OurError::App(format!("Camera with ID '{id}' not found")));
            }
        }

        // status.selected_cameras = camera_ids;
        // info!("Selected cameras: {:?}", status.selected_cameras);
        Ok(())
    }

    #[allow(dead_code)]
    async fn start_streaming(&mut self) -> OurResult<()> {
        // let status = self.status_write().await;

        // // Allow starting streaming with no cameras selected - this is a valid state
        // if status.selected_cameras.is_empty() {
        //     info!("Starting streaming with no cameras selected - this is allowed");
        // }

        // status.streaming = true;
        // info!(
        //     "Started streaming from {} cameras",
        //     status.selected_cameras.len()
        // );
        Ok(())
    }

    #[allow(dead_code)]
    async fn stop_streaming(&mut self) -> OurResult<()> {
        let mut status = self.status_write().await;
        status.streaming = false;
        info!("Stopped camera streaming");
        Ok(())
    }

    #[allow(dead_code)]
    async fn capture_image(&self, camera_id: &str) -> OurResult<Vec<u8>> {
        let snapshot_url = {
            let status = self.status_read().await;
            let camera = status
                .cameras
                .get(camera_id)
                .ok_or_else(|| OurError::App(format!("Camera with ID '{camera_id}' not found")))?;

            if !camera.online {
                return Err(OurError::App(format!("Camera '{camera_id}' is offline")));
            }

            camera.snapshot_url.clone()
        };

        debug!("Capturing image from camera '{camera_id}' at {snapshot_url}");

        let response = self
            .client
            .get(snapshot_url)
            .send()
            .await
            .map_err(|e| OurError::App(format!("Failed to request snapshot: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            return Err(OurError::App(format!(
                "Snapshot request failed with status: {status}"
            )));
        }

        let image_bytes = response
            .bytes()
            .await
            .map_err(|e| OurError::App(format!("Failed to read image data: {e}")))?;

        let len = image_bytes.len();
        info!("Captured {len} bytes from camera '{camera_id}'");
        Ok(image_bytes.to_vec())
    }

    async fn probe_esphome_camera(&self, hostname: &str) -> OurResult<()> {
        let base_url = if hostname.starts_with("http://") {
            Url::from_str(hostname)?
        } else {
            Url::from_str(&format!("http://{hostname}"))?
        };

        // Try to get device info to verify it's an ESPHome device
        let info_url = base_url.join("/text_sensor/device_info")?;
        let response = self
            .client
            .get(info_url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| OurError::App(format!("Failed to probe camera: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            return Err(OurError::App(format!(
                "Camera probe failed with status: {status}"
            )));
        }
        Ok(())
    }
}
