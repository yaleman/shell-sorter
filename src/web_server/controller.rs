//! Functionality to control the machine itself
//!

use tokio::sync::RwLock;

use super::prelude::*;

pub(crate) async fn hardware_status(
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<HashMap<String, String>>> {
    let (responder, receiver) = tokio::sync::oneshot::channel();

    match state.global_tx.send(GlobalMessage::ControllerStatus {
        responder: Arc::new(responder),
    }) {
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

pub(crate) async fn trigger_next_case(State(state): State<Arc<AppState>>) -> Json<ApiResponse<()>> {
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

pub(crate) async fn machine_status(
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
