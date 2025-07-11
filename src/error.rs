//! Error handling for the Shell Sorter application.

use thiserror::Error;

/// Application error types
#[derive(Error, Debug)]
pub enum OurError {
    /// IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization errors
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// HTTP request errors
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Image processing errors
    #[error("Image error: {0}")]
    Image(#[from] image::ImageError),

    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// Camera errors
    #[error("Camera error: {0}")]
    Camera(String),

    /// Hardware controller errors
    #[error("Hardware error: {0}")]
    Hardware(String),

    /// Machine learning errors
    #[error("ML error: {0}")]
    Ml(String),

    /// Generic application errors
    #[error("Application error: {0}")]
    App(String),
}

/// Application result type
pub type OurResult<T> = std::result::Result<T, OurError>;
