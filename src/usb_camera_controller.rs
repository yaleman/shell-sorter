//! USB Camera Controller with device identification
//!
//! This module provides direct USB camera access with hardware-based device identification
//! using vendor/product IDs and serial numbers for stable camera mapping across system reboots.

use image::DynamicImage;
use nokhwa::{
    Camera,
    pixel_format::RgbFormat,
    utils::{
        ApiBackend, CameraIndex, CameraInfo as NokhwaCameraInfo, RequestedFormat,
        RequestedFormatType,
    },
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, info, warn};

use crate::{OurError, OurResult};

/// USB Camera device information with hardware identification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsbCameraInfo {
    /// Camera index for access
    pub index: u32,
    /// Camera name/model
    pub name: String,
    /// Hardware vendor ID (USB VID)
    pub vendor_id: Option<String>,
    /// Hardware product ID (USB PID)
    pub product_id: Option<String>,
    /// Device serial number
    pub serial_number: Option<String>,
    /// Stable hardware-based identifier
    pub hardware_id: String,
    /// Current connection status
    pub connected: bool,
    /// Supported resolutions
    pub supported_formats: Vec<CameraFormatInfo>,
    /// Currently selected format
    pub current_format: Option<CameraFormatInfo>,
}

/// Camera format information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraFormatInfo {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub format: String,
}

/// USB Camera status information
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsbCameraStatus {
    /// Available USB cameras by hardware ID
    pub cameras: HashMap<String, UsbCameraInfo>,
    /// Currently selected cameras
    pub selected_cameras: Vec<String>,
    /// Streaming status
    pub streaming: bool,
    /// Last detection timestamp
    pub last_detection: Option<chrono::DateTime<chrono::Utc>>,
}

/// USB Camera control commands
#[derive(Debug)]
pub enum UsbCameraRequest {
    /// Detect and enumerate USB cameras
    DetectCameras {
        respond_to: oneshot::Sender<OurResult<Vec<UsbCameraInfo>>>,
    },
    /// List currently known cameras
    ListCameras {
        respond_to: oneshot::Sender<OurResult<Vec<UsbCameraInfo>>>,
    },
    /// Select cameras for operations
    SelectCameras {
        hardware_ids: Vec<String>,
        respond_to: oneshot::Sender<OurResult<()>>,
    },
    /// Start streaming from selected cameras
    StartStreaming {
        respond_to: oneshot::Sender<OurResult<()>>,
    },
    /// Stop all streaming
    StopStreaming {
        respond_to: oneshot::Sender<OurResult<()>>,
    },
    /// Capture image from specific camera
    CaptureImage {
        hardware_id: String,
        respond_to: oneshot::Sender<OurResult<Vec<u8>>>,
    },
    /// Get current status
    GetStatus {
        respond_to: oneshot::Sender<OurResult<UsbCameraStatus>>,
    },
    /// Set camera brightness
    SetBrightness {
        hardware_id: String,
        brightness: i64,
        respond_to: oneshot::Sender<OurResult<()>>,
    },
    /// Get camera brightness
    GetBrightness {
        hardware_id: String,
        respond_to: oneshot::Sender<OurResult<i64>>,
    },
    /// Set camera format
    SetCameraFormat {
        hardware_id: String,
        format: CameraFormatInfo,
        respond_to: oneshot::Sender<OurResult<()>>,
    },
    /// Capture streaming frame from specific camera
    CaptureStreamingFrame {
        hardware_id: String,
        response_sender: oneshot::Sender<OurResult<Vec<u8>>>,
    },
}

/// USB Camera Manager implementation
pub struct UsbCameraManager {
    /// Current camera status
    status: Arc<Mutex<UsbCameraStatus>>,
    /// Request receiver channel
    request_receiver: mpsc::UnboundedReceiver<UsbCameraRequest>,
    /// Preferred API backend
    backend: ApiBackend,
    /// Software brightness adjustments per camera (hardware_id -> brightness_offset)
    brightness_adjustments: HashMap<String, f32>,
}

/// Handle for communicating with USB Camera Manager
#[derive(Clone)]
pub struct UsbCameraHandle {
    request_sender: mpsc::UnboundedSender<UsbCameraRequest>,
    #[allow(dead_code)]
    status: Arc<Mutex<UsbCameraStatus>>,
}

impl UsbCameraHandle {
    /// Detect available USB cameras
    pub async fn detect_cameras(&self) -> OurResult<Vec<UsbCameraInfo>> {
        let (sender, receiver) = oneshot::channel();
        self.request_sender
            .send(UsbCameraRequest::DetectCameras { respond_to: sender })
            .map_err(|_| OurError::App("USB camera manager channel closed".to_string()))?;
        receiver
            .await
            .map_err(|_| OurError::App("USB camera manager response failed".to_string()))?
    }

    /// List currently known cameras
    pub async fn list_cameras(&self) -> OurResult<Vec<UsbCameraInfo>> {
        let (sender, receiver) = oneshot::channel();
        self.request_sender
            .send(UsbCameraRequest::ListCameras { respond_to: sender })
            .map_err(|_| OurError::App("USB camera manager channel closed".to_string()))?;
        receiver
            .await
            .map_err(|_| OurError::App("USB camera manager response failed".to_string()))?
    }

    /// Select cameras for operations
    pub async fn select_cameras(&self, hardware_ids: Vec<String>) -> OurResult<()> {
        let (sender, receiver) = oneshot::channel();
        self.request_sender
            .send(UsbCameraRequest::SelectCameras {
                hardware_ids,
                respond_to: sender,
            })
            .map_err(|_| OurError::App("USB camera manager channel closed".to_string()))?;
        receiver
            .await
            .map_err(|_| OurError::App("USB camera manager response failed".to_string()))?
    }

    /// Start streaming from selected cameras
    pub async fn start_streaming(&self) -> OurResult<()> {
        let (sender, receiver) = oneshot::channel();
        self.request_sender
            .send(UsbCameraRequest::StartStreaming { respond_to: sender })
            .map_err(|_| OurError::App("USB camera manager channel closed".to_string()))?;
        receiver
            .await
            .map_err(|_| OurError::App("USB camera manager response failed".to_string()))?
    }

    /// Capture a single frame from a specific camera for streaming
    pub async fn capture_streaming_frame(&self, hardware_id: &str) -> OurResult<Vec<u8>> {
        let (request_sender, response_receiver) = oneshot::channel();

        let request = UsbCameraRequest::CaptureStreamingFrame {
            hardware_id: hardware_id.to_string(),
            response_sender: request_sender,
        };

        self.request_sender
            .send(request)
            .map_err(|_| OurError::App("USB camera manager channel closed".to_string()))?;
        response_receiver
            .await
            .map_err(|_| OurError::App("USB camera manager response failed".to_string()))?
    }

    /// Stop all streaming
    pub async fn stop_streaming(&self) -> OurResult<()> {
        let (sender, receiver) = oneshot::channel();
        self.request_sender
            .send(UsbCameraRequest::StopStreaming { respond_to: sender })
            .map_err(|_| OurError::App("USB camera manager channel closed".to_string()))?;
        receiver
            .await
            .map_err(|_| OurError::App("USB camera manager response failed".to_string()))?
    }

    /// Get current status including selected cameras and streaming state
    pub async fn get_status(&self) -> OurResult<UsbCameraStatus> {
        let (sender, receiver) = oneshot::channel();
        self.request_sender
            .send(UsbCameraRequest::GetStatus { respond_to: sender })
            .map_err(|_| OurError::App("USB camera manager channel closed".to_string()))?;
        receiver
            .await
            .map_err(|_| OurError::App("USB camera manager response failed".to_string()))?
    }

    /// Capture image from specific camera
    pub async fn capture_image(&self, hardware_id: String) -> OurResult<Vec<u8>> {
        let (sender, receiver) = oneshot::channel();
        self.request_sender
            .send(UsbCameraRequest::CaptureImage {
                hardware_id,
                respond_to: sender,
            })
            .map_err(|_| OurError::App("USB camera manager channel closed".to_string()))?;
        receiver
            .await
            .map_err(|_| OurError::App("USB camera manager response failed".to_string()))?
    }

    /// Set camera format
    pub async fn set_camera_format(
        &self,
        hardware_id: String,
        format: CameraFormatInfo,
    ) -> OurResult<()> {
        let (sender, receiver) = oneshot::channel();
        self.request_sender
            .send(UsbCameraRequest::SetCameraFormat {
                hardware_id,
                format,
                respond_to: sender,
            })
            .map_err(|_| OurError::App("USB camera manager channel closed".to_string()))?;
        receiver
            .await
            .map_err(|_| OurError::App("USB camera manager response failed".to_string()))?
    }

    /// Set camera brightness
    pub async fn set_brightness(&self, hardware_id: String, brightness: i64) -> OurResult<()> {
        let (sender, receiver) = oneshot::channel();
        self.request_sender
            .send(UsbCameraRequest::SetBrightness {
                hardware_id,
                brightness,
                respond_to: sender,
            })
            .map_err(|_| OurError::App("USB camera manager channel closed".to_string()))?;
        receiver
            .await
            .map_err(|_| OurError::App("USB camera manager response failed".to_string()))?
    }

    /// Get camera brightness
    pub async fn get_brightness(&self, hardware_id: String) -> OurResult<i64> {
        let (sender, receiver) = oneshot::channel();
        self.request_sender
            .send(UsbCameraRequest::GetBrightness {
                hardware_id,
                respond_to: sender,
            })
            .map_err(|_| OurError::App("USB camera manager channel closed".to_string()))?;
        receiver
            .await
            .map_err(|_| OurError::App("USB camera manager response failed".to_string()))?
    }
}

impl UsbCameraManager {
    /// Get read-only access to camera status
    fn get_status(&self) -> std::sync::MutexGuard<UsbCameraStatus> {
        self.status.lock().unwrap_or_else(|e| {
            error!("USB camera status mutex poisoned: {e}");
            e.into_inner()
        })
    }

    /// Get mutable access to camera status
    fn get_status_mut(&mut self) -> std::sync::MutexGuard<UsbCameraStatus> {
        self.status.lock().unwrap_or_else(|e| {
            error!("USB camera status mutex poisoned: {e}");
            e.into_inner()
        })
    }

    /// Get camera info by hardware ID
    fn get_camera_info(&self, hardware_id: &str) -> OurResult<UsbCameraInfo> {
        let status = self.get_status();
        status
            .cameras
            .values()
            .find(|camera| camera.hardware_id == hardware_id)
            .cloned()
            .ok_or_else(|| OurError::App(format!("Camera with ID '{hardware_id}' not found")))
    }

    /// Create a new camera instance with efficient error handling
    fn create_camera(&self, hardware_id: &str) -> OurResult<Camera> {
        let camera_info = self.get_camera_info(hardware_id)?;
        let camera_index = CameraIndex::Index(camera_info.index);

        // Use highest resolution for best quality
        let format =
            RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestResolution);
        Camera::new(camera_index, format)
            .map_err(|e| OurError::App(format!("Failed to create camera {hardware_id}: {e}")))
    }

    /// Apply software brightness adjustment to an image
    fn apply_brightness_adjustment(&self, image: &mut image::RgbImage, hardware_id: &str) {
        if let Some(&brightness_offset) = self.brightness_adjustments.get(hardware_id) {
            if brightness_offset != 0.0 {
                debug!(
                    "Applying brightness adjustment of {} to camera {}",
                    brightness_offset, hardware_id
                );

                // Convert brightness from -100 to +100 range to a multiplier
                // -100 = 0.0 (black), 0 = 1.0 (no change), +100 = 4.0 (quadruple brightness)
                // This provides much brighter images for dark cameras like FaceTime
                let brightness_multiplier = if brightness_offset >= 0.0 {
                    // For positive adjustments: 0 to +100 maps to 1.0 to 4.0
                    1.0 + (brightness_offset / 100.0) * 3.0
                } else {
                    // For negative adjustments: -100 to 0 maps to 0.0 to 1.0
                    (brightness_offset + 100.0) / 100.0
                };

                // Apply brightness adjustment to each pixel
                for pixel in image.pixels_mut() {
                    let [r, g, b] = pixel.0;

                    // Apply brightness adjustment and clamp to valid range
                    let new_r = ((r as f32 * brightness_multiplier).clamp(0.0, 255.0)) as u8;
                    let new_g = ((g as f32 * brightness_multiplier).clamp(0.0, 255.0)) as u8;
                    let new_b = ((b as f32 * brightness_multiplier).clamp(0.0, 255.0)) as u8;

                    *pixel = image::Rgb([new_r, new_g, new_b]);
                }
            }
        }
    }

    /// Create new USB camera manager
    pub fn new() -> OurResult<(UsbCameraManager, UsbCameraHandle)> {
        let (request_sender, request_receiver) = mpsc::unbounded_channel();
        let status = Arc::new(Mutex::new(UsbCameraStatus::default()));

        let backend = Self::select_best_backend()?;

        let manager = UsbCameraManager {
            status: status.clone(),
            request_receiver,
            backend,
            brightness_adjustments: HashMap::new(),
        };

        let handle = UsbCameraHandle {
            request_sender,
            status,
        };

        Ok((manager, handle))
    }

    /// Select the best API backend for the current platform
    fn select_best_backend() -> OurResult<ApiBackend> {
        #[cfg(target_os = "linux")]
        return Ok(ApiBackend::Video4Linux);

        #[cfg(target_os = "windows")]
        return Ok(ApiBackend::MediaFoundation);

        #[cfg(target_os = "macos")]
        return Ok(ApiBackend::AVFoundation);

        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        {
            error!(
                "Unsupported platform for USB camera access - only Linux, Windows, and macOS are supported"
            );
            Err(OurError::App(
                "Unsupported platform for USB camera access".to_string(),
            ))
        }
    }

    /// Run the USB camera manager event loop
    pub async fn run(&mut self) -> OurResult<()> {
        info!(
            "Starting USB camera manager with backend: {:?}",
            self.backend
        );

        // Initial camera detection
        if let Err(e) = self.detect_cameras_internal().await {
            warn!("Initial camera detection failed: {e}");
        }

        // Main event loop
        while let Some(request) = self.request_receiver.recv().await {
            match request {
                UsbCameraRequest::DetectCameras { respond_to } => {
                    let result = self.detect_cameras_internal().await;
                    let _cameras = match &result {
                        Ok(cameras) => cameras.clone(),
                        Err(_) => Vec::new(),
                    };
                    if respond_to.send(result).is_err() {
                        debug!("Failed to send camera detection response");
                    }
                }
                UsbCameraRequest::ListCameras { respond_to } => {
                    let cameras = self.list_cameras_internal();
                    if respond_to.send(Ok(cameras)).is_err() {
                        debug!("Failed to send camera list response");
                    }
                }
                UsbCameraRequest::SelectCameras {
                    hardware_ids,
                    respond_to,
                } => {
                    let result = self.select_cameras_internal(hardware_ids);
                    if respond_to.send(result).is_err() {
                        debug!("Failed to send camera selection response");
                    }
                }
                UsbCameraRequest::StartStreaming { respond_to } => {
                    let result = self.start_streaming_internal().await;
                    if respond_to.send(result).is_err() {
                        debug!("Failed to send streaming start response");
                    }
                }
                UsbCameraRequest::StopStreaming { respond_to } => {
                    let result = self.stop_streaming_internal();
                    if respond_to.send(result).is_err() {
                        debug!("Failed to send streaming stop response");
                    }
                }
                UsbCameraRequest::CaptureImage {
                    hardware_id,
                    respond_to,
                } => {
                    let result = self.capture_image_internal(&hardware_id).await;
                    if respond_to.send(result).is_err() {
                        debug!("Failed to send image capture response");
                    }
                }
                UsbCameraRequest::GetStatus { respond_to } => {
                    let status = self.get_status_internal();
                    if respond_to.send(Ok(status)).is_err() {
                        debug!("Failed to send status response");
                    }
                }
                UsbCameraRequest::SetCameraFormat {
                    hardware_id,
                    format,
                    respond_to,
                } => {
                    let result = self.set_camera_format_internal(&hardware_id, format).await;
                    if respond_to.send(result).is_err() {
                        debug!("Failed to send camera format response");
                    }
                }
                UsbCameraRequest::CaptureStreamingFrame {
                    hardware_id,
                    response_sender,
                } => {
                    let result = self.capture_streaming_frame_internal(&hardware_id).await;
                    if response_sender.send(result).is_err() {
                        debug!("Failed to send streaming frame response");
                    }
                }
                UsbCameraRequest::SetBrightness {
                    hardware_id,
                    brightness,
                    respond_to,
                } => {
                    let result = self.set_brightness_internal(&hardware_id, brightness).await;
                    if respond_to.send(result).is_err() {
                        debug!("Failed to send brightness set response");
                    }
                }
                UsbCameraRequest::GetBrightness {
                    hardware_id,
                    respond_to,
                } => {
                    let result = self.get_brightness_internal(&hardware_id).await;
                    if respond_to.send(result).is_err() {
                        debug!("Failed to send brightness get response");
                    }
                }
            }
        }

        info!("USB camera manager shutting down");
        Ok(())
    }

    /// Internal camera detection implementation
    async fn detect_cameras_internal(&mut self) -> OurResult<Vec<UsbCameraInfo>> {
        info!("Detecting USB cameras with backend: {:?}", self.backend);

        let cameras = match nokhwa::query(self.backend) {
            Ok(camera_list) => camera_list,
            Err(e) => {
                error!("Failed to query cameras: {e}");
                return Err(OurError::App(format!("Failed to query cameras: {e}")));
            }
        };

        let mut detected_cameras = Vec::new();
        let mut detected_hardware_ids = std::collections::HashSet::new();

        for (index, camera_info) in cameras.iter().enumerate() {
            let usb_camera_info = self.create_camera_info(index as u32, camera_info).await;
            detected_cameras.push(usb_camera_info.clone());
            detected_hardware_ids.insert(usb_camera_info.hardware_id.clone());

            // Update status one camera at a time to avoid holding lock across await
            {
                let mut status = self.get_status_mut();
                status
                    .cameras
                    .insert(usb_camera_info.hardware_id.clone(), usb_camera_info);
            }
        }

        // Remove cameras that are no longer detected from status
        let cameras_to_remove: Vec<String> = {
            let status = self.get_status();
            status
                .cameras
                .keys()
                .filter(|hardware_id| !detected_hardware_ids.contains(*hardware_id))
                .cloned()
                .collect()
        };

        for hardware_id in cameras_to_remove {
            info!("Camera {hardware_id} no longer detected, removing from status");
            let mut status = self.get_status_mut();
            status.cameras.remove(&hardware_id);
        }

        // Update last detection time
        {
            let mut status = self.get_status_mut();
            status.last_detection = Some(chrono::Utc::now());
        }

        info!("Detected {} USB cameras", detected_cameras.len());
        Ok(detected_cameras)
    }

    /// Create camera info from nokhwa camera info
    async fn create_camera_info(
        &self,
        index: u32,
        camera_info: &NokhwaCameraInfo,
    ) -> UsbCameraInfo {
        // Extract hardware identification
        let (vendor_id, product_id, serial_number) =
            self.extract_hardware_identifiers(index, camera_info);
        let hardware_id =
            self.generate_hardware_id(index, camera_info, &vendor_id, &product_id, &serial_number);

        // Try to get supported formats
        let supported_formats = self.get_camera_formats(index).await.unwrap_or_default();

        UsbCameraInfo {
            index,
            name: camera_info.human_name().to_string(),
            vendor_id,
            product_id,
            serial_number,
            hardware_id,
            connected: true,
            supported_formats,
            current_format: None,
        }
    }

    /// Extract hardware identifiers from system
    fn extract_hardware_identifiers(
        &self,
        index: u32,
        camera_info: &NokhwaCameraInfo,
    ) -> (Option<String>, Option<String>, Option<String>) {
        // For now, extract what we can from the camera description
        // TODO: Implement platform-specific hardware ID extraction
        let desc = camera_info.description();
        debug!("Extracting hardware info for camera {}: {}", index, desc);

        // Try to parse vendor/product IDs from description if available
        // Many cameras include this in their description string
        let vendor_id = self.parse_vendor_id_from_description(desc);
        let product_id = self.parse_product_id_from_description(desc);
        let serial_number = self.parse_serial_from_description(desc);

        (vendor_id, product_id, serial_number)
    }

    /// Parse vendor ID from camera description
    fn parse_vendor_id_from_description(&self, description: &str) -> Option<String> {
        // Look for common patterns like "VID_1234" or "Vendor:1234"
        if let Some(captures) = regex::Regex::new(r"(?i)vid[_:]([0-9a-f]{4})")
            .ok()?
            .captures(description)
        {
            return captures.get(1).map(|m| m.as_str().to_uppercase());
        }
        None
    }

    /// Parse product ID from camera description
    fn parse_product_id_from_description(&self, description: &str) -> Option<String> {
        // Look for common patterns like "PID_5678" or "Product:5678"
        if let Some(captures) = regex::Regex::new(r"(?i)pid[_:]([0-9a-f]{4})")
            .ok()?
            .captures(description)
        {
            return captures.get(1).map(|m| m.as_str().to_uppercase());
        }
        None
    }

    /// Parse serial number from camera description
    fn parse_serial_from_description(&self, description: &str) -> Option<String> {
        // Look for serial number patterns
        if let Some(captures) = regex::Regex::new(r"(?i)s[en]r?[_:]([0-9a-f]+)")
            .ok()?
            .captures(description)
        {
            return captures.get(1).map(|m| m.as_str().to_uppercase());
        }
        None
    }

    /// Generate stable hardware ID for camera
    fn generate_hardware_id(
        &self,
        index: u32,
        camera_info: &NokhwaCameraInfo,
        vendor_id: &Option<String>,
        product_id: &Option<String>,
        serial_number: &Option<String>,
    ) -> String {
        // Create stable identifier based on available hardware info
        let mut parts = vec!["usb".to_string()];

        if let (Some(vid), Some(pid)) = (vendor_id, product_id) {
            parts.push(format!("{vid}:{pid}"));

            if let Some(serial) = serial_number {
                parts.push(serial.clone());
            } else {
                // Use camera name as fallback if no serial
                parts.push(camera_info.human_name().replace(' ', "_").to_lowercase());
            }
        } else {
            // Fallback to description-based ID
            let desc = camera_info.description().replace(' ', "_").to_lowercase();
            parts.push(format!("{desc}:{index}"));
        }

        parts.join(":")
    }

    /// Get supported camera formats
    async fn get_camera_formats(&self, index: u32) -> OurResult<Vec<CameraFormatInfo>> {
        debug!("Getting default formats for camera {}", index);

        // Instead of trying to access the camera (which can panic or fail),
        // return a reasonable set of common formats that most USB cameras support
        let default_formats = vec![
            CameraFormatInfo {
                width: 320,
                height: 240,
                fps: 30,
                format: "MJPEG".to_string(),
            },
            CameraFormatInfo {
                width: 640,
                height: 480,
                fps: 30,
                format: "MJPEG".to_string(),
            },
            CameraFormatInfo {
                width: 1280,
                height: 720,
                fps: 30,
                format: "MJPEG".to_string(),
            },
            CameraFormatInfo {
                width: 1920,
                height: 1080,
                fps: 30,
                format: "MJPEG".to_string(),
            },
        ];

        debug!(
            "Returning {} default formats for camera {}",
            default_formats.len(),
            index
        );

        Ok(default_formats)
    }

    /// List currently known cameras
    fn list_cameras_internal(&self) -> Vec<UsbCameraInfo> {
        let status = self.get_status();
        status.cameras.values().cloned().collect()
    }

    /// Select cameras for operations
    fn select_cameras_internal(&mut self, hardware_ids: Vec<String>) -> OurResult<()> {
        let mut status = self.get_status_mut();

        // Validate that all requested cameras exist
        for hardware_id in &hardware_ids {
            if !status.cameras.contains_key(hardware_id) {
                return Err(OurError::App(format!("Camera not found: {hardware_id}")));
            }
        }

        status.selected_cameras = hardware_ids;
        info!("Selected {} cameras", status.selected_cameras.len());
        Ok(())
    }

    /// Start streaming from selected cameras
    async fn start_streaming_internal(&mut self) -> OurResult<()> {
        // Update streaming status - actual streaming is done on-demand during capture
        let mut status = self.get_status_mut();

        // Allow starting streaming with no cameras selected - this is a valid state
        if status.selected_cameras.is_empty() {
            info!("Starting streaming with no cameras selected - this is allowed");
        }

        status.streaming = true;
        let camera_count = status.selected_cameras.len();
        drop(status);

        info!("Enabled streaming for {} cameras", camera_count);
        Ok(())
    }

    /// Stop all streaming
    fn stop_streaming_internal(&mut self) -> OurResult<()> {
        // Update streaming status
        let mut status = self.get_status_mut();

        let camera_count = status.selected_cameras.len();
        status.streaming = false;

        info!("Disabled streaming for {} cameras", camera_count);
        Ok(())
    }

    /// Capture streaming frame from specific camera (optimized for streaming)
    async fn capture_streaming_frame_internal(&mut self, hardware_id: &str) -> OurResult<Vec<u8>> {
        // Create camera instance
        let mut camera = self.create_camera(hardware_id)?;

        // Open camera stream
        camera
            .open_stream()
            .map_err(|e| OurError::App(format!("Failed to open camera stream: {e}")))?;

        match camera.frame() {
            Ok(frame) => {
                // Convert frame to RGB image
                let mut image = frame
                    .decode_image::<RgbFormat>()
                    .map_err(|e| OurError::App(format!("Failed to decode frame: {e}")))?;

                // Apply software brightness adjustment
                self.apply_brightness_adjustment(&mut image, hardware_id);

                // Convert to JPEG with lower quality for streaming
                let mut jpeg_data = Vec::new();
                let mut cursor = std::io::Cursor::new(&mut jpeg_data);

                DynamicImage::ImageRgb8(image)
                    .write_to(&mut cursor, image::ImageFormat::Jpeg)
                    .map_err(|e| OurError::App(format!("Failed to encode JPEG: {e}")))?;

                // Clean up camera
                if let Err(e) = camera.stop_stream() {
                    warn!("Failed to stop camera stream: {e}");
                }

                debug!(
                    "Captured streaming frame from camera {} ({} bytes)",
                    hardware_id,
                    jpeg_data.len()
                );
                Ok(jpeg_data)
            }
            Err(e) => {
                warn!("Failed to capture frame from camera {hardware_id}: {e}");
                if let Err(stop_err) = camera.stop_stream() {
                    warn!("Failed to stop camera stream after error: {stop_err}");
                }
                Err(OurError::App(format!(
                    "Failed to capture streaming frame: {e}"
                )))
            }
        }
    }

    async fn capture_image_internal(&mut self, hardware_id: &str) -> OurResult<Vec<u8>> {
        // Create camera instance
        let mut camera = self.create_camera(hardware_id)?;

        // Open camera stream
        camera
            .open_stream()
            .map_err(|e| OurError::App(format!("Failed to open camera stream: {e}")))?;

        match camera.frame() {
            Ok(frame) => {
                // Convert frame to RGB image
                let mut image = frame
                    .decode_image::<RgbFormat>()
                    .map_err(|e| OurError::App(format!("Failed to decode frame: {e}")))?;

                // Apply software brightness adjustment
                self.apply_brightness_adjustment(&mut image, hardware_id);

                // Convert to JPEG
                let mut jpeg_data = Vec::new();
                let mut cursor = std::io::Cursor::new(&mut jpeg_data);

                DynamicImage::ImageRgb8(image)
                    .write_to(&mut cursor, image::ImageFormat::Jpeg)
                    .map_err(|e| OurError::App(format!("Failed to encode JPEG: {e}")))?;

                // Clean up camera
                if let Err(e) = camera.stop_stream() {
                    warn!("Failed to stop camera stream: {e}");
                }

                Ok(jpeg_data)
            }
            Err(e) => {
                warn!("Failed to capture frame from camera {hardware_id}: {e}");
                if let Err(stop_err) = camera.stop_stream() {
                    warn!("Failed to stop camera stream after error: {stop_err}");
                }
                Err(OurError::App(format!("Failed to capture frame: {e}")))
            }
        }
    }

    /// Get current status
    fn get_status_internal(&self) -> UsbCameraStatus {
        self.get_status().clone()
    }

    /// Set camera format
    async fn set_camera_format_internal(
        &mut self,
        hardware_id: &str,
        format_info: CameraFormatInfo,
    ) -> OurResult<()> {
        // Update camera info with new format
        {
            let mut status = self.get_status_mut();
            if let Some(camera_info) = status.cameras.get_mut(hardware_id) {
                camera_info.current_format = Some(format_info.clone());
            }
        }

        info!(
            "Set camera format for {hardware_id}: {}x{}@{}",
            format_info.width, format_info.height, format_info.fps
        );
        Ok(())
    }

    /// Set camera brightness control (software-based image adjustment)
    async fn set_brightness_internal(
        &mut self,
        hardware_id: &str,
        brightness: i64,
    ) -> OurResult<()> {
        info!(
            "Setting software brightness adjustment for camera {} to {}",
            hardware_id, brightness
        );

        // Validate camera exists
        let _camera_info = self.get_camera_info(hardware_id)?;

        // Convert brightness from 0-100 range to -100 to +100 offset
        // 50 = no adjustment (0), 0 = darkest (-100), 100 = brightest (+100)
        let brightness_offset = (brightness as f32 - 50.0) * 2.0;

        // Store the brightness adjustment for this camera
        self.brightness_adjustments
            .insert(hardware_id.to_string(), brightness_offset);

        info!(
            "Set software brightness offset for camera {} to {} (original: {})",
            hardware_id, brightness_offset, brightness
        );

        Ok(())
    }

    /// Get camera brightness control (software-based image adjustment)
    async fn get_brightness_internal(&mut self, hardware_id: &str) -> OurResult<i64> {
        info!(
            "Getting software brightness adjustment for camera {}",
            hardware_id
        );

        // Validate camera exists
        let _camera_info = self.get_camera_info(hardware_id)?;

        // Get the stored brightness offset and convert back to 0-100 range
        let brightness_offset = self
            .brightness_adjustments
            .get(hardware_id)
            .copied()
            .unwrap_or(0.0);

        // Convert from -100 to +100 offset back to 0-100 brightness scale
        // 0 offset = 50 brightness, -100 offset = 0 brightness, +100 offset = 100 brightness
        let brightness = ((brightness_offset / 2.0) + 50.0) as i64;

        info!(
            "Current software brightness for camera {}: {} (offset: {})",
            hardware_id, brightness, brightness_offset
        );

        Ok(brightness)
    }
}

/// Start USB camera manager in separate task
pub async fn start_usb_camera_manager() -> OurResult<UsbCameraHandle> {
    let (mut manager, handle) = UsbCameraManager::new()?;

    tokio::spawn(async move {
        if let Err(e) = manager.run().await {
            error!("USB camera manager error: {e}");
        }
    });

    Ok(handle)
}
