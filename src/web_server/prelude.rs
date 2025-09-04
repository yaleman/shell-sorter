pub use std::sync::Arc;

pub(crate) use axum::{
    extract::{Path, State},
    response::{Html, Json},
};

pub(crate) use askama::Template;
pub(crate) use askama_web::WebTemplate;

pub(crate) use reqwest::StatusCode;
pub(crate) use serde::{Deserialize, Serialize};

pub(crate) use crate::protocol::ApiResponse;
pub(crate) use crate::protocol::CameraType;
pub(crate) use crate::protocol::GlobalMessage;
pub(crate) use crate::server::AppState;
pub(crate) use std::collections::HashMap;
pub(crate) use tracing::{debug, error, info, instrument};
