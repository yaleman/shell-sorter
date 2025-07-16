# Shell Sorter: Complete Python to Rust Migration Plan

## Executive Summary

This document outlines a comprehensive plan to migrate all remaining Python functionality to Rust, achieving a single-language codebase while maintaining all existing features and improving performance, type safety, and maintainability.

## Current Migration Status

### ✅ Already Migrated to Rust
- **Web Server**: FastAPI → Axum with Askama templates
- **Camera Management**: USB and ESPHome camera control
- **Hardware Controller**: ESPHome device communication  
- **Configuration Management**: Persistent settings with JSON storage
- **Basic API Endpoints**: Camera detection, selection, streaming
- **Frontend Assets**: CSS, JavaScript, and static files (shared)

### 🔄 Partially Migrated
- **Templates**: HTML templates exist but some routes not implemented
- **API Endpoints**: Core camera functionality complete, ML/shell management missing

### ❌ Needs Migration
- **ML Training System**: Complete ML pipeline and model management
- **Shell Data Management**: Data models, CRUD operations, and persistence
- **Advanced API Endpoints**: Shell editing, tagging, training, composites
- **CLI Commands**: Shell data operations, ML training commands
- **Image Processing**: Region selection, composite generation, image manipulation

## Detailed Analysis

### 1. Data Models Migration

#### Python Models (`shell_sorter/shell.py`)
```python
class ViewType(StrEnum):
    SIDE = "side"
    TAIL = "tail" 
    UNKNOWN = "unknown"

class CameraRegion(BaseModel):
    view_type: ViewType
    region_x: Optional[int]
    region_y: Optional[int] 
    region_width: Optional[int]
    region_height: Optional[int]

class CapturedImage(BaseModel):
    camera_index: int
    filename: str
    camera_name: str
    view_type: ViewType
    region_x: Optional[int]
    region_y: Optional[int]
    region_width: Optional[int] 
    region_height: Optional[int]

class Shell(BaseModel):
    date_captured: datetime
    brand: str
    shell_type: str
    image_filenames: list[str]
    captured_images: Optional[List[CapturedImage]]
    include: bool
```

#### Target Rust Models (`src/shell_data.rs`)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ViewType {
    Side,
    Tail,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraRegion {
    pub view_type: ViewType,
    pub region_x: Option<i32>,
    pub region_y: Option<i32>,
    pub region_width: Option<i32>,
    pub region_height: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shell {
    #[serde(with = "chrono::serde::ts_seconds")]
    pub date_captured: chrono::DateTime<chrono::Utc>,
    pub brand: String,
    pub shell_type: String,
    pub image_filenames: Vec<String>,
    pub captured_images: Option<Vec<CapturedImage>>,
    pub include: bool,
}
```

### 2. ML Training System Migration

#### Python ML System (`shell_sorter/ml_trainer.py`)
- **CaseType**: Shell case type management with training data
- **MLTrainer**: Model training pipeline and data organization
- **Features**: Case type CRUD, reference/training image management, model training

#### Target Rust ML System (`src/ml_training.rs`)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseType {
    pub name: String,
    pub designation: String,
    pub brand: Option<String>,
    pub reference_images: Vec<PathBuf>,
    pub training_images: Vec<PathBuf>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

pub struct MLTrainer {
    settings: Settings,
    case_types: HashMap<String, CaseType>,
    models_dir: PathBuf,
    references_dir: PathBuf,
    images_dir: PathBuf,
    case_types_file: PathBuf,
}

impl MLTrainer {
    pub fn new(settings: Settings) -> Self { /* ... */ }
    pub fn load_case_types(&mut self) -> OurResult<()> { /* ... */ }
    pub fn save_case_types(&self) -> OurResult<()> { /* ... */ }
    pub fn add_case_type(&mut self, name: String, designation: String, brand: Option<String>) -> OurResult<CaseType> { /* ... */ }
    pub fn add_reference_image(&mut self, case_type_name: &str, image_path: PathBuf) -> OurResult<()> { /* ... */ }
    pub fn add_training_image(&mut self, case_type_name: &str, image_path: PathBuf) -> OurResult<()> { /* ... */ }
    pub fn get_training_summary(&self) -> HashMap<String, TrainingSummary> { /* ... */ }
    pub fn train_model(&self, case_types: Option<Vec<String>>) -> OurResult<String> { /* ... */ }
}
```

### 3. Missing API Endpoints

#### Shell Data Management
- `GET /api/ml/shells` - List shells for ML training
- `POST /api/shells/save` - Save shell capture session data  
- `POST /api/ml/shells/{session_id}/toggle` - Toggle training flag
- `POST /api/ml/shells/{session_id}/update` - Update shell data
- `DELETE /api/ml/shells/{session_id}` - Delete shell session
- `DELETE /api/ml/shells/{session_id}/images/{filename}` - Delete shell image

#### Case Type Management  
- `GET /api/case-types` - List case types
- `POST /api/case-types` - Create new case type
- `POST /api/case-types/{name}/reference-image` - Add reference image
- `POST /api/case-types/{name}/training-image` - Add training image

#### ML Training
- `POST /api/train-model` - Start model training
- `POST /api/ml/generate-composites` - Generate composite images
- `GET /api/composites/{session_id}` - Get composite image

#### Camera Region Management
- `POST /api/cameras/{camera_index}/view-type` - Set camera view type
- `POST /api/cameras/{camera_index}/region` - Set camera region
- `DELETE /api/cameras/{camera_index}/region` - Clear camera region
- `POST /api/ml/shells/{session_id}/images/{filename}/region` - Set image region

#### Machine Control (Extended)
- `POST /api/machine/flash/on` - Turn flash on
- `POST /api/machine/flash/off` - Turn flash off  
- `POST /api/machine/flash/capture` - Capture with flash
- `POST /api/machine/photo-with-flash` - Photo session with flash

### 4. Template Integration

#### Existing Templates (Need Route Implementation)
- `ml_training.html` - ML training interface ✅ (exists)
- `shell_edit.html` - Shell data editing ✅ (exists)
- `region_selection.html` - Camera region selection ✅ (exists)
- `tagging.html` - Image tagging interface ✅ (exists)

#### Target Rust Template Routes
```rust
// Template routes
.route("/ml-training", get(ml_training_page))
.route("/shell-edit/{session_id}", get(shell_edit_page))
.route("/region-selection/{camera_index}", get(region_selection_page))
.route("/tagging/{session_id}", get(tagging_page))
```

### 5. Frontend JavaScript Integration

#### Existing JavaScript Files (Reuse As-Is)
- `ml_training.js` - ML training interface logic ✅
- `shell_edit.js` - Shell editing functionality ✅  
- `region_selection.js` - Region selection interface ✅
- `config.js` - Configuration management ✅

## Migration Implementation Plan

### Phase 1: Data Models Foundation (Week 1)
**Priority: Critical**

1. **Create `src/shell_data.rs`**
   - Implement all data models (ViewType, CameraRegion, CapturedImage, Shell)
   - Add serialization/deserialization with proper error handling
   - Include validation and conversion methods

2. **Create `src/ml_training.rs`**
   - Implement CaseType and MLTrainer structures
   - Add case type management functionality
   - Implement training data organization

3. **Update `src/lib.rs`**
   - Export new modules
   - Add error types for ML operations

### Phase 2: Shell Data Management (Week 2)
**Priority: High**

1. **Implement Shell CRUD Operations**
   - Shell session creation and persistence
   - Image capture metadata storage
   - Shell data querying and filtering

2. **Add Shell API Endpoints** (`src/server.rs`)
   ```rust
   .route("/api/ml/shells", get(list_shells))
   .route("/api/shells/save", post(save_shell_data))
   .route("/api/ml/shells/{session_id}/toggle", post(toggle_shell_training))
   .route("/api/ml/shells/{session_id}/update", post(update_shell_data))
   .route("/api/ml/shells/{session_id}", delete(delete_shell_session))
   ```

3. **Implement Shell Templates**
   - `/shell-edit/{session_id}` route
   - `/tagging/{session_id}` route
   - Askama template integration

### Phase 3: Case Type Management (Week 3)
**Priority: High**

1. **Implement Case Type API Endpoints**
   ```rust
   .route("/api/case-types", get(list_case_types))
   .route("/api/case-types", post(create_case_type))
   .route("/api/case-types/{name}/reference-image", post(add_reference_image))
   .route("/api/case-types/{name}/training-image", post(add_training_image))
   ```

2. **Add Image Upload Handling**
   - Multipart form data processing
   - Image validation and storage
   - File organization by case type

### Phase 4: Camera Region Management (Week 4)
**Priority: Medium**

1. **Implement Region API Endpoints**
   ```rust
   .route("/api/cameras/{camera_index}/view-type", post(set_camera_view_type))
   .route("/api/cameras/{camera_index}/region", post(set_camera_region))
   .route("/api/cameras/{camera_index}/region", delete(clear_camera_region))
   ```

2. **Add Region Selection Template**
   - `/region-selection/{camera_index}` route
   - Interactive region selection interface

3. **Update Configuration Persistence**
   - Extend camera config with region data
   - Region storage and retrieval

### Phase 5: ML Training Pipeline (Week 5-6)
**Priority: High**

1. **Implement ML Training API**
   ```rust
   .route("/api/train-model", post(train_model))
   .route("/api/ml/generate-composites", post(generate_composites))
   .route("/api/composites/{session_id}", get(get_composite))
   ```

2. **Add ML Training Template**
   - `/ml-training` route
   - Training progress monitoring
   - Model management interface

3. **Implement Image Processing**
   - Composite image generation
   - Image region extraction
   - Training data preparation

### Phase 6: Extended Machine Control (Week 7)
**Priority: Medium**

1. **Implement Flash Control API**
   ```rust
   .route("/api/machine/flash/on", post(flash_on))
   .route("/api/machine/flash/off", post(flash_off))
   .route("/api/machine/flash/capture", post(flash_capture))
   ```

2. **Add Photo Session Management**
   - Coordinated capture sequences
   - Flash timing control
   - Multi-camera synchronization

### Phase 7: CLI Commands (Week 8)
**Priority: Medium**

1. **Implement Shell Management Commands**
   ```rust
   // In src/main.rs
   shell_commands: ShellCommands {
       list: bool,
       export: Option<PathBuf>,
       import: Option<PathBuf>,
       tag: Option<String>,
   }
   ```

2. **Add ML Training Commands**
   ```rust
   ml_commands: MLCommands {
       list_case_types: bool,
       add_case_type: Option<(String, String)>,
       generate_composites: bool,
       train: Option<Vec<String>>,
   }
   ```

### Phase 8: Image Processing & Utilities (Week 9)
**Priority: Low**

1. **Advanced Image Operations**
   - Region-based cropping
   - Image enhancement
   - Batch processing utilities

2. **Data Export/Import**
   - Shell data export formats
   - Training data packaging
   - Model deployment utilities

## Dependencies Required

### New Rust Dependencies
```toml
[dependencies]
# Image processing
image = "0.24"
imageproc = "0.23"

# Date/time handling  
chrono = { version = "0.4", features = ["serde"] }

# File operations
walkdir = "2.3"
uuid = { version = "1.0", features = ["v4", "serde"] }

# Data processing
csv = "1.2"
zip = "0.6"

# Optional ML dependencies (for future)
# candle = "0.3"  # For actual ML training
# hf-hub = "0.3"  # For model management
```

## Testing Strategy

### Unit Tests
- Data model serialization/deserialization
- ML trainer functionality
- Shell data operations
- API endpoint validation

### Integration Tests  
- End-to-end shell capture workflow
- ML training pipeline
- Template rendering
- File operations

### Migration Validation
- Python vs Rust API compatibility testing
- Data format consistency verification
- Performance benchmark comparison

## Risk Mitigation

### Data Safety
- Backup existing shell data before migration
- Validate data integrity after conversion
- Implement rollback procedures

### Feature Parity
- Comprehensive API endpoint testing
- Frontend functionality verification
- Template rendering validation

### Performance
- Benchmark critical operations
- Monitor memory usage during ML operations
- Optimize image processing pipelines

## Success Criteria

### Functional Requirements
- [ ] All Python API endpoints migrated and functional
- [ ] All HTML templates properly integrated  
- [ ] Shell data CRUD operations working
- [ ] ML training pipeline operational
- [ ] Case type management complete
- [ ] Camera region functionality working
- [ ] CLI commands implemented

### Quality Requirements
- [ ] 100% test coverage for new modules
- [ ] No performance regression vs Python
- [ ] Memory usage within acceptable limits
- [ ] All existing shell data successfully migrated

### Deployment Requirements
- [ ] Single Rust binary deployment
- [ ] Docker image updated to Rust-only
- [ ] Documentation updated
- [ ] Python dependencies removed

## Timeline Summary

| Phase | Duration | Priority | Deliverables |
|-------|----------|----------|--------------|
| 1 | Week 1 | Critical | Data models foundation |
| 2 | Week 2 | High | Shell data management |
| 3 | Week 3 | High | Case type management |
| 4 | Week 4 | Medium | Camera region management |
| 5-6 | Week 5-6 | High | ML training pipeline |
| 7 | Week 7 | Medium | Extended machine control |
| 8 | Week 8 | Medium | CLI commands |
| 9 | Week 9 | Low | Advanced utilities |

**Total Estimated Duration: 9 weeks**

## Next Steps

1. **Create task tracking in TodoWrite tool for Phase 1**
2. **Begin implementation with data models (`src/shell_data.rs`)**
3. **Set up integration testing framework**
4. **Create data backup procedures**
5. **Start with highest priority endpoints first**

This plan ensures a systematic, safe migration that maintains all functionality while achieving the goal of a single-language Rust codebase with improved performance and maintainability.