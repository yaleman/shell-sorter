//! Web server implementation using Axum.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, Json},
    routing::{get, post, delete},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpListener;

use crate::config::Settings;
use crate::{Error, Result};
use tracing::info;

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub settings: Settings,
}

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

impl<T> ApiResponse<T> {
    fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: "Success".to_string(),
        }
    }

    fn error(message: String) -> ApiResponse<()> {
        ApiResponse {
            success: false,
            data: None,
            message,
        }
    }
}

/// Start the web server
pub async fn start_server(host: String, port: u16, settings: Settings) -> Result<()> {
    let state = Arc::new(AppState { settings });

    let app = Router::new()
        // Static files and main dashboard
        .route("/", get(dashboard))
        
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
        .route("/api/shells/{session_id}/toggle", post(toggle_shell_training))
        
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
    let listener = TcpListener::bind(&addr).await
        .map_err(|e| Error::App(format!("Failed to bind to {}: {}", addr, e)))?;

    info!("Web server listening on http://{}", addr);
    
    axum::serve(listener, app).await
        .map_err(|e| Error::App(format!("Server error: {}", e)))?;

    Ok(())
}

// Handler implementations

async fn dashboard() -> Html<&'static str> {
    Html(
    r#"<!DOCTYPE html>
<html>
<head>
    <title>Shell Sorter Dashboard</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; }
        .section { margin: 20px 0; padding: 20px; border: 1px solid #ddd; border-radius: 5px; }
        button { padding: 10px 20px; margin: 5px; background: #007cba; color: white; border: none; border-radius: 3px; cursor: pointer; }
        button:hover { background: #005a85; }
    </style>
</head>
<body>
    <h1>Shell Sorter Control Dashboard</h1>
    
    <div class="section">
        <h2>Machine Control</h2>
        <button onclick="triggerNextCase()">Next Case</button>
        <button onclick="getMachineStatus()">Machine Status</button>
        <button onclick="getSensorReadings()">Sensor Readings</button>
    </div>
    
    <div class="section">
        <h2>Camera Operations</h2>
        <button onclick="detectCameras()">Detect Cameras</button>
        <button onclick="listCameras()">List Cameras</button>
        <button onclick="captureImages()">Capture Images</button>
    </div>
    
    <div class="section">
        <h2>Machine Learning</h2>
        <button onclick="listCaseTypes()">List Case Types</button>
        <button onclick="generateComposites()">Generate Composites</button>
        <button onclick="trainModel()">Train Model</button>
    </div>
    
    <div id="output" style="margin-top: 20px; padding: 10px; background: #f5f5f5; border-radius: 3px; min-height: 100px; white-space: pre-wrap;"></div>
    
    <script>
        async function apiCall(method, url, body = null) {
            try {
                const response = await fetch(url, {
                    method,
                    headers: body ? {'Content-Type': 'application/json'} : {},
                    body: body ? JSON.stringify(body) : null
                });
                const data = await response.json();
                document.getElementById('output').textContent = JSON.stringify(data, null, 2);
            } catch (error) {
                document.getElementById('output').textContent = 'Error: ' + error.message;
            }
        }
        
        function triggerNextCase() { apiCall('POST', '/api/machine/next-case'); }
        function getMachineStatus() { apiCall('GET', '/api/machine/status'); }
        function getSensorReadings() { apiCall('GET', '/api/machine/sensors'); }
        function detectCameras() { apiCall('GET', '/api/cameras/detect'); }
        function listCameras() { apiCall('GET', '/api/cameras'); }
        function captureImages() { apiCall('POST', '/api/cameras/capture'); }
        function listCaseTypes() { apiCall('GET', '/api/case-types'); }
        function generateComposites() { apiCall('POST', '/api/ml/generate-composites'); }
        function trainModel() { apiCall('POST', '/api/train-model'); }
    </script>
</body>
</html>"#
    )
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
    let readings = SensorReadings {
        case_ready: false,
        case_in_view: false,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };
    Json(ApiResponse::success(readings))
}

async fn hardware_status(State(_state): State<Arc<AppState>>) -> Json<ApiResponse<HashMap<String, String>>> {
    let mut status = HashMap::new();
    status.insert("controller".to_string(), "Connected".to_string());
    status.insert("esphome_hostname".to_string(), "shell-sorter-controller.local".to_string());
    Json(ApiResponse::success(status))
}

async fn list_cameras(State(_state): State<Arc<AppState>>) -> Json<ApiResponse<Vec<CameraInfo>>> {
    // TODO: Implement camera detection
    let cameras = vec![
        CameraInfo {
            index: 0,
            name: "USB Camera 0".to_string(),
            active: false,
            view_type: None,
        }
    ];
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

async fn camera_stream(Path(_index): Path<usize>, State(_state): State<Arc<AppState>>) -> StatusCode {
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

async fn list_shells(State(_state): State<Arc<AppState>>) -> Json<ApiResponse<Vec<HashMap<String, String>>>> {
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

async fn ml_list_shells(State(_state): State<Arc<AppState>>) -> Json<ApiResponse<Vec<HashMap<String, String>>>> {
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

async fn get_config(State(state): State<Arc<AppState>>) -> Json<ApiResponse<Settings>> {
    Json(ApiResponse::success(state.settings.clone()))
}

async fn save_config(State(_state): State<Arc<AppState>>) -> Json<ApiResponse<()>> {
    // TODO: Implement config saving
    Json(ApiResponse::success(()))
}

async fn delete_camera_config(
    Path(_index): Path<usize>,
    State(_state): State<Arc<AppState>>,
) -> Json<ApiResponse<()>> {
    // TODO: Implement camera config deletion
    Json(ApiResponse::success(()))
}

async fn clear_camera_configs(State(_state): State<Arc<AppState>>) -> Json<ApiResponse<()>> {
    // TODO: Implement camera config clearing
    Json(ApiResponse::success(()))
}

async fn reset_config(State(_state): State<Arc<AppState>>) -> Json<ApiResponse<()>> {
    // TODO: Implement config reset
    Json(ApiResponse::success(()))
}