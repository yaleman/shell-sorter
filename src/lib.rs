#![deny(clippy::expect_used)]
#![deny(clippy::unwrap_used)]

pub mod camera_manager;
pub mod config;
pub mod controller_monitor;
pub mod error;
pub mod server;

pub use error::{OurError, OurResult};
