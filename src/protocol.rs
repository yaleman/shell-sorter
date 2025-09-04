use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::{config::Settings, constants::USB_DEVICE_PREFIX_WITH_COLON};

#[derive(Debug, Clone, Deserialize)]

pub enum CameraType {
    EspHome(String),
    Usb(String),
}

impl Serialize for CameraType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            CameraType::EspHome(name) => serializer.serialize_str(name),
            CameraType::Usb(name) => {
                serializer.serialize_str(&format!("{USB_DEVICE_PREFIX_WITH_COLON}{name}"))
            }
        }
    }
}

impl From<String> for CameraType {
    fn from(camera_id: String) -> Self {
        if camera_id.starts_with(USB_DEVICE_PREFIX_WITH_COLON) {
            CameraType::Usb(
                camera_id
                    .strip_prefix(USB_DEVICE_PREFIX_WITH_COLON)
                    .unwrap_or(&camera_id)
                    .to_string(),
            )
        } else {
            CameraType::EspHome(camera_id)
        }
    }
}

#[test]
fn test_camera_tyoe() {
    let camera_id = "usb:1234".to_string();
    let camera_type: CameraType = camera_id.into();
    assert!(matches!(camera_type, CameraType::Usb(_)));
    assert_eq!(
        "\"usb:1234\"".to_string(),
        serde_json::to_string(&camera_type).expect("Failed to serialize")
    );

    let camera_id = "camera1".to_string();
    let camera_type: CameraType = camera_id.into();
    assert!(matches!(camera_type, CameraType::EspHome(_)));
    assert_eq!(
        "\"camera1\"".to_string(),
        serde_json::to_string(&camera_type).expect("Failed to serialize")
    );
}

#[derive(Debug, Clone)]
pub enum GlobalMessage {
    Shutdown,
    NextCase,
    DetectCameras,
    GetSensors(Arc<tokio::sync::oneshot::Sender<crate::controller_monitor::SensorReadings>>),
    ControllerStatus {
        responder: Arc<tokio::sync::oneshot::Sender<crate::controller_monitor::ControllerStatus>>,
    },
    MachineStatus(Arc<tokio::sync::oneshot::Sender<crate::controller_monitor::MachineStatus>>),
    SelectCameras(Vec<CameraType>),
    StartCameras(Vec<CameraType>),
    StopCameras,
    SetCameraBrightness {
        camera_id: CameraType,
        brightness: i64,
    },
    NewConfig(Settings),
}

/// Generic API response
#[derive(Serialize)]
pub(crate) struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: String,
}

impl<T> ApiResponse<T> {
    pub(crate) fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: "Success".to_string(),
        }
    }
    #[allow(dead_code)]
    pub(crate) fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            message,
        }
    }
}
