//! Shell data models and management
//!
//! This module provides data structures for managing shell case information,
//! captured images, and camera regions. It replaces the Python shell.py module
//! with Rust implementations that provide better type safety and performance.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::config::ViewType;
use crate::{OurError, OurResult};

/// Camera region information for image processing
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameraRegion {
    pub view_type: ViewType,
    pub region_x: Option<i32>,
    pub region_y: Option<i32>,
    pub region_width: Option<i32>,
    pub region_height: Option<i32>,
}

impl Default for CameraRegion {
    fn default() -> Self {
        Self {
            view_type: ViewType::Unknown,
            region_x: None,
            region_y: None,
            region_width: None,
            region_height: None,
        }
    }
}

impl CameraRegion {
    /// Create a new camera region with the specified parameters
    pub fn new(
        view_type: ViewType,
        region_x: Option<i32>,
        region_y: Option<i32>,
        region_width: Option<i32>,
        region_height: Option<i32>,
    ) -> Self {
        Self {
            view_type,
            region_x,
            region_y,
            region_width,
            region_height,
        }
    }

    /// Check if this region has complete coordinate data
    pub fn is_complete(&self) -> bool {
        self.region_x.is_some()
            && self.region_y.is_some()
            && self.region_width.is_some()
            && self.region_height.is_some()
    }

    /// Get the region as a tuple (x, y, width, height) if complete
    pub fn as_rect(&self) -> Option<(i32, i32, i32, i32)> {
        if let (Some(x), Some(y), Some(w), Some(h)) = (
            self.region_x,
            self.region_y,
            self.region_width,
            self.region_height,
        ) {
            Some((x, y, w, h))
        } else {
            None
        }
    }
}

/// Information about a captured image including camera and region data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapturedImage {
    pub camera_index: u32,
    pub filename: String,
    pub camera_name: String,
    pub view_type: ViewType,
    pub region_x: Option<i32>,
    pub region_y: Option<i32>,
    pub region_width: Option<i32>,
    pub region_height: Option<i32>,
}

impl CapturedImage {
    /// Create a new captured image record
    pub fn new(
        camera_index: u32,
        filename: String,
        camera_name: String,
        view_type: ViewType,
    ) -> Self {
        Self {
            camera_index,
            filename,
            camera_name,
            view_type,
            region_x: None,
            region_y: None,
            region_width: None,
            region_height: None,
        }
    }

    /// Set the region data for this captured image
    pub fn set_region(&mut self, region: &CameraRegion) {
        self.view_type = region.view_type;
        self.region_x = region.region_x;
        self.region_y = region.region_y;
        self.region_width = region.region_width;
        self.region_height = region.region_height;
    }

    /// Get the region data as a CameraRegion
    pub fn get_region(&self) -> CameraRegion {
        CameraRegion {
            view_type: self.view_type,
            region_x: self.region_x,
            region_y: self.region_y,
            region_width: self.region_width,
            region_height: self.region_height,
        }
    }

    /// Check if this image has complete region data
    pub fn has_complete_region(&self) -> bool {
        self.region_x.is_some()
            && self.region_y.is_some()
            && self.region_width.is_some()
            && self.region_height.is_some()
    }
}

/// Model representing a shell case with metadata and captured images
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Shell {
    /// Date when the shell was captured
    pub date_captured: DateTime<Utc>,
    /// Shell case brand (e.g., "Winchester", "Remington")
    pub brand: String,
    /// Shell case type (e.g., "9mm", "45acp", "223rem")
    pub shell_type: String,
    /// List of image filenames associated with this shell
    pub image_filenames: Vec<String>,
    /// Detailed information about captured images including camera regions
    pub captured_images: Option<Vec<CapturedImage>>,
    /// Whether to include this shell in the training set
    pub include: bool,
}

impl Default for Shell {
    fn default() -> Self {
        Self {
            date_captured: Utc::now(),
            brand: String::new(),
            shell_type: String::new(),
            image_filenames: Vec::new(),
            captured_images: None,
            include: true,
        }
    }
}

impl Shell {
    /// Create a new shell record
    pub fn new(brand: String, shell_type: String) -> Self {
        Self {
            date_captured: Utc::now(),
            brand,
            shell_type,
            image_filenames: Vec::new(),
            captured_images: None,
            include: true,
        }
    }

    /// Add an image filename to this shell
    pub fn add_image(&mut self, filename: String) {
        self.image_filenames.push(filename);
    }

    /// Add a captured image with metadata
    pub fn add_captured_image(&mut self, captured_image: CapturedImage) {
        if let Some(ref mut images) = self.captured_images {
            images.push(captured_image);
        } else {
            self.captured_images = Some(vec![captured_image]);
        }
    }

    /// Get the shell type key for case type management (brand_shell_type)
    pub fn get_case_type_key(&self) -> String {
        format!("{}_{}", self.brand, self.shell_type)
    }

    /// Get the number of captured images
    pub fn image_count(&self) -> usize {
        self.captured_images
            .as_ref()
            .map(|images| images.len())
            .unwrap_or(0)
    }

    /// Check if this shell has images with complete region data
    pub fn has_complete_regions(&self) -> bool {
        self.captured_images
            .as_ref()
            .map(|images| images.iter().all(|img| img.has_complete_region()))
            .unwrap_or(false)
    }

    /// Get images grouped by view type
    pub fn images_by_view_type(&self) -> HashMap<ViewType, Vec<&CapturedImage>> {
        let mut grouped = HashMap::new();
        if let Some(ref images) = self.captured_images {
            for image in images {
                grouped
                    .entry(image.view_type)
                    .or_insert_with(Vec::new)
                    .push(image);
            }
        }
        grouped
    }
}

/// Shell data manager for persistence and CRUD operations
pub struct ShellDataManager {
    data_directory: PathBuf,
}

impl ShellDataManager {
    /// Create a new shell data manager
    pub fn new(data_directory: PathBuf) -> Self {
        Self { data_directory }
    }

    /// Generate a new session ID for shell data
    pub fn generate_session_id() -> String {
        Uuid::new_v4().to_string()
    }

    /// Save shell data to a JSON file with the given session ID
    pub fn save_shell(&self, session_id: &str, shell: &Shell) -> OurResult<()> {
        let file_path = self.data_directory.join(format!("{session_id}.json"));

        // Ensure the data directory exists
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| OurError::App(format!("Failed to create data directory: {e}")))?;
        }

        let json_data = serde_json::to_string_pretty(shell)
            .map_err(|e| OurError::App(format!("Failed to serialize shell data: {e}")))?;

        fs::write(&file_path, json_data)
            .map_err(|e| OurError::App(format!("Failed to write shell data: {e}")))?;

        info!("Saved shell data for session {}", session_id);
        Ok(())
    }

    /// Load shell data from a JSON file
    pub fn load_shell(&self, session_id: &str) -> OurResult<Shell> {
        let file_path = self.data_directory.join(format!("{session_id}.json"));

        if !file_path.exists() {
            return Err(OurError::App(format!(
                "Shell data file not found: {session_id}"
            )));
        }

        let json_data = fs::read_to_string(&file_path)
            .map_err(|e| OurError::App(format!("Failed to read shell data: {e}")))?;

        let shell: Shell = serde_json::from_str(&json_data)
            .map_err(|e| OurError::App(format!("Failed to parse shell data: {e}")))?;

        debug!("Loaded shell data for session {}", session_id);
        Ok(shell)
    }

    /// Get shell data, returning None if not found
    pub fn get_shell(&self, session_id: &str) -> OurResult<Option<Shell>> {
        match self.load_shell(session_id) {
            Ok(shell) => Ok(Some(shell)),
            Err(OurError::App(msg)) if msg.contains("file not found") => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Delete shell data file
    pub fn delete_shell(&self, session_id: &str) -> OurResult<()> {
        let file_path = self.data_directory.join(format!("{session_id}.json"));

        if file_path.exists() {
            fs::remove_file(&file_path)
                .map_err(|e| OurError::App(format!("Failed to delete shell data: {e}")))?;
            info!("Deleted shell data for session {}", session_id);
        } else {
            warn!("Shell data file not found for deletion: {}", session_id);
        }

        Ok(())
    }

    /// List all shell data files
    pub fn list_shells(&self) -> OurResult<Vec<(String, Shell)>> {
        let mut shells = Vec::new();

        if !self.data_directory.exists() {
            return Ok(shells);
        }

        let entries = fs::read_dir(&self.data_directory)
            .map_err(|e| OurError::App(format!("Failed to read data directory: {e}")))?;

        for entry in entries {
            let entry =
                entry.map_err(|e| OurError::App(format!("Failed to read directory entry: {e}")))?;
            let path = entry.path();

            if path.is_file() && path.extension() == Some(std::ffi::OsStr::new("json")) {
                // Skip case_types.json and other non-shell files
                if let Some(file_name) = path.file_stem() {
                    let file_name_str = file_name.to_string_lossy();
                    if file_name_str == "case_types" {
                        continue;
                    }

                    match self.load_shell(&file_name_str) {
                        Ok(shell) => {
                            shells.push((file_name_str.to_string(), shell));
                        }
                        Err(e) => {
                            warn!("Failed to load shell data from {}: {}", path.display(), e);
                        }
                    }
                }
            }
        }

        // Sort by date captured, newest first
        shells.sort_by(|a, b| b.1.date_captured.cmp(&a.1.date_captured));

        info!("Listed {} shell records", shells.len());
        Ok(shells)
    }

    /// Get shells filtered by criteria
    pub fn get_shells_for_training(&self) -> OurResult<Vec<(String, Shell)>> {
        let all_shells = self.list_shells()?;
        let training_shells: Vec<(String, Shell)> = all_shells
            .into_iter()
            .filter(|(_, shell)| shell.include)
            .collect();

        info!("Found {} shells marked for training", training_shells.len());
        Ok(training_shells)
    }

    /// Get training statistics by case type
    pub fn get_training_stats(&self) -> OurResult<HashMap<String, usize>> {
        let training_shells = self.get_shells_for_training()?;
        let mut stats = HashMap::new();

        for (_, shell) in training_shells {
            let case_type_key = shell.get_case_type_key();
            *stats.entry(case_type_key).or_insert(0) += 1;
        }

        Ok(stats)
    }

    /// Update shell data
    pub fn update_shell(&self, session_id: &str, shell: &Shell) -> OurResult<()> {
        // This is the same as save_shell, but explicit for clarity
        self.save_shell(session_id, shell)
    }

    /// Toggle the include flag for a shell
    pub fn toggle_shell_training(&self, session_id: &str) -> OurResult<bool> {
        let mut shell = self.load_shell(session_id)?;
        shell.include = !shell.include;
        self.save_shell(session_id, &shell)?;

        info!(
            "Toggled training flag for session {} to {}",
            session_id, shell.include
        );
        Ok(shell.include)
    }

    /// Check if the data directory exists and is writable
    pub fn validate_data_directory(&self) -> OurResult<()> {
        if !self.data_directory.exists() {
            fs::create_dir_all(&self.data_directory).map_err(|e| {
                OurError::App(format!(
                    "Failed to create data directory {}: {}",
                    self.data_directory.display(),
                    e
                ))
            })?;
        }

        // Try to write a test file to verify permissions
        let test_file = self.data_directory.join(".test_write");
        fs::write(&test_file, "test").map_err(|e| {
            OurError::App(format!(
                "Data directory is not writable {}: {}",
                self.data_directory.display(),
                e
            ))
        })?;
        fs::remove_file(&test_file).ok(); // Clean up test file

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_view_type_serialization() {
        assert_eq!(ViewType::Side.to_string(), "side");
        assert_eq!(ViewType::Tail.to_string(), "tail");
        assert_eq!(ViewType::Unknown.to_string(), "unknown");
    }

    #[test]
    fn test_view_type_parsing() {
        assert_eq!(
            "side"
                .parse::<ViewType>()
                .expect("Test operation should succeed"),
            ViewType::Side
        );
        assert_eq!(
            "TAIL"
                .parse::<ViewType>()
                .expect("Test operation should succeed"),
            ViewType::Tail
        );
        assert!("invalid".parse::<ViewType>().is_err());
    }

    #[test]
    fn test_camera_region_complete() {
        let region = CameraRegion::new(ViewType::Side, Some(10), Some(20), Some(100), Some(200));
        assert!(region.is_complete());
        assert_eq!(region.as_rect(), Some((10, 20, 100, 200)));

        let incomplete = CameraRegion::new(ViewType::Side, Some(10), None, Some(100), Some(200));
        assert!(!incomplete.is_complete());
        assert_eq!(incomplete.as_rect(), None);
    }

    #[test]
    fn test_shell_case_type_key() {
        let shell = Shell::new("Winchester".to_string(), "9mm".to_string());
        assert_eq!(shell.get_case_type_key(), "Winchester_9mm");
    }

    #[test]
    fn test_captured_image_region() {
        let mut image = CapturedImage::new(
            0,
            "test.jpg".to_string(),
            "Camera 1".to_string(),
            ViewType::Side,
        );

        assert!(!image.has_complete_region());

        let region = CameraRegion::new(ViewType::Tail, Some(5), Some(10), Some(50), Some(100));
        image.set_region(&region);

        assert!(image.has_complete_region());
        assert_eq!(image.view_type, ViewType::Tail);
        assert_eq!(image.region_x, Some(5));
    }

    #[test]
    fn test_shell_data_manager() {
        let temp_dir = TempDir::new().expect("Test operation should succeed");
        let manager = ShellDataManager::new(temp_dir.path().to_path_buf());

        let session_id = ShellDataManager::generate_session_id();
        let shell = Shell::new("TestBrand".to_string(), "TestType".to_string());

        // Test save and load
        manager
            .save_shell(&session_id, &shell)
            .expect("Test operation should succeed");
        let loaded_shell = manager
            .load_shell(&session_id)
            .expect("Test operation should succeed");
        assert_eq!(shell.brand, loaded_shell.brand);
        assert_eq!(shell.shell_type, loaded_shell.shell_type);

        // Test list
        let shells = manager
            .list_shells()
            .expect("Test operation should succeed");
        assert_eq!(shells.len(), 1);
        assert_eq!(shells[0].1.brand, "TestBrand");

        // Test toggle training
        let include_flag = manager
            .toggle_shell_training(&session_id)
            .expect("Test operation should succeed");
        assert!(!include_flag); // Should be toggled to false

        // Test delete
        manager
            .delete_shell(&session_id)
            .expect("Test operation should succeed");
        let shells_after_delete = manager
            .list_shells()
            .expect("Test operation should succeed");
        assert_eq!(shells_after_delete.len(), 0);
    }
}
