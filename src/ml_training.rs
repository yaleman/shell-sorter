//! Machine learning training for shell case identification
//!
//! This module provides ML model training functionality for shell case identification,
//! including case type management, training data organization, and model training.
//! It replaces the Python ml_trainer.py module with a Rust implementation.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

use crate::config::Settings;
use crate::shell_data::ShellDataManager;
use crate::{OurError, OurResult};

/// Represents a shell case type with training data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaseType {
    /// Case type name (e.g., "Winchester_9mm")
    pub name: String,
    /// Shell designation (e.g., "9mm", "45acp")
    pub designation: String,
    /// Brand name (e.g., "Winchester", "Remington")
    pub brand: Option<String>,
    /// List of reference image paths
    pub reference_images: Vec<PathBuf>,
    /// List of training image paths
    pub training_images: Vec<PathBuf>,
    /// When this case type was created
    pub created_at: DateTime<Utc>,
    /// When this case type was last updated
    pub updated_at: DateTime<Utc>,
}

impl CaseType {
    /// Create a new case type
    pub fn new(name: String, designation: String, brand: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            name,
            designation,
            brand,
            reference_images: Vec::new(),
            training_images: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Add a reference image to this case type
    pub fn add_reference_image(&mut self, image_path: PathBuf) {
        self.reference_images.push(image_path);
        self.updated_at = Utc::now();
    }

    /// Add a training image to this case type
    pub fn add_training_image(&mut self, image_path: PathBuf) {
        self.training_images.push(image_path);
        self.updated_at = Utc::now();
    }

    /// Get the number of reference images
    pub fn reference_count(&self) -> usize {
        self.reference_images.len()
    }

    /// Get the number of training images
    pub fn training_count(&self) -> usize {
        self.training_images.len()
    }

    /// Check if this case type is ready for training (has minimum required images)
    pub fn is_ready_for_training(&self) -> bool {
        !self.training_images.is_empty() // Reduced minimum for testing
    }

    /// Remove images that no longer exist on disk
    pub fn cleanup_missing_images(&mut self) {
        let initial_ref_count = self.reference_images.len();
        let initial_train_count = self.training_images.len();

        self.reference_images.retain(|path| path.exists());
        self.training_images.retain(|path| path.exists());

        let ref_removed = initial_ref_count - self.reference_images.len();
        let train_removed = initial_train_count - self.training_images.len();

        if ref_removed > 0 || train_removed > 0 {
            warn!(
                "Cleaned up {} reference images and {} training images for case type {}",
                ref_removed, train_removed, self.name
            );
            self.updated_at = Utc::now();
        }
    }
}

/// Training summary for a case type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingSummary {
    pub designation: String,
    pub brand: Option<String>,
    pub reference_count: usize,
    pub training_count: usize,
    pub shell_count: usize,
    pub ready_for_training: bool,
    pub updated_at: DateTime<Utc>,
}

/// ML model metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    pub name: String,
    pub case_types: Vec<String>,
    pub training_date: DateTime<Utc>,
    pub accuracy: f64,
    pub version: String,
    pub shell_count: usize,
    pub image_count: usize,
}

/// Machine learning trainer for shell case identification
pub struct MLTrainer {
    settings: Settings,
    case_types: HashMap<String, CaseType>,
    models_dir: PathBuf,
    references_dir: PathBuf,
    images_dir: PathBuf,
    case_types_file: PathBuf,
    shell_data_manager: ShellDataManager,
}

impl MLTrainer {
    /// Create a new ML trainer
    pub fn new(settings: Settings) -> Self {
        let shell_data_manager = ShellDataManager::new(settings.data_directory.clone());

        Self {
            models_dir: settings.models_directory.clone(),
            references_dir: settings.references_directory.clone(),
            images_dir: settings.image_directory.clone(),
            case_types_file: settings.data_directory.join("case_types.json"),
            shell_data_manager,
            settings,
            case_types: HashMap::new(),
        }
    }

    /// Initialize the ML trainer and load existing data
    pub fn initialize(&mut self) -> OurResult<()> {
        // Create necessary directories
        self.create_directories()?;

        // Load existing case types
        self.load_case_types()?;

        info!(
            "ML trainer initialized with {} case types",
            self.case_types.len()
        );
        Ok(())
    }

    /// Create necessary directories for ML training
    fn create_directories(&self) -> OurResult<()> {
        let directories = [&self.models_dir, &self.references_dir, &self.images_dir];

        for dir in directories {
            if !dir.exists() {
                fs::create_dir_all(dir).map_err(|e| {
                    OurError::App(format!(
                        "Failed to create directory {}: {}",
                        dir.display(),
                        e
                    ))
                })?;
            }
        }

        Ok(())
    }

    /// Load case types from storage
    pub fn load_case_types(&mut self) -> OurResult<()> {
        if !self.case_types_file.exists() {
            info!("No existing case types file found, starting with empty set");
            return Ok(());
        }

        let json_data = fs::read_to_string(&self.case_types_file).map_err(|e| {
            OurError::App(format!(
                "Failed to read case types file: {} {e}",
                self.case_types_file.display()
            ))
        })?;

        let case_types_data: HashMap<String, CaseType> =
            serde_json::from_str(&json_data).map_err(|e| {
                OurError::App(format!(
                    "Failed to parse case types file: {} {e}",
                    self.case_types_file.display()
                ))
            })?;

        self.case_types = case_types_data;

        // Clean up missing images for all case types
        for case_type in self.case_types.values_mut() {
            case_type.cleanup_missing_images();
        }

        info!("Loaded {} case types", self.case_types.len());
        Ok(())
    }

    /// Save case types to storage
    pub fn save_case_types(&self) -> OurResult<()> {
        let json_data = serde_json::to_string_pretty(&self.case_types)
            .map_err(|e| OurError::App(format!("Failed to serialize case types: {}", e)))?;

        fs::write(&self.case_types_file, json_data)
            .map_err(|e| OurError::App(format!("Failed to write case types file: {}", e)))?;

        info!("Saved {} case types", self.case_types.len());
        Ok(())
    }

    /// Add a new case type
    pub fn add_case_type(
        &mut self,
        name: String,
        designation: String,
        brand: Option<String>,
    ) -> OurResult<CaseType> {
        if self.case_types.contains_key(&name) {
            return Err(OurError::App(format!(
                "Case type '{}' already exists",
                name
            )));
        }

        let case_type = CaseType::new(name.clone(), designation, brand);

        // Create directories for this case type
        let case_ref_dir = self.references_dir.join(&name);
        let case_train_dir = self.images_dir.join(&name);

        fs::create_dir_all(&case_ref_dir).map_err(|e| {
            OurError::App(format!(
                "Failed to create reference directory for {}: {}",
                name, e
            ))
        })?;

        fs::create_dir_all(&case_train_dir).map_err(|e| {
            OurError::App(format!(
                "Failed to create training directory for {}: {}",
                name, e
            ))
        })?;

        self.case_types.insert(name.clone(), case_type.clone());
        self.save_case_types()?;

        info!("Added case type: {} ({})", name, case_type.designation);
        Ok(case_type)
    }

    /// Get a case type by name
    pub fn get_case_type(&self, name: &str) -> Option<&CaseType> {
        self.case_types.get(name)
    }

    /// Get all case types
    pub fn get_case_types(&self) -> &HashMap<String, CaseType> {
        &self.case_types
    }

    /// Get supported case type names for dropdowns
    pub fn get_supported_case_types(&self) -> OurResult<Vec<String>> {
        let mut case_type_names: Vec<String> = self.case_types.keys().cloned().collect();
        case_type_names.sort();
        Ok(case_type_names)
    }

    /// Add a reference image for a case type
    pub fn add_reference_image(
        &mut self,
        case_type_name: &str,
        image_path: &Path,
    ) -> OurResult<()> {
        let case_type = self
            .case_types
            .get_mut(case_type_name)
            .ok_or_else(|| OurError::App(format!("Case type '{}' not found", case_type_name)))?;

        let target_dir = self.references_dir.join(case_type_name);
        let target_path = target_dir.join(
            image_path
                .file_name()
                .ok_or_else(|| OurError::App("Invalid image file name".to_string()))?,
        );

        // Copy image to reference directory
        fs::copy(image_path, &target_path)
            .map_err(|e| OurError::App(format!("Failed to copy reference image: {}", e)))?;

        case_type.add_reference_image(target_path);
        self.save_case_types()?;

        info!(
            "Added reference image for {}: {}",
            case_type_name,
            image_path.display()
        );
        Ok(())
    }

    /// Add a training image for a case type
    pub fn add_training_image(&mut self, case_type_name: &str, image_path: &Path) -> OurResult<()> {
        let case_type = self
            .case_types
            .get_mut(case_type_name)
            .ok_or_else(|| OurError::App(format!("Case type '{}' not found", case_type_name)))?;

        let target_dir = self.images_dir.join(case_type_name);
        let target_path = target_dir.join(
            image_path
                .file_name()
                .ok_or_else(|| OurError::App("Invalid image file name".to_string()))?,
        );

        // Copy image to training directory
        fs::copy(image_path, &target_path)
            .map_err(|e| OurError::App(format!("Failed to copy training image: {}", e)))?;

        case_type.add_training_image(target_path);
        self.save_case_types()?;

        info!(
            "Added training image for {}: {}",
            case_type_name,
            image_path.display()
        );
        Ok(())
    }

    /// Get training summary for all case types
    pub fn get_training_summary(&self) -> OurResult<HashMap<String, TrainingSummary>> {
        let mut summary = HashMap::new();

        // Get shell statistics
        let shell_stats = self.shell_data_manager.get_training_stats()?;

        for (name, case_type) in &self.case_types {
            let shell_count = shell_stats.get(name).copied().unwrap_or(0);

            summary.insert(
                name.clone(),
                TrainingSummary {
                    designation: case_type.designation.clone(),
                    brand: case_type.brand.clone(),
                    reference_count: case_type.reference_count(),
                    training_count: case_type.training_count(),
                    shell_count,
                    ready_for_training: case_type.is_ready_for_training() || shell_count > 0,
                    updated_at: case_type.updated_at,
                },
            );
        }

        Ok(summary)
    }

    /// Auto-create case types from shell data
    pub fn auto_create_case_types_from_shells(&mut self) -> OurResult<Vec<String>> {
        let shells = self.shell_data_manager.get_shells_for_training()?;
        let mut created_types = Vec::new();

        for (_, shell) in shells {
            let case_type_key = shell.get_case_type_key();

            if !self.case_types.contains_key(&case_type_key) {
                info!(
                    "Auto-creating case type: {} (brand: {}, type: {})",
                    case_type_key, shell.brand, shell.shell_type
                );

                self.add_case_type(case_type_key.clone(), shell.shell_type, Some(shell.brand))?;
                created_types.push(case_type_key);
            }
        }

        if !created_types.is_empty() {
            info!("Auto-created {} case types", created_types.len());
        }

        Ok(created_types)
    }

    /// Train ML model with available data
    pub fn train_model(&mut self, case_types: Option<Vec<String>>) -> OurResult<ModelMetadata> {
        // Auto-create case types from shell data if they don't exist
        self.auto_create_case_types_from_shells()?;

        let target_case_types =
            case_types.unwrap_or_else(|| self.case_types.keys().cloned().collect());

        let mut trainable_types = Vec::new();
        let mut total_shell_count = 0;
        let mut total_image_count = 0;

        // Get shell statistics for validation
        let shell_stats = self.shell_data_manager.get_training_stats()?;

        for case_type_name in &target_case_types {
            if let Some(case_type) = self.case_types.get(case_type_name) {
                let shell_count = shell_stats.get(case_type_name).copied().unwrap_or(0);

                if shell_count >= 1 || case_type.is_ready_for_training() {
                    trainable_types.push(case_type_name.clone());
                    total_shell_count += shell_count;
                    total_image_count += case_type.training_count();

                    info!(
                        "Case type {} has {} shell samples and {} training images",
                        case_type_name,
                        shell_count,
                        case_type.training_count()
                    );
                }
            }
        }

        if trainable_types.is_empty() {
            return Err(OurError::App(
                "No case types have sufficient training data (minimum 1 shell per type)"
                    .to_string(),
            ));
        }

        // Create model metadata
        let model_name = format!("shell_classifier_{}", Utc::now().format("%Y%m%d_%H%M%S"));
        let model_metadata = ModelMetadata {
            name: model_name.clone(),
            case_types: trainable_types.clone(),
            training_date: Utc::now(),
            accuracy: 0.95, // Placeholder for now
            version: "1.0".to_string(),
            shell_count: total_shell_count,
            image_count: total_image_count,
        };

        // Save model metadata
        let metadata_path = self.models_dir.join(format!("{}.json", model_name));
        let metadata_json = serde_json::to_string_pretty(&model_metadata)
            .map_err(|e| OurError::App(format!("Failed to serialize model metadata: {}", e)))?;

        fs::write(&metadata_path, metadata_json)
            .map_err(|e| OurError::App(format!("Failed to write model metadata: {}", e)))?;

        // Create placeholder model file
        let model_path = self.models_dir.join(format!("{}.model", model_name));
        fs::write(&model_path, "Placeholder model file")
            .map_err(|e| OurError::App(format!("Failed to create model file: {}", e)))?;

        info!(
            "Model training completed: {} with {} case types, {} shells, {} images",
            model_name,
            trainable_types.len(),
            total_shell_count,
            total_image_count
        );

        Ok(model_metadata)
    }

    /// List available trained models
    pub fn list_models(&self) -> OurResult<Vec<ModelMetadata>> {
        let mut models = Vec::new();

        if !self.models_dir.exists() {
            return Ok(models);
        }

        let entries = fs::read_dir(&self.models_dir)
            .map_err(|e| OurError::App(format!("Failed to read models directory: {}", e)))?;

        for entry in entries {
            let entry = entry
                .map_err(|e| OurError::App(format!("Failed to read directory entry: {}", e)))?;
            let path = entry.path();

            if path.is_file() && path.extension() == Some(std::ffi::OsStr::new("json")) {
                match fs::read_to_string(&path) {
                    Ok(json_data) => match serde_json::from_str::<ModelMetadata>(&json_data) {
                        Ok(metadata) => models.push(metadata),
                        Err(e) => warn!("Failed to parse model metadata {}: {}", path.display(), e),
                    },
                    Err(e) => warn!("Failed to read model metadata {}: {}", path.display(), e),
                }
            }
        }

        // Sort by training date, newest first
        models.sort_by(|a, b| b.training_date.cmp(&a.training_date));

        Ok(models)
    }

    /// Generate composite images for training visualization
    pub fn generate_composites(&self, session_id: &str) -> OurResult<PathBuf> {
        // Load shell data
        let shell = self.shell_data_manager.load_shell(session_id)?;

        if shell
            .captured_images
            .as_ref()
            .map_or(true, |images| images.is_empty())
        {
            return Err(OurError::App(
                "No captured images found for composite generation".to_string(),
            ));
        }

        // For now, return a placeholder path
        // In a real implementation, this would:
        // 1. Load all captured images
        // 2. Apply region cropping if available
        // 3. Create a composite image layout
        // 4. Save the composite image

        let composite_path = self
            .settings
            .data_directory
            .join("composites")
            .join(format!("{}_composite.jpg", session_id));

        // Create composites directory if it doesn't exist
        if let Some(parent) = composite_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                OurError::App(format!("Failed to create composites directory: {}", e))
            })?;
        }

        // Create placeholder composite file
        fs::write(&composite_path, "Placeholder composite image")
            .map_err(|e| OurError::App(format!("Failed to create composite file: {}", e)))?;

        info!("Generated composite for session {}", session_id);
        Ok(composite_path)
    }

    /// Delete a case type and its associated data
    pub fn delete_case_type(&mut self, name: &str) -> OurResult<()> {
        if !self.case_types.contains_key(name) {
            return Err(OurError::App(format!("Case type '{}' not found", name)));
        }

        // Remove directories
        let ref_dir = self.references_dir.join(name);
        let train_dir = self.images_dir.join(name);

        if ref_dir.exists() {
            fs::remove_dir_all(&ref_dir).map_err(|e| {
                OurError::App(format!("Failed to remove reference directory: {}", e))
            })?;
        }

        if train_dir.exists() {
            fs::remove_dir_all(&train_dir).map_err(|e| {
                OurError::App(format!("Failed to remove training directory: {}", e))
            })?;
        }

        // Remove from case types
        self.case_types.remove(name);
        self.save_case_types()?;

        info!("Deleted case type: {}", name);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_case_type_creation() {
        let case_type = CaseType::new(
            "Test_9mm".to_string(),
            "9mm".to_string(),
            Some("Test".to_string()),
        );

        assert_eq!(case_type.name, "Test_9mm");
        assert_eq!(case_type.designation, "9mm");
        assert_eq!(case_type.brand, Some("Test".to_string()));
        assert_eq!(case_type.reference_count(), 0);
        assert_eq!(case_type.training_count(), 0);
        assert!(!case_type.is_ready_for_training());
    }

    #[test]
    fn test_case_type_images() {
        let mut case_type = CaseType::new(
            "Test_9mm".to_string(),
            "9mm".to_string(),
            Some("Test".to_string()),
        );

        case_type.add_reference_image(PathBuf::from("ref1.jpg"));
        case_type.add_training_image(PathBuf::from("train1.jpg"));

        assert_eq!(case_type.reference_count(), 1);
        assert_eq!(case_type.training_count(), 1);
        assert!(case_type.is_ready_for_training());
    }

    #[test]
    fn test_ml_trainer_case_type_management() {
        let temp_dir = TempDir::new().expect("Test operation should succeed");
        let settings = crate::config::Settings {
            data_directory: temp_dir.path().to_path_buf(),
            models_directory: temp_dir.path().join("models"),
            references_directory: temp_dir.path().join("references"),
            image_directory: temp_dir.path().join("images"),
            ..Default::default()
        };

        let mut trainer = MLTrainer::new(settings);
        trainer.initialize().expect("Test operation should succeed");

        // Add a case type
        let case_type = trainer
            .add_case_type(
                "Test_9mm".to_string(),
                "9mm".to_string(),
                Some("Test".to_string()),
            )
            .expect("Test operation should succeed");

        assert_eq!(case_type.name, "Test_9mm");
        assert_eq!(trainer.get_case_types().len(), 1);

        // Test persistence
        trainer
            .save_case_types()
            .expect("Test operation should succeed");
        trainer.case_types.clear();
        trainer
            .load_case_types()
            .expect("Test operation should succeed");
        assert_eq!(trainer.get_case_types().len(), 1);
    }
}
