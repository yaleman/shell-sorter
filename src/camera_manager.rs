use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use reqwest::Url;
use serde::{Deserialize, Serialize};

use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, info, warn};

use crate::protocol::GlobalMessage;
use crate::{OurError, OurResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde_with::serde_as]
pub struct CameraInfo {
    pub id: String,
    pub name: String,
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
    pub selected_cameras: Vec<String>,
    pub streaming: bool,
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

pub struct CameraManager {
    network_camera_hostnames: Vec<String>,
    status: Arc<RwLock<CameraStatus>>,
    request_receiver: mpsc::UnboundedReceiver<CameraRequest>,
    client: reqwest::Client,
}

pub struct CameraHandle {
    request_sender: mpsc::UnboundedSender<CameraRequest>,
    #[allow(dead_code)]
    global_tx: tokio::sync::broadcast::Sender<GlobalMessage>,
    #[allow(dead_code)]
    global_rx: Arc<tokio::sync::broadcast::Receiver<GlobalMessage>>,

    #[allow(dead_code)]
    status: Arc<RwLock<CameraStatus>>,
}

impl CameraHandle {
    pub async fn detect_cameras(&self) -> OurResult<()> {
        self.request_sender
            .send(CameraRequest::DetectCameras)
            .map_err(|_| OurError::App("Camera manager channel closed".to_string()))?;
        Ok(())
    }

    pub async fn list_cameras(&self) -> OurResult<Vec<CameraInfo>> {
        let (sender, receiver) = oneshot::channel();
        self.request_sender
            .send(CameraRequest::ListCameras { respond_to: sender })
            .map_err(|_| OurError::App("Camera manager channel closed".to_string()))?;
        receiver
            .await
            .map_err(|_| OurError::App("Camera manager response failed".to_string()))?
    }

    pub async fn select_cameras(&self, camera_ids: Vec<String>) -> OurResult<()> {
        let (sender, receiver) = oneshot::channel();
        self.request_sender
            .send(CameraRequest::SelectCameras {
                camera_ids,
                respond_to: sender,
            })
            .map_err(|_| OurError::App("Camera manager channel closed".to_string()))?;
        receiver
            .await
            .map_err(|_| OurError::App("Camera manager response failed".to_string()))?
    }

    pub async fn start_streaming(&self) -> OurResult<()> {
        let (sender, receiver) = oneshot::channel();
        self.request_sender
            .send(CameraRequest::StartStreaming { respond_to: sender })
            .map_err(|_| OurError::App("Camera manager channel closed".to_string()))?;
        receiver
            .await
            .map_err(|_| OurError::App("Camera manager response failed".to_string()))?
    }

    pub async fn stop_streaming(&self) -> OurResult<()> {
        let (sender, receiver) = oneshot::channel();
        self.request_sender
            .send(CameraRequest::StopStreaming { respond_to: sender })
            .map_err(|_| OurError::App("Camera manager channel closed".to_string()))?;
        receiver
            .await
            .map_err(|_| OurError::App("Camera manager response failed".to_string()))?
    }

    pub async fn capture_image(&self, camera_id: String) -> OurResult<Vec<u8>> {
        let (sender, receiver) = oneshot::channel();
        self.request_sender
            .send(CameraRequest::CaptureImage {
                camera_id,
                respond_to: sender,
            })
            .map_err(|_| OurError::App("Camera manager channel closed".to_string()))?;
        receiver
            .await
            .map_err(|_| OurError::App("Camera manager response failed".to_string()))?
    }

    pub async fn get_status(&self) -> OurResult<CameraStatus> {
        let (sender, receiver) = oneshot::channel();
        self.request_sender
            .send(CameraRequest::GetStatus { respond_to: sender })
            .map_err(|_| OurError::App("Camera manager channel closed".to_string()))?;
        receiver
            .await
            .map_err(|_| OurError::App("Camera manager response failed".to_string()))?
    }
}

impl CameraManager {
    async fn lock_status(&self) -> tokio::sync::RwLockReadGuard<CameraStatus> {
        self.status.read().await
    }

    async fn lock_status_write(&self) -> tokio::sync::RwLockWriteGuard<CameraStatus> {
        self.status.write().await
    }

    pub fn new(
        global_tx: tokio::sync::broadcast::Sender<GlobalMessage>,
        global_rx: Arc<tokio::sync::broadcast::Receiver<GlobalMessage>>,
        network_camera_hostnames: Vec<String>,
    ) -> Result<(Self, CameraHandle), Box<dyn std::error::Error>> {
        let (request_sender, request_receiver) = mpsc::unbounded_channel();

        let status = Arc::new(RwLock::new(CameraStatus::default()));

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;

        let manager = Self {
            network_camera_hostnames,
            status: status.clone(),
            request_receiver,
            client,
        };

        let handle = CameraHandle {
            request_sender,
            global_tx,
            global_rx,
            status,
        };

        Ok((manager, handle))
    }

    pub async fn run(mut self) -> OurResult<()> {
        info!("Starting camera manager");

        while let Some(request) = self.request_receiver.recv().await {
            match request {
                CameraRequest::DetectCameras => {
                    let result = self.detect_cameras().await;
                    debug!("Camera detection result: {result:?}",);
                }
                CameraRequest::ListCameras { respond_to } => {
                    let result = self.list_cameras().await;
                    if respond_to.send(result).is_err() {
                        error!("Failed to send camera list response");
                    }
                }
                CameraRequest::SelectCameras {
                    camera_ids,
                    respond_to,
                } => {
                    let result = self.select_cameras(camera_ids).await;
                    if respond_to.send(result).is_err() {
                        error!("Failed to send camera selection response");
                    }
                }
                CameraRequest::StartStreaming { respond_to } => {
                    let result = self.start_streaming().await;
                    if respond_to.send(result).is_err() {
                        error!("Failed to send streaming start response");
                    }
                }
                CameraRequest::StopStreaming { respond_to } => {
                    let result = self.stop_streaming().await;
                    if respond_to.send(result).is_err() {
                        error!("Failed to send streaming stop response");
                    }
                }
                CameraRequest::CaptureImage {
                    camera_id,
                    respond_to,
                } => {
                    let result = self.capture_image(&camera_id).await;
                    if respond_to.send(result).is_err() {
                        error!("Failed to send image capture response");
                    }
                }
                CameraRequest::GetStatus { respond_to } => {
                    let status = Ok(self.lock_status().await.clone());
                    if let Err(err) = respond_to.send(status) {
                        error!("Failed to send status response: {err:?}");
                    }
                }
            }
        }

        info!("Camera manager stopped");
        Ok(())
    }

    async fn detect_cameras(&mut self) -> OurResult<Vec<CameraInfo>> {
        debug!("Detecting ESPHome cameras");
        let mut cameras = Vec::new();

        for hostname in &self.network_camera_hostnames {
            match self.probe_esphome_camera(hostname).await {
                Ok(camera_info) => {
                    cameras.push(camera_info);
                    info!("Detected camera at {hostname}");
                }
                Err(e) => {
                    warn!("Failed to detect camera at {hostname}: {e}");
                }
            }
        }

        // Update status
        {
            let mut status = self.lock_status_write().await;
            status.cameras.clear();
            for camera in &cameras {
                status.cameras.insert(camera.id.clone(), camera.clone());
            }
        }

        Ok(cameras)
    }

    async fn list_cameras(&self) -> OurResult<Vec<CameraInfo>> {
        let status = self.lock_status().await;
        Ok(status.cameras.values().cloned().collect())
    }

    async fn select_cameras(&mut self, camera_ids: Vec<String>) -> OurResult<()> {
        let mut status = self.lock_status_write().await;

        // Validate all camera IDs exist
        for id in &camera_ids {
            if !status.cameras.contains_key(id) {
                return Err(OurError::App(format!("Camera with ID '{id}' not found")));
            }
        }

        status.selected_cameras = camera_ids;
        info!("Selected cameras: {:?}", status.selected_cameras);
        Ok(())
    }

    async fn start_streaming(&mut self) -> OurResult<()> {
        let mut status = self.lock_status_write().await;

        // Allow starting streaming with no cameras selected - this is a valid state
        if status.selected_cameras.is_empty() {
            info!("Starting streaming with no cameras selected - this is allowed");
        }

        status.streaming = true;
        info!(
            "Started streaming from {} cameras",
            status.selected_cameras.len()
        );
        Ok(())
    }

    async fn stop_streaming(&mut self) -> OurResult<()> {
        let mut status = self.lock_status_write().await;
        status.streaming = false;
        info!("Stopped camera streaming");
        Ok(())
    }

    async fn capture_image(&self, camera_id: &str) -> OurResult<Vec<u8>> {
        let snapshot_url = {
            let status = self.lock_status().await;
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

    async fn probe_esphome_camera(&self, hostname: &str) -> OurResult<CameraInfo> {
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

        // Extract camera name from hostname (remove protocol and port)
        let camera_name = hostname
            .replace("http://", "")
            .replace("https://", "")
            .split(':')
            .next()
            .unwrap_or(hostname)
            .to_string();

        let camera_id = format!("esphome_{camera_name}");

        Ok(CameraInfo {
            id: camera_id,
            name: camera_name.clone(),
            hostname: hostname.to_string(),
            stream_url: base_url.join("/camera/stream")?,
            snapshot_url: base_url.join("/camera/snapshot")?,
            online: true,
        })
    }
}
