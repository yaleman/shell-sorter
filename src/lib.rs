#![deny(clippy::expect_used)]
#![deny(clippy::unwrap_used)]

pub mod camera_manager;
pub mod config;
pub mod controller_monitor;
pub mod error;
pub mod ml_training;
pub mod server;
pub mod shell_data;
pub mod usb_camera_controller;

pub use error::{OurError, OurResult};
