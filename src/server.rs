//! Web server implementation using Axum.

use askama::Template;
use askama_web::WebTemplate;
use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::{Html, Json},
    routing::{delete, get, post},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpListener;

use crate::config::Settings;
use crate::{OurError, OurResult};
use tracing::{error, info};

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub settings: Settings,
}

/// Dashboard template
#[derive(Template, WebTemplate)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    title: String,
    subtitle: String,
    machine_name: String,
    host: String,
    port: u16,
    ml_enabled: bool,
    camera_count: u32,
}

/// Config template
#[derive(Template, WebTemplate)]
#[template(path = "config.html")]
struct ConfigTemplate {}

/// Machine status response
#[derive(Serialize)]
struct MachineStatus {
    status: String,
    ready: bool,
    active_jobs: u32,
}

/// Sensor readings response
#[derive(Serialize)]
struct SensorReadings {
    case_ready: bool,
    case_in_view: bool,
    timestamp: u64,
}

/// Camera info response
#[derive(Serialize)]
struct CameraInfo {
    index: usize,
    name: String,
    active: bool,
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
    fn error(message: String) -> ApiResponse<()> {
        ApiResponse {
            success: false,
            data: None,
            message,
        }
    }
}

/// Start the web server
pub async fn start_server(host: String, port: u16, settings: Settings) -> OurResult<()> {
    let state = Arc::new(AppState { settings });

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

    let addr = format!("{}:{}", host, port);
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| OurError::App(format!("Failed to bind to {}: {}", addr, e)))?;

    info!("Web server listening on http://{}", addr);

    axum::serve(listener, app)
        .await
        .map_err(|e| OurError::App(format!("Server error: {}", e)))?;

    Ok(())
}

// Handler implementations
#[axum::debug_handler]
async fn dashboard(
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, (StatusCode, &'static str)> {
    let template = DashboardTemplate {
        title: "Shell Sorter Control Dashboard".to_string(),
        subtitle: "Ammunition shell case sorting machine controller".to_string(),
        machine_name: state.settings.machine_name.clone(),
        host: state.settings.host.clone(),
        port: state.settings.port,
        ml_enabled: state.settings.ml_enabled,
        camera_count: state.settings.camera_count,
    };

    template.render().map(|res| Html::from(res)).map_err(|e| {
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

    template.render().map(|res| Html::from(res)).map_err(|e| {
        error!("Failed to render config template: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Template rendering failed",
        )
    })
}

async fn trigger_next_case(State(_state): State<Arc<AppState>>) -> Json<ApiResponse<()>> {
    // TODO: Implement machine control
    Json(ApiResponse::success(()))
}

async fn machine_status(State(_state): State<Arc<AppState>>) -> Json<ApiResponse<MachineStatus>> {
    let status = MachineStatus {
        status: "Ready".to_string(),
        ready: true,
        active_jobs: 0,
    };
    Json(ApiResponse::success(status))
}

async fn sensor_readings(State(_state): State<Arc<AppState>>) -> Json<ApiResponse<SensorReadings>> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0); // Fallback to 0 if system time is before epoch

    let readings = SensorReadings {
        case_ready: false,
        case_in_view: false,
        timestamp,
    };
    Json(ApiResponse::success(readings))
}

async fn hardware_status(
    State(_state): State<Arc<AppState>>,
) -> Json<ApiResponse<HashMap<String, String>>> {
    let mut status = HashMap::new();
    status.insert("controller".to_string(), "Connected".to_string());
    status.insert(
        "esphome_hostname".to_string(),
        "shell-sorter-controller.local".to_string(),
    );
    Json(ApiResponse::success(status))
}

async fn list_cameras(State(_state): State<Arc<AppState>>) -> Json<ApiResponse<Vec<CameraInfo>>> {
    // TODO: Implement camera detection
    let cameras = vec![CameraInfo {
        index: 0,
        name: "USB Camera 0".to_string(),
        active: false,
        view_type: None,
    }];
    Json(ApiResponse::success(cameras))
}

async fn detect_cameras(State(_state): State<Arc<AppState>>) -> Json<ApiResponse<Vec<CameraInfo>>> {
    // TODO: Implement camera detection
    let cameras = vec![];
    Json(ApiResponse::success(cameras))
}

async fn select_cameras(State(_state): State<Arc<AppState>>) -> Json<ApiResponse<()>> {
    // TODO: Implement camera selection
    Json(ApiResponse::success(()))
}

async fn start_cameras(State(_state): State<Arc<AppState>>) -> Json<ApiResponse<()>> {
    // TODO: Implement camera startup
    Json(ApiResponse::success(()))
}

async fn stop_cameras(State(_state): State<Arc<AppState>>) -> Json<ApiResponse<()>> {
    // TODO: Implement camera shutdown
    Json(ApiResponse::success(()))
}

async fn capture_images(State(_state): State<Arc<AppState>>) -> Json<ApiResponse<()>> {
    // TODO: Implement image capture
    Json(ApiResponse::success(()))
}

async fn camera_stream(
    Path(_index): Path<usize>,
    State(_state): State<Arc<AppState>>,
) -> StatusCode {
    // TODO: Implement camera streaming
    StatusCode::NOT_IMPLEMENTED
}

#[derive(Deserialize)]
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
