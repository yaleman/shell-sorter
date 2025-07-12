//! Web server implementation using Axum.

use askama::Template;
use askama_web::WebTemplate;
use axum::{
    Router,
    body::Body,
    extract::{Json as ExtractJson, Path, Request, State},
    http::{HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{Html, Json, Response},
    routing::{delete, get, post},
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::{collections::HashMap, num::NonZeroU16};
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

use crate::camera_manager::CameraHandle;
use crate::config::Settings;
use crate::controller_monitor::{ControllerCommand, ControllerHandle, ControllerResponse};
use crate::usb_camera_controller::UsbCameraHandle;
use crate::{OurError, OurResult};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{error, info};

/// Middleware to add no-cache headers to prevent browser caching
async fn no_cache_middleware(request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();
    let mut response = next.run(request).await;

    // Get the headers map mutably
    let headers = response.headers_mut();

    // Add no-cache headers for all responses
    headers.insert(
        "Cache-Control",
        HeaderValue::from_static("no-cache, no-store, must-revalidate, max-age=0"),
    );
    headers.insert("Pragma", HeaderValue::from_static("no-cache"));
    headers.insert("Expires", HeaderValue::from_static("0"));

    // Generate ETag with current timestamp
    if let Ok(timestamp) = SystemTime::now().duration_since(UNIX_EPOCH) {
        let etag_value = format!("\"{}\"", timestamp.as_secs());
        if let Ok(etag_header) = HeaderValue::from_str(&etag_value) {
            headers.insert("ETag", etag_header);
        }
    }

    // Additional headers for static files (JS, CSS, HTML)
    if path.ends_with(".js")
        || path.ends_with(".css")
        || path.ends_with(".html")
        || path.ends_with(".htm")
    {
        headers.insert(
            "Cache-Control",
            HeaderValue::from_static("no-cache, no-store, must-revalidate, max-age=0, private"),
        );
    }

    response
}

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub settings: Settings,
    pub controller: ControllerHandle,
    pub camera_manager: CameraHandle,
    pub usb_camera_manager: UsbCameraHandle,
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

#[derive(Deserialize, Serialize)]
enum CameraType {
    #[serde(rename = "esphome")]
    EspHome,
    #[serde(rename = "usb")]
    Usb,
}

/// Camera info response
#[derive(Serialize)]
struct CameraInfo {
    id: String,
    name: String,
    hostname: Option<String>,
    online: bool,
    view_type: Option<String>,
    camera_type: CameraType,
    index: Option<u32>, // For USB cameras
    vendor_id: Option<String>,
    product_id: Option<String>,
    serial_number: Option<String>,
    is_active: bool,
    is_selected: bool,
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

/// Status data for frontend status updates
#[derive(Serialize)]
struct StatusData {
    status: String,
    total_sorted: u32,
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

/// Create a test router for integration testing
pub fn create_test_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/status", get(status))
        .route("/api/cameras", get(list_cameras))
        .route("/api/cameras/detect", get(detect_cameras))
        .route("/api/machine/hardware-status", get(hardware_status))
        .route("/api/case-types", get(list_case_types))
        .route("/", get(dashboard))
        .with_state(state)
}

/// Start the web server
pub async fn start_server(
    host: String,
    port: NonZeroU16,
    settings: Settings,
    controller: ControllerHandle,
    camera_manager: CameraHandle,
    usb_camera_manager: UsbCameraHandle,
) -> OurResult<()> {
    let state = Arc::new(AppState {
        settings,
        controller,
        camera_manager,
        usb_camera_manager,
    });

    let app = Router::new()
        // Static files
        .nest_service("/static", ServeDir::new("shell_sorter/static"))
        // Main dashboard and pages
        .route("/", get(dashboard))
        .route("/config", get(config_page))
        // Machine control API
        .route("/api/status", get(status))
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
        .route("/api/cameras/{camera_id}/stream", get(camera_stream))
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
        .layer(middleware::from_fn(no_cache_middleware))
        .with_state(state);

    let addr = format!("{host}:{port}");
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| OurError::App(format!("Failed to bind to {addr}: {e}")))?;

    info!("Web server listening on http://{addr}");

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
        error!("Failed to render dashboard template: {e}");
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
        error!("Failed to render config template: {e}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Template rendering failed",
        )
    })
}

async fn status(State(state): State<Arc<AppState>>) -> Json<StatusData> {
    // Get machine status for the overall system status
    let machine_status = match state
        .controller
        .send_command(ControllerCommand::GetStatus)
        .await
    {
        Ok(ControllerResponse::StatusData(status)) => status.status,
        Ok(_) => {
            error!("Unexpected response type for machine status");
            "Error".to_string()
        }
        Err(e) => {
            error!("Failed to get machine status: {e}");
            "Offline".to_string()
        }
    };

    // TODO: Implement actual sorted count tracking
    // For now, return a placeholder value
    let total_sorted = 0;

    Json(StatusData {
        status: machine_status,
        total_sorted,
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
            error!("Failed to get machine status: {e}");
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
            error!("Failed to get sensor readings: {e}");
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
            error!("Failed to get hardware status: {e}");
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
    let mut all_cameras = Vec::new();

    // Get ESPHome camera status
    let esphome_status = state.camera_manager.get_status().await.unwrap_or_default();

    // Get ESPHome cameras
    match state.camera_manager.list_cameras().await {
        Ok(cameras) => {
            let esphome_cameras: Vec<CameraInfo> = cameras
                .into_iter()
                .map(|cam| {
                    let is_selected = esphome_status.selected_cameras.contains(&cam.id);
                    let is_active = is_selected && esphome_status.streaming;

                    CameraInfo {
                        id: cam.id,
                        name: cam.name,
                        hostname: Some(cam.hostname),
                        online: cam.online,
                        view_type: None,
                        camera_type: CameraType::EspHome,
                        index: None,
                        vendor_id: None,
                        product_id: None,
                        serial_number: None,
                        is_active,
                        is_selected,
                    }
                })
                .collect();
            all_cameras.extend(esphome_cameras);
        }
        Err(e) => {
            error!("Failed to list ESPHome cameras: {e}");
        }
    }

    // Get USB camera status
    let usb_status = state
        .usb_camera_manager
        .get_status()
        .await
        .unwrap_or_default();

    // Get USB cameras
    match state.usb_camera_manager.list_cameras().await {
        Ok(cameras) => {
            let usb_cameras: Vec<CameraInfo> = cameras
                .into_iter()
                .map(|cam| {
                    let is_selected = usb_status.selected_cameras.contains(&cam.hardware_id);
                    let is_active = is_selected && usb_status.streaming;

                    CameraInfo {
                        id: cam.hardware_id.clone(),
                        name: cam.name,
                        hostname: None,
                        online: cam.connected,
                        view_type: None,
                        camera_type: CameraType::Usb,
                        index: Some(cam.index),
                        vendor_id: cam.vendor_id,
                        product_id: cam.product_id,
                        serial_number: cam.serial_number,
                        is_active,
                        is_selected,
                    }
                })
                .collect();
            all_cameras.extend(usb_cameras);
        }
        Err(e) => {
            error!("Failed to list USB cameras: {e}");
        }
    }

    Json(ApiResponse::success(all_cameras))
}

async fn detect_cameras(State(state): State<Arc<AppState>>) -> Json<ApiResponse<Vec<CameraInfo>>> {
    let mut all_cameras = Vec::new();
    let mut errors = Vec::new();

    // Detect ESPHome cameras
    match state.camera_manager.detect_cameras().await {
        Ok(cameras) => {
            let esphome_cameras: Vec<CameraInfo> = cameras
                .into_iter()
                .map(|cam| CameraInfo {
                    id: cam.id,
                    name: cam.name,
                    hostname: Some(cam.hostname),
                    online: cam.online,
                    view_type: None,
                    camera_type: CameraType::EspHome,
                    index: None,
                    vendor_id: None,
                    product_id: None,
                    serial_number: None,
                    is_active: false,
                    is_selected: false,
                })
                .collect();
            all_cameras.extend(esphome_cameras);
        }
        Err(e) => {
            error!("Failed to detect ESPHome cameras: {e}");
            errors.push(format!("ESPHome: {e}"));
        }
    }

    // Detect USB cameras
    match state.usb_camera_manager.detect_cameras().await {
        Ok(cameras) => {
            let usb_cameras: Vec<CameraInfo> = cameras
                .into_iter()
                .map(|cam| CameraInfo {
                    id: cam.hardware_id.clone(),
                    name: cam.name,
                    hostname: None,
                    online: cam.connected,
                    view_type: None,
                    camera_type: CameraType::Usb,
                    index: Some(cam.index),
                    vendor_id: cam.vendor_id,
                    product_id: cam.product_id,
                    serial_number: cam.serial_number,
                    is_active: false,
                    is_selected: false,
                })
                .collect();
            all_cameras.extend(usb_cameras);
        }
        Err(e) => {
            error!("Failed to detect USB cameras: {e}");
            errors.push(format!("USB: {e}"));
        }
    }

    if all_cameras.is_empty() && !errors.is_empty() {
        Json(ApiResponse::<Vec<CameraInfo>>::error(format!(
            "Failed to detect cameras: {}",
            errors.join(", ")
        )))
    } else {
        Json(ApiResponse::success(all_cameras))
    }
}

async fn select_cameras(
    State(state): State<Arc<AppState>>,
    ExtractJson(payload): ExtractJson<SelectCamerasRequest>,
) -> Json<ApiResponse<()>> {
    // Separate camera IDs by type
    let mut esphome_cameras = Vec::new();
    let mut usb_cameras = Vec::new();

    for camera_id in payload.camera_ids {
        if camera_id.starts_with("usb:") {
            usb_cameras.push(camera_id);
        } else {
            esphome_cameras.push(camera_id);
        }
    }

    // Select ESPHome cameras if any
    if !esphome_cameras.is_empty() {
        if let Err(e) = state.camera_manager.select_cameras(esphome_cameras).await {
            error!("Failed to select ESPHome cameras: {e}");
            return Json(ApiResponse::<()>::error(format!(
                "Failed to select ESPHome cameras: {e}"
            )));
        }
    }

    // Select USB cameras if any
    if !usb_cameras.is_empty() {
        if let Err(e) = state.usb_camera_manager.select_cameras(usb_cameras).await {
            error!("Failed to select USB cameras: {e}");
            return Json(ApiResponse::<()>::error(format!(
                "Failed to select USB cameras: {e}"
            )));
        }
    }

    Json(ApiResponse::success(()))
}

async fn start_cameras(State(state): State<Arc<AppState>>) -> Json<ApiResponse<()>> {
    let mut errors = Vec::new();
    let mut started_any = false;

    // Try to start ESPHome cameras
    match state.camera_manager.start_streaming().await {
        Ok(()) => {
            started_any = true;
        }
        Err(e) => {
            error!("Failed to start ESPHome cameras: {e}");
            errors.push(format!("ESPHome: {e}"));
        }
    }

    // Try to start USB cameras
    match state.usb_camera_manager.start_streaming().await {
        Ok(()) => {
            started_any = true;
        }
        Err(e) => {
            error!("Failed to start USB cameras: {e}");
            errors.push(format!("USB: {e}"));
        }
    }

    if started_any {
        if errors.is_empty() {
            Json(ApiResponse::success(()))
        } else {
            // Some cameras started but others failed
            Json(ApiResponse::success(()))
        }
    } else {
        // No cameras started
        Json(ApiResponse::<()>::error(format!(
            "Failed to start cameras: {}",
            errors.join(", ")
        )))
    }
}

async fn stop_cameras(State(state): State<Arc<AppState>>) -> Json<ApiResponse<()>> {
    let mut errors = Vec::new();
    let mut stopped_any = false;

    // Try to stop ESPHome cameras
    match state.camera_manager.stop_streaming().await {
        Ok(()) => {
            stopped_any = true;
        }
        Err(e) => {
            error!("Failed to stop ESPHome cameras: {e}");
            errors.push(format!("ESPHome: {e}"));
        }
    }

    // Try to stop USB cameras
    match state.usb_camera_manager.stop_streaming().await {
        Ok(()) => {
            stopped_any = true;
        }
        Err(e) => {
            error!("Failed to stop USB cameras: {e}");
            errors.push(format!("USB: {e}"));
        }
    }

    if stopped_any || errors.is_empty() {
        Json(ApiResponse::success(()))
    } else {
        Json(ApiResponse::<()>::error(format!(
            "Failed to stop cameras: {}",
            errors.join(", ")
        )))
    }
}

async fn capture_images(
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<HashMap<String, String>>> {
    let status = state.camera_manager.get_status().await.unwrap_or_default();
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
                error!("Failed to capture from camera {camera_id}: {e}");
                results.insert(camera_id.clone(), format!("Error: {e}"));
            }
        }
    }

    Json(ApiResponse::success(results))
}

async fn camera_stream(
    Path(camera_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Response<Body>, StatusCode> {
    // Determine camera type and route to appropriate manager
    if camera_id.starts_with("usb:") {
        stream_usb_camera(&state, &camera_id).await
    } else {
        stream_esphome_camera(&state, &camera_id).await
    }
}

async fn stream_usb_camera(
    state: &Arc<AppState>,
    camera_id: &str,
) -> Result<Response<Body>, StatusCode> {
    let state_clone = state.clone();
    let camera_id_clone = camera_id.to_string();

    // Create an MJPEG stream
    let stream = async_stream::stream! {
        // Send initial boundary
        yield Ok::<Vec<u8>, Box<dyn std::error::Error + Send + Sync>>(
            b"--frame\r\n".to_vec()
        );

        loop {
            // Check if streaming should continue
            match state_clone.usb_camera_manager.get_status().await {
                Ok(status) => {
                    if !status.streaming {
                        info!("USB camera streaming stopped for camera {}", camera_id_clone);
                        break;
                    }
                }
                Err(e) => {
                    error!("Failed to get USB camera status: {e}");
                    break;
                }
            }

            match state_clone.usb_camera_manager.capture_streaming_frame(&camera_id_clone).await {
                Ok(frame_data) => {
                    // Create MJPEG frame with proper headers
                    let header = format!(
                        "Content-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                        frame_data.len()
                    );

                    // Yield the header
                    yield Ok(header.as_bytes().to_vec());

                    // Yield the frame data
                    yield Ok(frame_data);

                    // Yield the boundary for next frame
                    yield Ok(b"\r\n--frame\r\n".to_vec());
                }
                Err(e) => {
                    error!("Failed to capture frame from USB camera {}: {e}", camera_id_clone);
                    // Don't break the stream, just wait and try again
                    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                    continue;
                }
            }

            // Control frame rate - roughly 5 FPS for streaming
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        }
    };

    // Convert the stream to the format expected by Body::from_stream
    let byte_stream = stream.map(|result| result.map(axum::body::Bytes::from));

    let body = Body::from_stream(byte_stream);

    Response::builder()
        .header("Content-Type", "multipart/x-mixed-replace; boundary=frame")
        .header("Cache-Control", "no-cache, no-store, must-revalidate")
        .header("Pragma", "no-cache")
        .header("Expires", "0")
        .header("Connection", "close")
        .body(body)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn stream_esphome_camera(
    state: &Arc<AppState>,
    camera_id: &str,
) -> Result<Response<Body>, StatusCode> {
    // Get camera info to find the stream URL
    match state.camera_manager.list_cameras().await {
        Ok(cameras) => {
            let camera = cameras
                .iter()
                .find(|c| c.id == camera_id)
                .ok_or(StatusCode::NOT_FOUND)?;

            // Proxy the request to the ESPHome camera's stream URL
            match reqwest::get(camera.stream_url.clone()).await {
                Ok(response) => {
                    let status = response.status();
                    let headers = response.headers().clone();
                    let body = response
                        .bytes()
                        .await
                        .map_err(|_| StatusCode::BAD_GATEWAY)?;

                    let mut builder = Response::builder().status(status);

                    // Copy relevant headers
                    for (name, value) in headers.iter() {
                        if name == "content-type" || name == "cache-control" || name == "connection"
                        {
                            builder = builder.header(name, value);
                        }
                    }

                    builder
                        .body(Body::from(body))
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
                }
                Err(e) => {
                    error!("Failed to proxy ESPHome camera stream {}: {e}", camera_id);
                    Err(StatusCode::BAD_GATEWAY)
                }
            }
        }
        Err(e) => {
            error!("Failed to get camera list for streaming: {e}");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
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
#[allow(dead_code)]
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

async fn get_config(State(_state): State<Arc<AppState>>) -> Json<ApiResponse<ConfigData>> {
    // Load current configuration from user config file to ensure it's up to date
    let user_config = Settings::load_user_config();
    let config_data = ConfigData {
        auto_start_cameras: user_config.auto_start_esp32_cameras,
        auto_detect_cameras: user_config.auto_detect_cameras,
        esphome_hostname: user_config.esphome_hostname,
        network_camera_hostnames: user_config.network_camera_hostnames,
    };
    Json(ApiResponse::success(config_data))
}

async fn save_config(
    State(state): State<Arc<AppState>>,
    Json(config): Json<ConfigData>,
) -> Json<ApiResponse<()>> {
    info!(
        "Config save requested: auto_start={}, auto_detect={}, esphome={}, cameras={:?}",
        config.auto_start_cameras,
        config.auto_detect_cameras,
        config.esphome_hostname,
        config.network_camera_hostnames
    );

    // Load current user config to check for changes
    let current_user_config = Settings::load_user_config();

    // Check if ESPHome hostname has changed
    let hostname_changed = current_user_config.esphome_hostname != config.esphome_hostname;
    let camera_hostnames_changed =
        current_user_config.network_camera_hostnames != config.network_camera_hostnames;

    // Update controller monitor configuration if hostname changed
    if hostname_changed {
        // Create updated settings for the controller
        let mut new_settings = state.settings.clone();
        new_settings.esphome_hostname = config.esphome_hostname.clone();
        new_settings.network_camera_hostnames = config.network_camera_hostnames.clone();

        match state.controller.update_config(new_settings).await {
            Ok(()) => {
                info!("Controller monitor configuration updated successfully");
            }
            Err(e) => {
                error!("Failed to update controller monitor configuration: {}", e);
                return Json(ApiResponse::<()>::error(format!(
                    "Failed to update controller configuration: {e}"
                )));
            }
        }
    }

    // TODO: Also update camera manager configuration if camera hostnames changed
    if camera_hostnames_changed {
        info!("Camera hostname configuration changed - camera manager restart needed");
    }

    // Save changes to persistent user config file
    let mut user_config = current_user_config;
    user_config.esphome_hostname = config.esphome_hostname;
    user_config.network_camera_hostnames = config.network_camera_hostnames;
    user_config.auto_detect_cameras = config.auto_detect_cameras;
    user_config.auto_start_esp32_cameras = config.auto_start_cameras;

    match Settings::save_user_config(&user_config) {
        Ok(()) => {
            info!("Configuration saved to user config file successfully");
        }
        Err(e) => {
            error!("Failed to save configuration to file: {}", e);
            return Json(ApiResponse::<()>::error(format!(
                "Failed to save configuration to file: {e}"
            )));
        }
    }

    info!("Configuration updated successfully");

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
