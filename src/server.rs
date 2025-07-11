//! Web server implementation using Axum.

use askama::Template;
use askama_web::WebTemplate;
use axum::{
    Router,
    extract::{Json as ExtractJson, Path, State},
    http::StatusCode,
    response::{Html, Json},
    routing::{delete, get, post},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::{collections::HashMap, num::NonZeroU16};
use tokio::net::TcpListener;

use crate::camera_manager::CameraHandle;
use crate::config::Settings;
use crate::controller_monitor::{ControllerCommand, ControllerHandle, ControllerResponse};
use crate::{OurError, OurResult};
use tracing::{error, info};

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub settings: Settings,
    pub controller: ControllerHandle,
    pub camera_manager: CameraHandle,
}

/// Dashboard template
#[derive(Template, WebTemplate)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    machine_name: String,
    host: String,
    port: u16,
}

/// Config template
#[derive(Template, WebTemplate)]
#[template(path = "config.html")]
struct ConfigTemplate {}

/// Camera info response
#[derive(Serialize)]
struct CameraInfo {
    id: String,
    name: String,
    hostname: String,
    online: bool,
    view_type: Option<String>,
}

/// Generic API response
#[derive(Serialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    message: String,
}

/// Configuration data for API responses
#[derive(Serialize, Deserialize)]
struct ConfigData {
    auto_start_cameras: bool,
    auto_detect_cameras: bool,
    esphome_hostname: String,
    network_camera_hostnames: Vec<String>,
}

impl<T> ApiResponse<T> {
    fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: "Success".to_string(),
        }
    }
    #[allow(dead_code)]
    fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            message,
        }
    }
}

/// Start the web server
pub async fn start_server(
    host: String,
    port: NonZeroU16,
    settings: Settings,
    controller: ControllerHandle,
    camera_manager: CameraHandle,
) -> OurResult<()> {
    let state = Arc::new(AppState {
        settings,
        controller,
        camera_manager,
    });

    let app = Router::new()
        // Static files and main dashboard
        .route("/", get(dashboard))
        .route("/config", get(config_page))
        // Machine control API
        .route("/api/machine/next-case", post(trigger_next_case))
        .route("/api/machine/status", get(machine_status))
        .route("/api/machine/sensors", get(sensor_readings))
        .route("/api/machine/hardware-status", get(hardware_status))
        // Camera management API
        .route("/api/cameras", get(list_cameras))
        .route("/api/cameras/detect", get(detect_cameras))
        .route("/api/cameras/select", post(select_cameras))
        .route("/api/cameras/start-selected", post(start_cameras))
        .route("/api/cameras/stop-all", post(stop_cameras))
        .route("/api/cameras/capture", post(capture_images))
        .route("/api/cameras/{index}/stream", get(camera_stream))
        .route("/api/cameras/{index}/view-type", post(set_camera_view_type))
        .route("/api/cameras/{index}/region", post(set_camera_region))
        .route("/api/cameras/{index}/region", delete(clear_camera_region))
        // Data management API
        .route("/api/shells", get(list_shells))
        .route("/api/shells/save", post(save_shell_data))
        .route(
            "/api/shells/{session_id}/toggle",
            post(toggle_shell_training),
        )
        // ML API
        .route("/api/ml/shells", get(ml_list_shells))
        .route("/api/ml/generate-composites", post(generate_composites))
        .route("/api/case-types", get(list_case_types))
        .route("/api/case-types", post(create_case_type))
        .route("/api/train-model", post(train_model))
        // Configuration API
        .route("/api/config", get(get_config))
        .route("/api/config", post(save_config))
        .route("/api/config/cameras/{index}", delete(delete_camera_config))
        .route("/api/config/cameras", delete(clear_camera_configs))
        .route("/api/config/reset", post(reset_config))
        .with_state(state);

    let addr = format!("{host}:{port}");
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| OurError::App(format!("Failed to bind to {addr}: {e}")))?;

    info!("Web server listening on http://{}", addr);

    axum::serve(listener, app)
        .await
        .map_err(|e| OurError::App(format!("Server error: {e}")))?;

    Ok(())
}

// Handler implementations
#[axum::debug_handler]
async fn dashboard(
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, (StatusCode, &'static str)> {
    let template = DashboardTemplate {
        machine_name: state.settings.machine_name.clone(),
        host: state.settings.host.clone(),
        port: state.settings.port,
    };

    template.render().map(Html::from).map_err(|e| {
        error!("Failed to render dashboard template: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Template rendering failed",
        )
    })
}

#[axum::debug_handler]
async fn config_page(
    State(_state): State<Arc<AppState>>,
) -> Result<Html<String>, (StatusCode, &'static str)> {
    let template = ConfigTemplate {};

    template.render().map(Html::from).map_err(|e| {
        error!("Failed to render config template: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Template rendering failed",
        )
    })
}

async fn trigger_next_case(State(state): State<Arc<AppState>>) -> Json<ApiResponse<()>> {
    match state
        .controller
        .send_command(ControllerCommand::NextCase)
        .await
    {
        Ok(_) => Json(ApiResponse::success(())),
        Err(e) => {
            error!("Failed to trigger next case: {e}");
            Json(ApiResponse::<()>::error(format!(
                "Failed to trigger next case: {e}",
            )))
        }
    }
}

async fn machine_status(
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<crate::controller_monitor::MachineStatus>> {
    match state
        .controller
        .send_command(ControllerCommand::GetStatus)
        .await
    {
        Ok(ControllerResponse::StatusData(status)) => Json(ApiResponse::success(status)),
        Ok(_) => {
            error!("Unexpected response type for machine status");
            let fallback_status = crate::controller_monitor::MachineStatus {
                status: "Error".to_string(),
                ready: false,
                active_jobs: 0,
                last_update: chrono::Utc::now(),
            };
            Json(ApiResponse::success(fallback_status))
        }
        Err(e) => {
            error!("Failed to get machine status: {}", e);
            let fallback_status = crate::controller_monitor::MachineStatus {
                status: "Offline".to_string(),
                ready: false,
                active_jobs: 0,
                last_update: chrono::Utc::now(),
            };
            Json(ApiResponse::success(fallback_status))
        }
    }
}

async fn sensor_readings(
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<crate::controller_monitor::SensorReadings>> {
    match state
        .controller
        .send_command(ControllerCommand::GetSensors)
        .await
    {
        Ok(ControllerResponse::SensorData(readings)) => Json(ApiResponse::success(readings)),
        Ok(_) => {
            error!("Unexpected response type for sensor readings");
            let fallback_readings = crate::controller_monitor::SensorReadings {
                case_ready: false,
                case_in_view: false,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            };
            Json(ApiResponse::success(fallback_readings))
        }
        Err(e) => {
            error!("Failed to get sensor readings: {}", e);
            let fallback_readings = crate::controller_monitor::SensorReadings {
                case_ready: false,
                case_in_view: false,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            };
            Json(ApiResponse::success(fallback_readings))
        }
    }
}

async fn hardware_status(
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<HashMap<String, String>>> {
    match state
        .controller
        .send_command(ControllerCommand::GetHardwareStatus)
        .await
    {
        Ok(ControllerResponse::HardwareData(status)) => Json(ApiResponse::success(status)),
        Ok(_) => {
            error!("Unexpected response type for hardware status");
            let mut fallback_status = HashMap::new();
            fallback_status.insert("controller".to_string(), "Error".to_string());
            fallback_status.insert(
                "esphome_hostname".to_string(),
                state.settings.esphome_hostname.clone(),
            );
            Json(ApiResponse::success(fallback_status))
        }
        Err(e) => {
            error!("Failed to get hardware status: {}", e);
            let mut fallback_status = HashMap::new();
            fallback_status.insert("controller".to_string(), "Disconnected".to_string());
            fallback_status.insert(
                "esphome_hostname".to_string(),
                state.settings.esphome_hostname.clone(),
            );
            Json(ApiResponse::success(fallback_status))
        }
    }
}

async fn list_cameras(State(state): State<Arc<AppState>>) -> Json<ApiResponse<Vec<CameraInfo>>> {
    match state.camera_manager.list_cameras().await {
        Ok(cameras) => {
            let camera_infos: Vec<CameraInfo> = cameras
                .into_iter()
                .map(|cam| CameraInfo {
                    id: cam.id,
                    name: cam.name,
                    hostname: cam.hostname,
                    online: cam.online,
                    view_type: None,
                })
                .collect();
            Json(ApiResponse::success(camera_infos))
        }
        Err(e) => {
            error!("Failed to list cameras: {}", e);
            Json(ApiResponse::<Vec<CameraInfo>>::error(format!(
                "Failed to list cameras: {}",
                e
            )))
        }
    }
}

async fn detect_cameras(State(state): State<Arc<AppState>>) -> Json<ApiResponse<Vec<CameraInfo>>> {
    match state.camera_manager.detect_cameras().await {
        Ok(cameras) => {
            let camera_infos: Vec<CameraInfo> = cameras
                .into_iter()
                .map(|cam| CameraInfo {
                    id: cam.id,
                    name: cam.name,
                    hostname: cam.hostname,
                    online: cam.online,
                    view_type: None,
                })
                .collect();
            Json(ApiResponse::success(camera_infos))
        }
        Err(e) => {
            error!("Failed to detect cameras: {}", e);
            Json(ApiResponse::<Vec<CameraInfo>>::error(format!(
                "Failed to detect cameras: {}",
                e
            )))
        }
    }
}

async fn select_cameras(
    State(state): State<Arc<AppState>>,
    ExtractJson(payload): ExtractJson<SelectCamerasRequest>,
) -> Json<ApiResponse<()>> {
    match state
        .camera_manager
        .select_cameras(payload.camera_ids)
        .await
    {
        Ok(()) => Json(ApiResponse::success(())),
        Err(e) => {
            error!("Failed to select cameras: {}", e);
            Json(ApiResponse::<()>::error(format!(
                "Failed to select cameras: {}",
                e
            )))
        }
    }
}

async fn start_cameras(State(state): State<Arc<AppState>>) -> Json<ApiResponse<()>> {
    match state.camera_manager.start_streaming().await {
        Ok(()) => Json(ApiResponse::success(())),
        Err(e) => {
            error!("Failed to start cameras: {}", e);
            Json(ApiResponse::<()>::error(format!(
                "Failed to start cameras: {}",
                e
            )))
        }
    }
}

async fn stop_cameras(State(state): State<Arc<AppState>>) -> Json<ApiResponse<()>> {
    match state.camera_manager.stop_streaming().await {
        Ok(()) => Json(ApiResponse::success(())),
        Err(e) => {
            error!("Failed to stop cameras: {}", e);
            Json(ApiResponse::<()>::error(format!(
                "Failed to stop cameras: {}",
                e
            )))
        }
    }
}

async fn capture_images(
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<HashMap<String, String>>> {
    let status = state.camera_manager.get_status();
    let mut results = HashMap::new();

    for camera_id in &status.selected_cameras {
        match state.camera_manager.capture_image(camera_id.clone()).await {
            Ok(image_data) => {
                results.insert(
                    camera_id.clone(),
                    format!("Captured {} bytes", image_data.len()),
                );
            }
            Err(e) => {
                error!("Failed to capture from camera {}: {}", camera_id, e);
                results.insert(camera_id.clone(), format!("Error: {}", e));
            }
        }
    }

    Json(ApiResponse::success(results))
}

async fn camera_stream(
    Path(_index): Path<usize>,
    State(_state): State<Arc<AppState>>,
) -> StatusCode {
    // TODO: Implement camera streaming
    StatusCode::NOT_IMPLEMENTED
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct ViewTypeRequest {
    view_type: String,
}

async fn set_camera_view_type(
    Path(_index): Path<usize>,
    State(_state): State<Arc<AppState>>,
    Json(_payload): Json<ViewTypeRequest>,
) -> Json<ApiResponse<()>> {
    // TODO: Implement view type setting
    Json(ApiResponse::success(()))
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct RegionRequest {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

async fn set_camera_region(
    Path(_index): Path<usize>,
    State(_state): State<Arc<AppState>>,
    Json(_payload): Json<RegionRequest>,
) -> Json<ApiResponse<()>> {
    // TODO: Implement region setting
    Json(ApiResponse::success(()))
}

async fn clear_camera_region(
    Path(_index): Path<usize>,
    State(_state): State<Arc<AppState>>,
) -> Json<ApiResponse<()>> {
    // TODO: Implement region clearing
    Json(ApiResponse::success(()))
}

async fn list_shells(
    State(_state): State<Arc<AppState>>,
) -> Json<ApiResponse<Vec<HashMap<String, String>>>> {
    // TODO: Implement shell listing
    let shells = vec![];
    Json(ApiResponse::success(shells))
}

async fn save_shell_data(State(_state): State<Arc<AppState>>) -> Json<ApiResponse<()>> {
    // TODO: Implement shell data saving
    Json(ApiResponse::success(()))
}

async fn toggle_shell_training(
    Path(_session_id): Path<String>,
    State(_state): State<Arc<AppState>>,
) -> Json<ApiResponse<()>> {
    // TODO: Implement training toggle
    Json(ApiResponse::success(()))
}

async fn ml_list_shells(
    State(_state): State<Arc<AppState>>,
) -> Json<ApiResponse<Vec<HashMap<String, String>>>> {
    // TODO: Implement ML shell listing
    let shells = vec![];
    Json(ApiResponse::success(shells))
}

async fn generate_composites(State(_state): State<Arc<AppState>>) -> Json<ApiResponse<()>> {
    // TODO: Implement composite generation
    Json(ApiResponse::success(()))
}

async fn list_case_types(State(_state): State<Arc<AppState>>) -> Json<ApiResponse<Vec<String>>> {
    let case_types = vec![
        "9mm".to_string(),
        "40sw".to_string(),
        "45acp".to_string(),
        "223rem".to_string(),
        "308win".to_string(),
        "3006spr".to_string(),
        "38special".to_string(),
        "357mag".to_string(),
    ];
    Json(ApiResponse::success(case_types))
}

#[derive(Deserialize)]
struct CreateCaseTypeRequest {
    name: String,
    designation: Option<String>,
}

#[derive(Deserialize)]
struct SelectCamerasRequest {
    camera_ids: Vec<String>,
}

async fn create_case_type(
    State(_state): State<Arc<AppState>>,
    Json(_payload): Json<CreateCaseTypeRequest>,
) -> Json<ApiResponse<()>> {
    // TODO: Implement case type creation
    Json(ApiResponse::success(()))
}

async fn train_model(State(_state): State<Arc<AppState>>) -> Json<ApiResponse<()>> {
    // TODO: Implement model training
    Json(ApiResponse::success(()))
}

async fn get_config(State(state): State<Arc<AppState>>) -> Json<ApiResponse<ConfigData>> {
    let config_data = ConfigData {
        auto_start_cameras: state.settings.auto_start_esp32_cameras,
        auto_detect_cameras: state.settings.auto_detect_cameras,
        esphome_hostname: state.settings.esphome_hostname.clone(),
        network_camera_hostnames: state.settings.network_camera_hostnames.clone(),
    };
    Json(ApiResponse::success(config_data))
}

async fn save_config(
    State(_state): State<Arc<AppState>>,
    Json(config): Json<ConfigData>,
) -> Json<ApiResponse<()>> {
    // For now, just acknowledge the save request
    // In a full implementation, this would save to the configuration file
    info!(
        "Config save requested: auto_start={}, auto_detect={}, esphome={}, cameras={:?}",
        config.auto_start_cameras,
        config.auto_detect_cameras,
        config.esphome_hostname,
        config.network_camera_hostnames
    );
    Json(ApiResponse::success(()))
}

async fn delete_camera_config(
    Path(index): Path<usize>,
    State(_state): State<Arc<AppState>>,
) -> Json<ApiResponse<()>> {
    // For now, just acknowledge the delete request
    // In a full implementation, this would remove the camera from the configuration
    info!("Camera {} delete requested", index);
    Json(ApiResponse::success(()))
}

async fn clear_camera_configs(State(_state): State<Arc<AppState>>) -> Json<ApiResponse<()>> {
    // For now, just acknowledge the clear request
    // In a full implementation, this would remove all cameras from configuration
    info!("Clear all cameras requested");
    Json(ApiResponse::success(()))
}

async fn reset_config(State(_state): State<Arc<AppState>>) -> Json<ApiResponse<()>> {
    // For now, just acknowledge the reset request
    // In a full implementation, this would reset configuration to defaults
    info!("Config reset to defaults requested");
    Json(ApiResponse::success(()))
}
