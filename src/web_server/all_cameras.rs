use super::prelude::*;

#[derive(Serialize, Deserialize)]
pub struct BrightnessRequest {
    brightness: i64,
}

pub(crate) async fn get_camera_brightness(
    Path(camera_id): Path<String>,
    State(_state): State<Arc<AppState>>,
) -> Json<ApiResponse<BrightnessRequest>> {
    info!("Getting brightness for camera: {}", camera_id);

    // // Determine camera type and route to appropriate manager
    // if camera_id.starts_with(USB_DEVICE_PREFIX_WITH_COLON) {
    //     match state.usb_camera_manager.get_brightness(camera_id).await {
    //         Ok(brightness) => {
    //             info!("Current brightness for USB camera: {}", brightness);
    //             Json(ApiResponse::success(BrightnessResponse { brightness }))
    //         }
    //         Err(e) => {
    //             error!("Failed to get USB camera brightness: {}", e);
    //             Json(ApiResponse::<BrightnessResponse>::error(format!(
    //                 "Failed to get camera brightness: {e}"
    //             )))
    //         }
    //     }
    // } else {
    //     // ESPHome cameras don't support brightness control
    Json(ApiResponse::<BrightnessRequest>::error(
        "ESPHome cameras do not support brightness control".to_string(),
    ))
    // }
}

pub(crate) async fn set_camera_brightness(
    Path(camera_id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<BrightnessRequest>,
) -> Json<ApiResponse<()>> {
    let camera_id: CameraType = camera_id.into();

    info!(
        "Setting brightness for camera: {camera_id:?} to {}",
        payload.brightness
    );

    // Validate brightness range (typically 0-100 or similar)
    if payload.brightness < 0 || payload.brightness > 255 {
        return Json(ApiResponse::<()>::error(
            "Brightness must be between 0 and 255".to_string(),
        ));
    }

    match state.global_tx.send(GlobalMessage::SetCameraBrightness {
        camera_id: camera_id.clone(),
        brightness: payload.brightness,
    }) {
        Ok(_) => {
            info!("Brightness set successfully for camera: {camera_id:?}");
            Json(ApiResponse::success(()))
        }
        Err(err) => {
            error!("Failed to set brightness for camera {camera_id:?}: {err}");
            Json(ApiResponse::<()>::error(format!(
                "Failed to set brightness for camera {camera_id:?}: {err}"
            )))
        }
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) enum CameraView {
    Bottom,
    Side,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct ViewTypeRequest {
    view_type: CameraView,
}

pub(crate) async fn set_camera_view_type(
    Path(_index): Path<usize>,
    State(_state): State<Arc<AppState>>,
    Json(_payload): Json<ViewTypeRequest>,
) -> Json<ApiResponse<()>> {
    // TODO: Implement view type setting
    Json(ApiResponse::success(()))
}
