//! Web server implementation using Axum.

use askama::Template;
use askama_web::WebTemplate;
use axum::{
    Router,
    body::Body,
    extract::{Json as ExtractJson, Path, State},
    http::StatusCode,
    middleware,
    response::{Html, Json, Response},
    routing::{delete, get, post},
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};
use std::{num::NonZeroU16, sync::Arc};
use tokio::{net::TcpListener, sync::RwLock};

use tower_http::services::ServeDir;

use crate::{OurError, OurResult, protocol::CameraType};
use crate::{config::Settings, protocol::GlobalMessage};
use crate::{constants::USB_DEVICE_PREFIX_WITH_COLON, protocol};
use crate::{
    shell_data::{Shell, ShellDataManager},
    web_server::middleware::no_cache_middleware,
};
use tracing::{debug, error, info, instrument};

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub settings: Arc<RwLock<Settings>>,
    pub settings_filename: PathBuf,
    pub global_tx: tokio::sync::broadcast::Sender<crate::protocol::GlobalMessage>,
    pub global_rx: Arc<tokio::sync::broadcast::Receiver<crate::protocol::GlobalMessage>>,
}

/// Dashboard template
#[derive(Template, WebTemplate)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    machine_name: String,
    host: String,
    port: NonZeroU16,
}

/// Config template
#[derive(Template, WebTemplate)]
#[template(path = "config.html")]
struct ConfigTemplate {}

/// Shell edit template
#[derive(Template, WebTemplate)]
#[template(path = "shell_edit.html")]
struct ShellEditTemplate {
    shell: Shell,
    session_id: String,
}

/// Shell tagging template
#[derive(Template, WebTemplate)]
#[template(path = "tagging.html")]
struct TaggingTemplate {
    session_id: String,
    captured_images: Vec<CapturedImageData>,
    supported_case_types: Vec<String>,
    image_filenames: String,
}

#[derive(Serialize)]
struct CapturedImageData {
    filename: String,
    camera_index: i32,
    camera_name: String,
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
pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        // Static files
        .nest_service("/static", ServeDir::new("shell_sorter/static"))
        // Main dashboard and pages
        .route("/", get(dashboard))
        .route("/config", get(config_page))
        .route("/shell-edit/{session_id}", get(shell_edit_page))
        .route("/tagging/{session_id}", get(tagging_page))
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
        .route(
            "/api/cameras/{camera_id}/brightness",
            get(get_camera_brightness),
        )
        .route(
            "/api/cameras/{camera_id}/brightness",
            post(set_camera_brightness),
        )
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
        .with_state(state)
}

/// Start the web server
pub async fn start_server(
    settings: Arc<RwLock<Settings>>,
    settings_filename: PathBuf,
    global_tx: tokio::sync::broadcast::Sender<crate::protocol::GlobalMessage>,
    global_rx: Arc<tokio::sync::broadcast::Receiver<crate::protocol::GlobalMessage>>,
) -> OurResult<()> {
    // Initialize ML trainer and shell data manager
    // let mut ml_trainer = MLTrainer::new(settings.clone());
    // ml_trainer
    //     .initialize()
    //     .map_err(|e| OurError::App(format!("Failed to initialize ML trainer: {e}")))?;

    let settings_reader = settings.clone();
    let settings_reader = settings_reader.read().await;

    let addr = format!("{}:{}", settings_reader.host, settings_reader.port);

    let shell_data_manager = ShellDataManager::new(settings_reader.data_directory.clone());
    shell_data_manager
        .validate_data_directory()
        .map_err(|e| OurError::App(format!("Failed to validate data directory: {e}")))?;

    let state = Arc::new(AppState {
        settings,
        settings_filename,
        global_tx,
        global_rx,
    });

    let app = create_router(state);

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
    let settings_reader = state.settings.read().await;

    let template = DashboardTemplate {
        machine_name: settings_reader.machine_name.clone(),
        host: settings_reader.host.clone(),
        port: settings_reader.port,
    };

    template.render().map(Html::from).map_err(|e| {
        error!("Failed to render dashboard template: {e}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Template rendering failed",
        )
    })
}

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

async fn shell_edit_page(
    Path(session_id): Path<String>,
    State(_state): State<Arc<AppState>>,
) -> Result<Html<String>, (StatusCode, &'static str)> {
    // Get the shell data for the session
    // let shell = match state.shell_data_manager.get_shell(&session_id) {
    //     Ok(Some(shell)) => shell,
    //     Ok(None) => {
    //         return Err((StatusCode::NOT_FOUND, "Shell session not found"));
    //     }
    //     Err(e) => {
    //         error!("Failed to get shell data for session {}: {}", session_id, e);
    //         return Err((
    //             StatusCode::INTERNAL_SERVER_ERROR,
    //             "Failed to load shell data",
    //         ));
    //     }
    // };
    let shell = Shell::default(); // TODO: fix this

    let template = ShellEditTemplate { shell, session_id };

    template.render().map(Html::from).map_err(|e| {
        error!("Failed to render shell edit template: {e}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Template rendering failed",
        )
    })
}

#[axum::debug_handler]
async fn tagging_page(
    Path(session_id): Path<String>,
    State(_state): State<Arc<AppState>>,
) -> Result<Html<String>, (StatusCode, &'static str)> {
    // TODO: Get captured images for this session from images directory
    // For now, create empty captured images list
    let captured_images = Vec::new();

    // Get supported case types from ML trainer
    let supported_case_types = {
        // let ml_trainer = match state.ml_trainer.lock() {
        //     Ok(trainer) => trainer,
        //     Err(e) => {
        //         error!("Failed to acquire ML trainer lock: {}", e);
        //         return Err((
        //             StatusCode::INTERNAL_SERVER_ERROR,
        //             "Failed to access ML trainer",
        //         ));
        //     }
        // };

        // match ml_trainer.get_supported_case_types() {
        //     Ok(types) => types,
        //     Err(e) => {
        //         error!("Failed to get supported case types: {}", e);
        //         // Return empty list as fallback
        //         Vec::new()
        //     }
        // }
        Vec::new() // TODO: fix this
    };

    // Create comma-separated list of image filenames
    let image_filenames = captured_images
        .iter()
        .map(|img: &CapturedImageData| img.filename.clone())
        .collect::<Vec<String>>()
        .join(",");

    let template = TaggingTemplate {
        session_id,
        captured_images,
        supported_case_types,
        image_filenames,
    };

    template.render().map(Html::from).map_err(|e| {
        error!("Failed to render tagging template: {e}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Template rendering failed",
        )
    })
}

async fn status(State(_state): State<Arc<AppState>>) -> Json<StatusData> {
    // Get machine status for the overall system status by sending a message to the broadcast controller and waiting for a response

    // let machine_status = match state
    //     .controller
    //     .send_command(ControllerCommand::GetStatus)
    //     .await
    // {
    //     Ok(ControllerResponse::StatusData(status)) => status.status,
    //     Ok(_) => {
    //         error!("Unexpected response type for machine status");
    //         "Error".to_string()
    //     }
    //     Err(e) => {
    //         error!("Failed to get machine status: {e}");
    //         "Offline".to_string()
    //     }
    // };

    // TODO: Implement actual sorted count tracking
    // For now, return a placeholder value
    let total_sorted = 0;

    let machine_status = "Unimplemented".to_string(); // TODO: fix this

    Json(StatusData {
        status: machine_status,
        total_sorted,
    })
}

async fn trigger_next_case(State(state): State<Arc<AppState>>) -> Json<ApiResponse<()>> {
    match state.global_tx.send(GlobalMessage::NextCase) {
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
    let (responder, receiver) = tokio::sync::oneshot::channel();
    match state
        .global_tx
        .send(GlobalMessage::MachineStatus(Arc::new(responder)))
    {
        Ok(_) => match receiver.await {
            Ok(status) => Json(ApiResponse::success(status)),
            Err(e) => {
                error!("Failed to receive machine status: {e}");
                Json(ApiResponse::error(format!(
                    "Failed to receive machine status: {e}"
                )))
            }
        },

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
    let (responder, receiver) = tokio::sync::oneshot::channel();
    match state
        .global_tx
        .send(GlobalMessage::GetSensors(Arc::new(responder)))
    {
        Ok(_) => match receiver.await {
            Ok(readings) => Json(ApiResponse::success(readings)),
            Err(e) => {
                error!("Failed to receive sensor readings: {e}");
                Json(ApiResponse::error(format!(
                    "Failed to receive sensor readings: {e}"
                )))
            }
        },
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
    let (responder, receiver) = tokio::sync::oneshot::channel();

    match state
        .global_tx
        .send(GlobalMessage::ControllerStatus(Arc::new(responder)))
    {
        Ok(_) => match receiver.await {
            Ok(status) => {
                let settings_reader = state.settings.read().await;
                let mut hardware_status = HashMap::new();
                hardware_status.insert("controller".to_string(), format!("{}", status.online));
                hardware_status.insert(
                    "esphome_hostname".to_string(),
                    settings_reader.esphome_hostname.clone(),
                );
                Json(ApiResponse::success(hardware_status))
            }
            Err(e) => {
                let settings_reader = state.settings.read().await;
                error!("Failed to receive hardware status: {e}");
                let mut fallback_status = HashMap::new();
                fallback_status.insert("controller".to_string(), "Error".to_string());
                fallback_status.insert(
                    "esphome_hostname".to_string(),
                    settings_reader.esphome_hostname.clone(),
                );
                Json(ApiResponse::success(fallback_status))
            }
        },
        Err(e) => {
            let settings_reader = state.settings.read().await;
            error!("Failed to get hardware status: {e}");
            let mut fallback_status = HashMap::new();
            fallback_status.insert("controller".to_string(), "Disconnected".to_string());
            fallback_status.insert(
                "esphome_hostname".to_string(),
                settings_reader.esphome_hostname.clone(),
            );
            Json(ApiResponse::success(fallback_status))
        }
    }
}

async fn list_cameras(State(_state): State<Arc<AppState>>) -> Json<ApiResponse<Vec<CameraInfo>>> {
    let all_cameras = Vec::new();

    // // Load saved camera selections from config
    // let user_config = Settings::load_user_config();
    // let saved_selections = user_config.get_selected_cameras();

    // // Get ESPHome camera status
    // let esphome_status = state.camera_manager.get_status().await.unwrap_or_default();

    // // Get ESPHome cameras
    // match state.camera_manager.list_cameras().await {
    //     Ok(cameras) => {
    //         let esphome_cameras: Vec<CameraInfo> = cameras
    //             .into_iter()
    //             .map(|cam| {
    //                 // Check both in-memory status and saved config for selection
    //                 let is_selected_in_memory = esphome_status.selected_cameras.contains(&cam.id);
    //                 let is_selected_in_config = saved_selections.contains(&cam.id);
    //                 let is_selected = is_selected_in_memory || is_selected_in_config;
    //                 let is_active = is_selected_in_memory && esphome_status.streaming;

    //                 CameraInfo {
    //                     id: cam.id,
    //                     name: cam.name,
    //                     hostname: Some(cam.hostname),
    //                     online: cam.online,
    //                     view_type: None,
    //                     camera_type: CameraType::EspHome,
    //                     index: None,
    //                     vendor_id: None,
    //                     product_id: None,
    //                     serial_number: None,
    //                     is_active,
    //                     is_selected,
    //                 }
    //             })
    //             .collect();
    //         all_cameras.extend(esphome_cameras);
    //     }
    //     Err(e) => {
    //         error!("Failed to list ESPHome cameras: {e}");
    //     }
    // }

    // // Get USB camera status
    // let usb_status = state
    //     .usb_camera_manager
    //     .get_status()
    //     .await
    //     .unwrap_or_default();

    // // Get USB cameras
    // match state.usb_camera_manager.list_cameras().await {
    //     Ok(cameras) => {
    //         let usb_cameras: Vec<CameraInfo> = cameras
    //             .into_iter()
    //             .map(|cam| {
    //                 // Check both in-memory status and saved config for selection
    //                 let is_selected_in_memory =
    //                     usb_status.selected_cameras().contains(&cam.hardware_id);
    //                 let is_selected_in_config = saved_selections.contains(&cam.hardware_id);
    //                 let is_selected = is_selected_in_memory || is_selected_in_config;
    //                 let is_active = is_selected_in_memory && usb_status.streaming;

    //                 CameraInfo {
    //                     id: cam.hardware_id.clone(),
    //                     name: cam.name,
    //                     hostname: None,
    //                     online: cam.connected,
    //                     view_type: None,
    //                     camera_type: CameraType::Usb,
    //                     index: Some(cam.index),
    //                     vendor_id: cam.vendor_id,
    //                     product_id: cam.product_id,
    //                     serial_number: cam.serial_number,
    //                     is_active,
    //                     is_selected,
    //                 }
    //             })
    //             .collect();
    //         all_cameras.extend(usb_cameras);
    //     }
    //     Err(e) => {
    //         error!("Failed to list USB cameras: {e}");
    //     }
    // }

    // // Sort cameras by human-facing name for consistency
    // all_cameras.sort_by(|a, b| a.name.cmp(&b.name));

    Json(ApiResponse::success(all_cameras))
}

/// Restore saved camera selections from persistent config
#[allow(dead_code)]
async fn restore_saved_camera_selections(_state: &Arc<AppState>) {
    let user_config = Settings::load_user_config();
    let saved_selections = user_config.get_selected_cameras();

    if saved_selections.is_empty() {
        return;
    }

    info!("Restoring saved camera selections: {:?}", saved_selections);

    // Separate camera IDs by type
    let mut esphome_cameras = Vec::new();
    let mut usb_cameras = Vec::new();

    for camera_id in saved_selections {
        if camera_id.starts_with(USB_DEVICE_PREFIX_WITH_COLON) {
            usb_cameras.push(camera_id.clone());
        } else {
            esphome_cameras.push(camera_id.clone());
        }
    }

    // Restore ESPHome camera selections
    // if !esphome_cameras.is_empty() {
    //     if let Err(e) = state.camera_manager.select_cameras(esphome_cameras).await {
    //         error!("Failed to restore ESPHome camera selections: {e}");
    //     } else {
    //         info!("Restored ESPHome camera selections");
    //     }
    // }

    // Restore USB camera selections
    // if !usb_cameras.is_empty() {
    //     if let Err(e) = state.usb_camera_manager.select_cameras(usb_cameras).await {
    //         error!("Failed to restore USB camera selections: {e}");
    //     } else {
    //         info!("Restored USB camera selections");
    //     }
    // }
}

async fn detect_cameras(State(state): State<Arc<AppState>>) -> Json<ApiResponse<String>> {
    debug!("Camera detection requested - triggering async detection");

    state.global_tx.send(GlobalMessage::DetectCameras).ok();

    // Return immediately with status message
    Json(ApiResponse::success(
        "Camera detection started. Use /api/cameras to check results.".to_string(),
    ))
}

#[instrument(level = "info", skip(state))]
async fn select_cameras(
    State(state): State<Arc<AppState>>,
    ExtractJson(payload): ExtractJson<SelectCamerasRequest>,
) -> Json<ApiResponse<()>> {
    // Store camera IDs for persistence before consuming them
    let camera_ids_for_config = payload.camera_ids.clone();

    // Separate camera IDs by type
    let mut esphome_cameras = Vec::new();
    let mut usb_cameras = Vec::new();

    for camera_id in payload.camera_ids {
        if camera_id.starts_with(USB_DEVICE_PREFIX_WITH_COLON) {
            usb_cameras.push(camera_id);
        } else {
            esphome_cameras.push(camera_id);
        }
    }

    if let Err(err) = state
        .global_tx
        .send(GlobalMessage::SelectCameras(Vec::new()))
    {
        error!("Failed to send global message: {err}");
        return Json(ApiResponse::<()>::error(format!(
            "Failed to send camera selection request: {err}"
        )));
    }

    // Save selected camera IDs to persistent configuration
    let mut user_config = Settings::load_user_config();
    user_config.set_selected_cameras(camera_ids_for_config);
    if let Err(e) = Settings::save_user_config(&user_config) {
        error!("Failed to save camera selections to config: {e}");
        // Don't fail the request, just log the error
    } else {
        info!("Saved camera selections to persistent configuration");
    }

    Json(ApiResponse::success(()))
}

async fn start_cameras(
    State(state): State<Arc<AppState>>,
    ExtractJson(payload): ExtractJson<SelectCamerasRequest>,
) -> Json<ApiResponse<()>> {
    let camera_ids: Vec<crate::protocol::CameraType> =
        payload.camera_ids.into_iter().map(|id| id.into()).collect();

    match state
        .global_tx
        .send(GlobalMessage::SelectCameras(camera_ids))
    {
        Ok(_) => Json(ApiResponse::success(())),
        Err(err) => {
            error!("Failed to send camera start request");
            Json(ApiResponse::<()>::error(format!(
                "Failed to start cameras: {err}",
            )))
        }
    }
}

async fn stop_cameras(State(state): State<Arc<AppState>>) -> Json<ApiResponse<()>> {
    state.global_tx.send(GlobalMessage::StopCameras).ok();

    Json(ApiResponse::success(()))
}

async fn capture_images(
    State(_state): State<Arc<AppState>>,
) -> Json<ApiResponse<HashMap<String, String>>> {
    let results = HashMap::new(); // TODO: implement camera capture logic

    Json(ApiResponse::success(results))
}

async fn camera_stream(
    Path(camera_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Response<Body>, StatusCode> {
    // Determine camera type and route to appropriate manager
    if camera_id.starts_with(USB_DEVICE_PREFIX_WITH_COLON) {
        stream_usb_camera(&state, &camera_id).await
    } else {
        stream_esphome_camera(&state, &camera_id).await
    }
}

async fn stream_usb_camera(
    _state: &Arc<AppState>,
    _camera_id: &str,
) -> Result<Response<Body>, StatusCode> {
    // let state_clone = state.clone();
    // let camera_id_clone = camera_id.to_string();

    // Create an MJPEG stream
    let stream = async_stream::stream! {
        // Send initial boundary
        yield Ok::<Vec<u8>, Box<dyn std::error::Error + Send + Sync>>(
            b"--frame\r\n".to_vec()
        );

        // loop {
        //     // Check if streaming should continue
        //     // match state_clone.usb_camera_manager.get_status().await {
        //     //     Ok(status) => {
        //     //         if !status.streaming {
        //     //             info!("USB camera streaming stopped for camera {}", camera_id_clone);
        //     //             break;
        //     //         }
        //     //     }
        //     //     Err(e) => {
        //     //         error!("Failed to get USB camera status: {e}");
        //     //         break;
        //     //     }
        //     // }

        //     // match state_clone.usb_camera_manager.capture_streaming_frame(&camera_id_clone).await {
        //     //     Ok(frame_data) => {
        //     //         // Create MJPEG frame with proper headers
        //     //         let header = format!(
        //     //             "Content-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
        //     //             frame_data.len()
        //     //         );

        //     //         // Yield the header
        //     //         yield Ok(header.as_bytes().to_vec());

        //     //         // Yield the frame data
        //     //         yield Ok(frame_data);

        //     //         // Yield the boundary for next frame
        //     //         yield Ok(b"\r\n--frame\r\n".to_vec());
        //     //     }
        //     //     Err(e) => {
        //     //         error!("Failed to capture frame from USB camera {}: {e}", camera_id_clone);
        //     //         // Don't break the stream, just wait and try again
        //     //         tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        //     //         continue;
        //     //     }
        //     // }

        //     // Control frame rate - roughly 5 FPS for streaming
        //     tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        // }
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
    _state: &Arc<AppState>,
    _camera_id: &str,
) -> Result<Response<Body>, StatusCode> {
    // TODO: implement ESPHome camera streaming
    // Get camera info to find the stream URL
    // match state.camera_manager.list_cameras().await {
    //     Ok(cameras) => {
    //         let camera = cameras
    //             .iter()
    //             .find(|c| c.id == camera_id)
    //             .ok_or(StatusCode::NOT_FOUND)?;

    //         // Proxy the request to the ESPHome camera's stream URL
    //         match reqwest::get(camera.stream_url.clone()).await {
    //             Ok(response) => {
    //                 let status = response.status();
    //                 let headers = response.headers().clone();
    //                 let body = response
    //                     .bytes()
    //                     .await
    //                     .map_err(|_| StatusCode::BAD_GATEWAY)?;

    //                 let mut builder = Response::builder().status(status);

    //                 // Copy relevant headers
    //                 for (name, value) in headers.iter() {
    //                     if name == "content-type" || name == "cache-control" || name == "connection"
    //                     {
    //                         builder = builder.header(name, value);
    //                     }
    //                 }

    //                 builder
    //                     .body(Body::from(body))
    //                     .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    //             }
    //             Err(e) => {
    //                 error!("Failed to proxy ESPHome camera stream {}: {e}", camera_id);
    //                 Err(StatusCode::BAD_GATEWAY)
    //             }
    //         }
    //     }
    //     Err(e) => {
    //         error!("Failed to get camera list for streaming: {e}");
    //         Err(StatusCode::INTERNAL_SERVER_ERROR)
    //     }
    // }
    Err(StatusCode::INTERNAL_SERVER_ERROR)
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
) -> Json<ApiResponse<Vec<HashMap<String, serde_json::Value>>>> {
    // match state.shell_data_manager.list_shells() {
    //     Ok(shells) => {
    //         let shell_data: Vec<HashMap<String, serde_json::Value>> = shells
    //             .into_iter()
    //             .map(|(session_id, shell)| {
    //                 let mut data = HashMap::new();
    //                 data.insert(
    //                     "session_id".to_string(),
    //                     serde_json::Value::String(session_id),
    //                 );
    //                 data.insert(
    //                     "brand".to_string(),
    //                     serde_json::Value::String(shell.brand.clone()),
    //                 );
    //                 data.insert(
    //                     "shell_type".to_string(),
    //                     serde_json::Value::String(shell.shell_type.clone()),
    //                 );
    //                 data.insert(
    //                     "date_captured".to_string(),
    //                     serde_json::Value::String(shell.date_captured.to_rfc3339()),
    //                 );
    //                 data.insert(
    //                     "include".to_string(),
    //                     serde_json::Value::Bool(shell.include),
    //                 );
    //                 data.insert(
    //                     "image_count".to_string(),
    //                     serde_json::Value::Number(serde_json::Number::from(shell.image_count())),
    //                 );
    //                 data.insert(
    //                     "has_complete_regions".to_string(),
    //                     serde_json::Value::Bool(shell.has_complete_regions()),
    //                 );
    //                 data
    //             })
    //             .collect();

    //         Json(ApiResponse::success(shell_data))
    //     }
    //     Err(e) => {
    //         error!("Failed to list shells: {}", e);
    //         Json(ApiResponse::error(format!("Failed to list shells: {e}")))
    //     }
    // }
    Json(ApiResponse::error(
        "Failed to list shells: unimplemented".to_string(),
    ))
}

async fn save_shell_data(
    State(_state): State<Arc<AppState>>,
    ExtractJson(payload): ExtractJson<SaveShellRequest>,
) -> Json<ApiResponse<HashMap<String, String>>> {
    let mut shell = Shell::new(payload.brand, payload.shell_type);
    shell.include = payload.include;
    shell.image_filenames = payload.image_filenames;

    // match state
    //     .shell_data_manager
    //     .save_shell(&payload.session_id, &shell)
    // {
    //     Ok(()) => {
    //         let mut response = HashMap::new();
    //         response.insert("session_id".to_string(), payload.session_id);
    //         response.insert(
    //             "message".to_string(),
    //             "Shell data saved successfully".to_string(),
    //         );
    //         Json(ApiResponse::success(response))
    //     }
    //     Err(e) => {
    //         error!(
    //             "Failed to save shell data for session {}: {}",
    //             payload.session_id, e
    //         );
    Json(ApiResponse::error(
        "Failed to save shell data: unimplemented".to_string(),
    ))
    //     }
    // }
}

async fn toggle_shell_training(
    Path(_session_id): Path<String>,
    State(_state): State<Arc<AppState>>,
) -> Json<ApiResponse<HashMap<String, bool>>> {
    // match state.shell_data_manager.toggle_shell_training(&session_id) {
    //     Ok(include_flag) => {
    //         let mut response = HashMap::new();
    //         response.insert("include".to_string(), include_flag);
    //         Json(ApiResponse::success(response))
    //     }
    //     Err(e) => {
    //         error!(
    //             "Failed to toggle training for session {}: {}",
    //             session_id, e
    //         );
    //         Json(ApiResponse::error(format!(
    //             "Failed to toggle training: {e}"
    //         )))
    //     }
    // }
    Json(ApiResponse::error(
        "Failed to toggle training: unimplemented".to_string(),
    ))
}

async fn ml_list_shells(
    State(_state): State<Arc<AppState>>,
) -> Json<ApiResponse<Vec<HashMap<String, serde_json::Value>>>> {
    // match state.shell_data_manager.get_shells_for_training() {
    //     Ok(shells) => {
    //         let shell_data: Vec<HashMap<String, serde_json::Value>> = shells
    //             .into_iter()
    //             .map(|(session_id, shell)| {
    //                 let mut data = HashMap::new();
    //                 data.insert(
    //                     "session_id".to_string(),
    //                     serde_json::Value::String(session_id),
    //                 );
    //                 data.insert(
    //                     "brand".to_string(),
    //                     serde_json::Value::String(shell.brand.clone()),
    //                 );
    //                 data.insert(
    //                     "shell_type".to_string(),
    //                     serde_json::Value::String(shell.shell_type.clone()),
    //                 );
    //                 data.insert(
    //                     "date_captured".to_string(),
    //                     serde_json::Value::String(shell.date_captured.to_rfc3339()),
    //                 );
    //                 data.insert(
    //                     "include".to_string(),
    //                     serde_json::Value::Bool(shell.include),
    //                 );
    //                 data.insert(
    //                     "image_count".to_string(),
    //                     serde_json::Value::Number(serde_json::Number::from(shell.image_count())),
    //                 );
    //                 data.insert(
    //                     "has_complete_regions".to_string(),
    //                     serde_json::Value::Bool(shell.has_complete_regions()),
    //                 );
    //                 data.insert(
    //                     "case_type_key".to_string(),
    //                     serde_json::Value::String(shell.get_case_type_key()),
    //                 );

    //                 // Add image filenames if available
    //                 if !shell.image_filenames.is_empty() {
    //                     let filenames: Vec<serde_json::Value> = shell
    //                         .image_filenames
    //                         .into_iter()
    //                         .map(serde_json::Value::String)
    //                         .collect();
    //                     data.insert(
    //                         "image_filenames".to_string(),
    //                         serde_json::Value::Array(filenames),
    //                     );
    //                 }

    //                 data
    //             })
    //             .collect();

    //         Json(ApiResponse::success(shell_data))
    //     }
    //     Err(e) => {
    //         error!("Failed to list shells for ML training: {}", e);
    //         Json(ApiResponse::error(format!(
    //             "Failed to list shells for ML training: {e}"
    //         )))
    //     }
    // }
    todo!();
}

async fn generate_composites(State(_state): State<Arc<AppState>>) -> Json<ApiResponse<()>> {
    // TODO: Implement composite generation
    Json(ApiResponse::success(()))
}

async fn list_case_types(
    State(_state): State<Arc<AppState>>,
) -> Json<ApiResponse<Vec<HashMap<String, serde_json::Value>>>> {
    // let ml_trainer = match state.ml_trainer.lock() {
    //     Ok(trainer) => trainer,
    //     Err(_) => {
    //         error!("Failed to acquire ML trainer lock");
    //         return Json(ApiResponse::error(
    //             "Failed to access ML trainer".to_string(),
    //         ));
    //     }
    // };

    // match ml_trainer.get_training_summary() {
    //     Ok(summary) => {
    //         let case_types: Vec<HashMap<String, serde_json::Value>> = summary
    //             .into_iter()
    //             .map(|(name, summary_data)| {
    //                 let mut data = HashMap::new();
    //                 data.insert("name".to_string(), serde_json::Value::String(name));
    //                 data.insert(
    //                     "designation".to_string(),
    //                     serde_json::Value::String(summary_data.designation),
    //                 );
    //                 data.insert(
    //                     "brand".to_string(),
    //                     summary_data
    //                         .brand
    //                         .map(serde_json::Value::String)
    //                         .unwrap_or(serde_json::Value::Null),
    //                 );
    //                 data.insert(
    //                     "reference_count".to_string(),
    //                     serde_json::Value::Number(serde_json::Number::from(
    //                         summary_data.reference_count,
    //                     )),
    //                 );
    //                 data.insert(
    //                     "training_count".to_string(),
    //                     serde_json::Value::Number(serde_json::Number::from(
    //                         summary_data.training_count,
    //                     )),
    //                 );
    //                 data.insert(
    //                     "shell_count".to_string(),
    //                     serde_json::Value::Number(serde_json::Number::from(
    //                         summary_data.shell_count,
    //                     )),
    //                 );
    //                 data.insert(
    //                     "ready_for_training".to_string(),
    //                     serde_json::Value::Bool(summary_data.ready_for_training),
    //                 );
    //                 data.insert(
    //                     "updated_at".to_string(),
    //                     serde_json::Value::String(summary_data.updated_at.to_rfc3339()),
    //                 );
    //                 data
    //             })
    //             .collect();

    //         Json(ApiResponse::success(case_types))
    //     }
    //     Err(e) => {
    //         error!("Failed to get training summary: {}", e);
    //         Json(ApiResponse::error(format!("Failed to get case types: {e}")))
    //     }
    // }
    todo!()
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct CreateCaseTypeRequest {
    name: String,
    designation: Option<String>,
}

#[derive(Deserialize, Debug)]
struct SelectCamerasRequest {
    camera_ids: Vec<String>,
}

#[derive(Deserialize)]
struct BrightnessRequest {
    brightness: i64,
}

#[derive(Deserialize)]
struct SaveShellRequest {
    _session_id: String,
    brand: String,
    shell_type: String,
    include: bool,
    image_filenames: Vec<String>,
}

#[derive(Serialize)]
struct BrightnessResponse {
    brightness: i64,
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

async fn get_config(State(_state): State<Arc<AppState>>) -> Json<ConfigData> {
    // Load current configuration from user config file to ensure it's up to date
    let user_config = Settings::load_user_config();
    let config_data = ConfigData {
        auto_start_cameras: user_config.auto_start_esp32_cameras,
        auto_detect_cameras: user_config.auto_detect_cameras,
        esphome_hostname: user_config.esphome_hostname,
        network_camera_hostnames: user_config.network_camera_hostnames,
    };
    Json(config_data)
}

async fn save_config(
    State(state): State<Arc<AppState>>,
    Json(new_config): Json<ConfigData>,
) -> Json<ApiResponse<()>> {
    info!(
        "Config save requested: auto_start={}, auto_detect={}, esphome={}, cameras={:?}",
        new_config.auto_start_cameras,
        new_config.auto_detect_cameras,
        new_config.esphome_hostname,
        new_config.network_camera_hostnames
    );

    let mut settings_writer = state.settings.write().await.clone();
    let mut changed_config = false;
    if new_config.auto_start_cameras != settings_writer.auto_start_esp32_cameras {
        info!(
            "Updating auto_start_cameras from {} to {}",
            settings_writer.auto_start_esp32_cameras, new_config.auto_start_cameras
        );
        settings_writer.auto_start_esp32_cameras = new_config.auto_start_cameras;
        changed_config = true;
    }

    if changed_config {
        match settings_writer
            .write_to_disk(&state.settings_filename)
            .await
        {
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
    } else {
        info!("No configuration changes detected");
    }

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

async fn get_camera_brightness(
    Path(camera_id): Path<String>,
    State(_state): State<Arc<AppState>>,
) -> Json<ApiResponse<BrightnessResponse>> {
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
    Json(ApiResponse::<BrightnessResponse>::error(
        "ESPHome cameras do not support brightness control".to_string(),
    ))
    // }
}

async fn set_camera_brightness(
    Path(camera_id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<BrightnessRequest>,
) -> Json<ApiResponse<()>> {
    let camera_id: protocol::CameraType = camera_id.into();

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

    match camera_id.clone() {
        protocol::CameraType::EspHome(_) => {
            // ESPHome cameras do not support brightness control
            Json(ApiResponse::<()>::error(
                "ESPHome cameras do not support brightness control".to_string(),
            ))
        }
        protocol::CameraType::Usb(id) => {
            // USB cameras can have brightness set
            info!("Setting brightness for USB camera: {id:?}");
            match state.global_tx.send(GlobalMessage::SetUsbCameraBrightness {
                camera_id,
                brightness: payload.brightness,
            }) {
                Ok(_) => {
                    info!("Brightness set successfully for USB camera: {id:?}");
                    Json(ApiResponse::success(()))
                }
                Err(err) => {
                    error!("Failed to set brightness for USB camera {id:?}: {err}");
                    Json(ApiResponse::<()>::error(format!(
                        "Failed to set brightness for USB camera {id:?}: {err}"
                    )))
                }
            }
        }
    }
}
