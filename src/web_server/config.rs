use super::prelude::*;

/// Config template
#[derive(Template, WebTemplate)]
#[template(path = "config.html")]
pub(crate) struct ConfigTemplate {}

pub(crate) async fn config_page(
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

pub(crate) async fn reset_config(State(_state): State<Arc<AppState>>) -> Json<ApiResponse<()>> {
    // For now, just acknowledge the reset request
    // In a full implementation, this would reset configuration to defaults
    info!("Config reset to defaults requested");
    Json(ApiResponse::success(()))
}
