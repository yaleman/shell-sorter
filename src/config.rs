//! Configuration management for the Shell Sorter application.
//!
//! This module provides configuration structures for managing application settings,
//! camera configurations, and user preferences. It uses Serde for serialization
//! and supports environment variable overrides.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;

/// Configuration settings for the Shell Sorter application.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Server host address
    pub host: String,
    /// Server port
    pub port: u16,
    /// Enable debug mode
    pub debug: bool,
    /// Machine identifier
    pub machine_name: String,
    /// Camera device paths
    pub cameras: Vec<String>,
    /// Number of cameras on the machine
    pub camera_count: u32,
    /// Camera resolution
    pub camera_resolution: String,
    /// Training images directory
    pub image_directory: PathBuf,
    /// Data directory for uploads and models
    pub data_directory: PathBuf,
    /// ML models directory
    pub models_directory: PathBuf,
    /// Reference images directory
    pub references_directory: PathBuf,
    /// Enable ML case identification
    pub ml_enabled: bool,
    /// ML confidence threshold
    pub confidence_threshold: f64,
    /// Active ML model name
    pub model_name: Option<String>,
    /// Supported ammunition case types
    pub supported_case_types: Vec<String>,
    /// ESPHome device hostname for API communication
    pub esphome_hostname: String,
    /// List of ESPHome camera hostnames to detect
    pub network_camera_hostnames: Vec<String>,
    /// Automatically detect and configure cameras on startup
    pub auto_detect_cameras: bool,
    /// Automatically start configured ESP32 cameras when they come online
    pub auto_start_esp32_cameras: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8000,
            debug: false,
            machine_name: "Shell Sorter v1.0".to_string(),
            cameras: Vec::new(),
            camera_count: 4,
            camera_resolution: "1920x1080".to_string(),
            image_directory: PathBuf::from("./images"),
            data_directory: PathBuf::from("./data"),
            models_directory: PathBuf::from("./data/models"),
            references_directory: PathBuf::from("./data/references"),
            ml_enabled: true,
            confidence_threshold: 0.8,
            model_name: None,
            supported_case_types: vec![
                "9mm".to_string(),
                "40sw".to_string(),
                "45acp".to_string(),
                "223rem".to_string(),
                "308win".to_string(),
                "3006spr".to_string(),
                "38special".to_string(),
                "357mag".to_string(),
            ],
            esphome_hostname: "shell-sorter-controller.local".to_string(),
            network_camera_hostnames: vec!["esp32cam1.local".to_string()],
            auto_detect_cameras: false,
            auto_start_esp32_cameras: true,
        }
    }
}

/// Camera view type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ViewType {
    Side,
    Tail,
}

/// Configuration for a specific camera
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CameraConfig {
    /// Camera view type
    pub view_type: Option<ViewType>,
    /// Region x coordinate
    pub region_x: Option<i32>,
    /// Region y coordinate
    pub region_y: Option<i32>,
    /// Region width
    pub region_width: Option<u32>,
    /// Region height
    pub region_height: Option<i32>,
    /// Detected resolution width for ESP cameras
    pub detected_resolution_width: Option<i32>,
    /// Detected resolution height for ESP cameras
    pub detected_resolution_height: Option<i32>,
    /// Manual resolution width
    pub manual_resolution_width: Option<i32>,
    /// Manual resolution height
    pub manual_resolution_height: Option<i32>,
    /// Resolution detection timestamp
    pub resolution_detection_timestamp: Option<f64>,
}

/// User configuration that persists across application restarts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserConfig {
    /// Camera configurations by name
    pub camera_configs: HashMap<String, CameraConfig>,
    /// List of ESPHome camera hostnames to detect
    pub network_camera_hostnames: Vec<String>,
    /// Automatically detect and configure cameras on startup
    pub auto_detect_cameras: bool,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            camera_configs: HashMap::new(),
            network_camera_hostnames: vec!["esp32cam1.local".to_string()],
            auto_detect_cameras: false,
        }
    }
}

impl UserConfig {
    /// Get configuration for a camera by name
    pub fn get_camera_config(&self, camera_name: &str) -> CameraConfig {
        self.camera_configs
            .get(camera_name)
            .cloned()
            .unwrap_or_default()
    }

    /// Set configuration for a camera by name
    pub fn set_camera_config(&mut self, camera_name: String, config: CameraConfig) {
        self.camera_configs.insert(camera_name, config);
    }

    /// Clear configuration for a camera by name
    pub fn clear_camera_config(&mut self, camera_name: &str) {
        self.camera_configs.remove(camera_name);
    }

    /// Remove configuration for a camera by name (alias for clear_camera_config)
    pub fn remove_camera_config(&mut self, camera_name: &str) {
        self.clear_camera_config(camera_name);
    }
}

impl Settings {
    /// Create a new instance of Settings with environment variable overrides
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut settings = Settings::default();

        // Override with environment variables if present
        if let Ok(host) = env::var("SHELL_SORTER_HOST") {
            settings.host = host;
        }
        if let Ok(port) = env::var("SHELL_SORTER_PORT") {
            settings.port = port.parse()?;
        }
        if let Ok(debug) = env::var("SHELL_SORTER_DEBUG") {
            settings.debug = debug.parse()?;
        }
        if let Ok(machine_name) = env::var("SHELL_SORTER_MACHINE_NAME") {
            settings.machine_name = machine_name;
        }
        if let Ok(camera_count) = env::var("SHELL_SORTER_CAMERA_COUNT") {
            settings.camera_count = camera_count.parse()?;
        }
        if let Ok(camera_resolution) = env::var("SHELL_SORTER_CAMERA_RESOLUTION") {
            settings.camera_resolution = camera_resolution;
        }
        if let Ok(ml_enabled) = env::var("SHELL_SORTER_ML_ENABLED") {
            settings.ml_enabled = ml_enabled.parse()?;
        }
        if let Ok(confidence_threshold) = env::var("SHELL_SORTER_CONFIDENCE_THRESHOLD") {
            settings.confidence_threshold = confidence_threshold.parse()?;
        }
        if let Ok(model_name) = env::var("SHELL_SORTER_MODEL_NAME") {
            settings.model_name = Some(model_name);
        }
        if let Ok(esphome_hostname) = env::var("SHELL_SORTER_ESPHOME_HOSTNAME") {
            settings.esphome_hostname = esphome_hostname;
        }
        if let Ok(auto_detect_cameras) = env::var("SHELL_SORTER_AUTO_DETECT_CAMERAS") {
            settings.auto_detect_cameras = auto_detect_cameras.parse()?;
        }
        if let Ok(auto_start_esp32_cameras) = env::var("SHELL_SORTER_AUTO_START_ESP32_CAMERAS") {
            settings.auto_start_esp32_cameras = auto_start_esp32_cameras.parse()?;
        }

        // Create all necessary directories
        settings.create_directories()?;

        Ok(settings)
    }

    /// Create all necessary directories
    fn create_directories(&self) -> Result<(), Box<dyn std::error::Error>> {
        let directories = [
            &self.image_directory,
            &self.data_directory,
            &self.models_directory,
            &self.references_directory,
        ];

        for directory in directories {
            if !directory.exists() {
                fs::create_dir_all(directory)?;
            }
        }

        Ok(())
    }

    /// Get the path to the user config file
    pub fn get_config_path() -> PathBuf {
        // Allow override via environment variable for testing
        if let Ok(config_path_override) = env::var("SHELL_SORTER_CONFIG_PATH") {
            let config_path = PathBuf::from(config_path_override);
            if let Some(parent) = config_path.parent() {
                fs::create_dir_all(parent).ok();
            }
            return config_path;
        }

        // Auto-detect pytest and use temporary config to prevent touching live config
        if env::var("PYTEST_CURRENT_TEST").is_ok() {
            let temp_dir = std::env::temp_dir().join("shell-sorter-pytest");
            fs::create_dir_all(&temp_dir).ok();
            return temp_dir.join("test-config.json");
        }

        // Default to ~/.config/shell-sorter.json for production
        let config_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config");
        fs::create_dir_all(&config_dir).ok();
        config_dir.join("shell-sorter.json")
    }

    /// Load user configuration from shell-sorter.json
    pub fn load_user_config() -> UserConfig {
        let config_path = Self::get_config_path();
        if !config_path.exists() {
            return UserConfig::default();
        }

        match fs::read_to_string(&config_path) {
            Ok(contents) => match serde_json::from_str::<UserConfig>(&contents) {
                Ok(config) => config,
                Err(e) => {
                    eprintln!("Failed to parse user config from {config_path:?}: {e}");
                    UserConfig::default()
                }
            },
            Err(e) => {
                eprintln!("Failed to read user config from {config_path:?}: {e}");
                UserConfig::default()
            }
        }
    }

    /// Save user configuration to shell-sorter.json
    pub fn save_user_config(config: &UserConfig) -> Result<(), Box<dyn std::error::Error>> {
        let config_path = Self::get_config_path();

        // Ensure directory exists
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let contents = serde_json::to_string_pretty(config)?;
        fs::write(&config_path, contents)?;

        println!("Saved user config to {config_path:?}");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_default() {
        let settings = Settings::default();
        assert_eq!(settings.host, "127.0.0.1");
        assert_eq!(settings.port, 8000);
        assert!(!settings.debug);
        assert_eq!(settings.machine_name, "Shell Sorter v1.0");
        assert_eq!(settings.camera_count, 4);
        assert_eq!(settings.camera_resolution, "1920x1080");
        assert!(settings.ml_enabled);
        assert_eq!(settings.confidence_threshold, 0.8);
        assert_eq!(settings.supported_case_types.len(), 8);
        assert_eq!(settings.esphome_hostname, "shell-sorter-controller.local");
        assert!(!settings.auto_detect_cameras);
        assert!(settings.auto_start_esp32_cameras);
    }

    #[test]
    fn test_camera_config_default() {
        let config = CameraConfig::default();
        assert!(config.view_type.is_none());
        assert!(config.region_x.is_none());
        assert!(config.region_y.is_none());
        assert!(config.region_width.is_none());
        assert!(config.region_height.is_none());
    }

    #[test]
    fn test_user_config_default() {
        let config = UserConfig::default();
        assert!(config.camera_configs.is_empty());
        assert_eq!(config.network_camera_hostnames.len(), 1);
        assert_eq!(config.network_camera_hostnames[0], "esp32cam1.local");
        assert!(!config.auto_detect_cameras);
    }

    #[test]
    fn test_user_config_camera_operations() {
        let mut config = UserConfig::default();
        let camera_name = "test_camera";
        let camera_config = CameraConfig {
            view_type: Some(ViewType::Side),
            region_x: Some(100),
            region_y: Some(200),
            region_width: Some(300),
            region_height: Some(400),
            ..Default::default()
        };

        // Test set and get
        config.set_camera_config(camera_name.to_string(), camera_config.clone());
        let retrieved_config = config.get_camera_config(camera_name);
        assert!(matches!(retrieved_config.view_type, Some(ViewType::Side)));
        assert_eq!(retrieved_config.region_x, Some(100));

        // Test clear
        config.clear_camera_config(camera_name);
        let default_config = config.get_camera_config(camera_name);
        assert!(default_config.view_type.is_none());
    }

    #[test]
    fn test_serialization() {
        let settings = Settings::default();
        let json = serde_json::to_string(&settings).unwrap();
        let deserialized: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(settings.host, deserialized.host);
        assert_eq!(settings.port, deserialized.port);
    }
}
