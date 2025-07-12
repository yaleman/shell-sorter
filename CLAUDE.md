## Project Overview

This application controls an ammunition shell case sorting machine that uses
computer vision and machine learning to automatically identify and sort
different types of shell cases.

## Development Guidelines

- It is mandatory that 'just check' finishes without warnings or errors before
  considering a task complete
- It is mandatory that the final step in completing a task is that all changes
  are commited to git
- Any time the design or implementation changes, CLAUDE.md must be updated
- It is mandatory that README.md is kept up to date with system design, hardware
  requirements, and setup instructions
- Never allow a bare "type: ignore" comment
- Never use global variables
- Never use inline javascript or css in a web page unless there's no other way
  to solve the problem
- If you're thinking about implementing backwards compatibility, check with the
  user first
- Never use `std::env::set_var`
- Unsafe code is a last resort, ask the user before continuing if that's the
  solution
- Use the tracing crate for logging in Rust, with the debug CLI flag enabling
  debug logging, and the default logging level set to "info"
- If you use unwrap or expect in production code, you have failed and will be
  terminated.

## Architecture

### Core Components (Rust Implementation)

1. **Axum Web Server** (`src/server.rs`)
   - Primary web server implementation using Axum framework
   - REST API for machine control, camera management, and configuration
   - Web dashboard with HTML template rendering using Askama
   - MJPEG video streaming endpoints for both USB and ESPHome cameras
   - Comprehensive API endpoints for shell data, ML training, and case type management
   - No-cache middleware to prevent browser caching issues
   - Static file serving for frontend assets

2. **USB Camera Controller** (`src/usb_camera_controller.rs`)
   - Hardware-based USB camera identification using vendor/product IDs and serial numbers
   - Stable camera mapping that persists across system reboots and device reordering
   - Cross-platform camera support (Linux V4L2, Windows MediaFoundation, macOS AVFoundation)
   - Real-time MJPEG streaming with optimized frame capture for web browsers
   - Camera format detection and configuration
   - Thread-safe async communication using mpsc channels
   - Error recovery and consecutive failure tracking

3. **ESPHome Camera Manager** (`src/camera_manager.rs`)
   - Network camera discovery and management for ESPHome devices
   - HTTP-based camera operations (detection, streaming, image capture)
   - Probes configured hostnames to detect available ESPHome cameras
   - Camera selection and streaming control
   - Thread-based architecture with async task communication
   - Integrated with web server for REST API access

4. **Hardware Controller Monitor** (`src/controller_monitor.rs`)
   - ESPHome hardware controller communication via HTTP API
   - Health monitoring with continuous status checks and automatic retry logic
   - Machine control command interface (next case, sensor readings, hardware status)
   - Sensor monitoring (case ready, case in camera view)
   - Thread-safe operation with proper error handling and status reporting

5. **Configuration Management** (`src/config.rs`)
   - Rust-based configuration using Serde for serialization
   - User configuration persistence in `~/.config/shell-sorter.json`
   - Environment variable support with `SHELL_SORTER_` prefix
   - Automatic directory creation for data storage
   - Settings validation and defaults
   - Runtime configuration updates for camera and controller settings

6. **Error Handling** (`src/error.rs`)
   - Comprehensive error types using thiserror
   - Proper error propagation throughout the application
   - No panics in production code (unwrap/expect forbidden)
   - Graceful error recovery and logging

7. **CLI Interface** (`src/main.rs`)
   - Command-line interface using Clap for argument parsing
   - Machine control commands (next-case, status, sensors)
   - Camera operations (detect, list, capture, streaming)
   - Data management (shells, case types, ML training)
   - Configuration management
   - Web server launcher with configurable host/port

### Legacy Python Components (shell_sorter/)

**Note**: The Python implementation is legacy and being replaced by Rust. The following components exist for reference but are no longer actively used:

- **FastAPI Web Application** (`shell_sorter/app.py`) - Replaced by Rust Axum server
- **Camera Management** (`shell_sorter/camera_manager.py`) - Replaced by Rust implementation
- **Hardware Controller** (`shell_sorter/hardware_controller.py`) - Replaced by Rust implementation
- **Configuration** (`shell_sorter/config.py`) - Replaced by Rust implementation
- **ML Training** (`shell_sorter/ml_trainer.py`) - TODO: Port to Rust
- **Shell Data Models** (`shell_sorter/shell.py`) - TODO: Port to Rust

### ESPHome Hardware Configuration

8. **ESPHome Controller** (`esphome-shell-sorter.yaml`)
   - ESP32-based hardware controller configuration
   - Two binary sensors for case detection (ready-to-feed, camera view)
   - Manual trigger button with automatic vibration sequence
   - Template switches for servo position control (home/feed/camera/drop positions)
   - Number sliders for fine servo control (0-100% positioning)
   - Vibration motor with template switch control
   - Test sequence button for system validation
   - Web server with HTTP API for remote control
   - Network communication over WiFi with fallback AP mode

9. **ESPHome Camera Devices** (`esphome-esp32cam1.yaml`)
   - ESP32-CAM based network camera configuration
   - MJPEG streaming support with configurable quality and resolution
   - HTTP snapshot endpoints for single frame capture
   - WiFi connectivity with fallback AP mode
   - Integration with main controller for coordinated operations

### Directory Structure

```
src/                         # Rust implementation (primary)
├── main.rs                  # CLI interface and application entry point
├── server.rs                # Axum web server with REST API and streaming
├── usb_camera_controller.rs # USB camera management with hardware identification
├── camera_manager.rs        # ESPHome camera management
├── controller_monitor.rs    # Hardware controller communication
├── config.rs                # Configuration management
├── error.rs                 # Error handling and types
└── lib.rs                   # Library exports

shell_sorter/                # Legacy Python implementation
├── app.py                   # FastAPI application (replaced by Rust)
├── camera_manager.py        # Camera management (replaced by Rust)
├── hardware_controller.py   # Hardware control (replaced by Rust)
├── config.py                # Configuration (replaced by Rust)
├── ml_trainer.py            # ML training (TODO: port to Rust)
├── shell.py                 # Data models (TODO: port to Rust)
├── static/                  # Frontend assets (CSS, JavaScript)
│   ├── style.css            # Main stylesheet
│   ├── script.js            # Camera management and UI logic
│   ├── config.js            # Configuration page logic
│   ├── ml_training.js       # ML training interface
│   └── region_selection.js  # Camera region selection
└── templates/               # HTML templates (used by Rust via Askama)
    ├── dashboard.html       # Main interface
    ├── config.html          # Configuration page
    ├── ml_training.html     # ML training interface
    └── shell_edit.html      # Shell data editing

templates/                   # Rust template directory (Askama)
├── dashboard.html           # Main dashboard template
└── config.html             # Configuration template

data/                        # Application data storage
├── models/                  # Trained ML models
├── images/                  # Training images organized by case type
├── references/              # Reference images for case types
├── composites/              # Generated composite images for training
└── *.json                   # Case type and session metadata

images/                      # Captured shell case images from cameras
├── *_camera_*.jpg          # Camera capture images with session/camera IDs
└── *_metadata.json         # Camera region metadata for capture sessions

tests/                       # Test suite
├── integration_tests.rs    # Rust integration tests
├── test_api.py             # Python API tests (legacy)
├── test_camera_manager.py  # Python camera tests (legacy)
├── test_config.py          # Python config tests (legacy)
└── conftest.py             # Pytest fixtures (legacy)

Configuration Files:
├── ~/.config/shell-sorter.json    # User configuration persistence
├── esphome-shell-sorter.yaml      # Main ESPHome controller
├── esphome-esp32cam1.yaml         # ESPHome camera device
├── Cargo.toml                     # Rust dependencies and project config
├── justfile                       # Build and development commands
└── pyproject.toml                 # Python dependencies (legacy)
```

## Implementation Status

### Completed Features ✅

#### Core Infrastructure
- **Rust Web Server**: Complete Axum-based web server with REST API
- **USB Camera Management**: Hardware-based identification and MJPEG streaming
- **ESPHome Camera Integration**: Network camera discovery and streaming
- **Hardware Controller Communication**: ESPHome device control and monitoring
- **Configuration Management**: Persistent settings with runtime updates
- **Video Streaming**: Real-time MJPEG streams for both USB and ESPHome cameras
- **Frontend Integration**: Working dashboard with camera feeds and controls
- **Error Handling**: Comprehensive error types with proper propagation
- **Testing**: Integration tests and CI/CD pipeline

#### Camera Operations
- Hardware-based camera identification using USB VID/PID/Serial
- Cross-platform support (Linux V4L2, Windows MediaFoundation, macOS AVFoundation)
- Camera detection, selection, and streaming control
- Real-time video streaming with optimized frame rates
- Camera format detection and configuration
- Stable camera mapping across system reboots

#### Web Interface
- Dashboard with live camera feeds
- Camera management (detection, selection, streaming)
- Configuration interface for hardware settings
- Machine control interface
- Real-time status monitoring

### TODO List - Unimplemented Features

#### Web API Endpoints (src/server.rs)
- **Camera Region Management**:
  - `set_camera_view_type()` - Set camera view type (line 810)
  - `set_camera_region()` - Set camera region for focusing (line 828)
  - `clear_camera_region()` - Clear camera region settings (line 836)

- **Shell Data Management**:
  - `list_shells()` - List captured shell data (line 843)
  - `save_shell_data()` - Save shell capture session data (line 849)
  - `toggle_shell_training()` - Toggle training flag for shell data (line 857)

- **Machine Learning**:
  - `ml_list_shells()` - List shells for ML training (line 864)
  - `generate_composites()` - Generate composite training images (line 870)
  - `create_case_type()` - Create new ammunition case types (line 904)
  - `train_model()` - Trigger ML model training (line 909)

- **Configuration Updates**:
  - Camera manager configuration updates when hostnames change (line 965)

#### CLI Commands (src/main.rs)
- **Machine Control**:
  - Machine control commands implementation (line 248)
  - Status check implementation (line 253)
  - Sensor reading implementation (line 260)
  - Flash control implementation (line 265)

- **Data Management**:
  - Image capture CLI commands (line 410)
  - Camera streaming CLI commands (line 416)
  - Shell listing implementation (line 593)
  - Image tagging implementation (line 599)
  - Data export implementation (line 604)
  - Data import implementation (line 609)

- **ML Training**:
  - Case type listing (line 619)
  - Case type addition (line 627)
  - Composite generation (line 633)
  - Model training (line 639)

- **Configuration**:
  - Config setting implementation (line 660)
  - Config reset implementation (line 665)

#### USB Camera Controller (src/usb_camera_controller.rs)
- **Platform-specific Hardware ID Extraction**:
  - Implement proper USB device enumeration for reliable hardware identification (line 458)
  - Extract actual vendor/product IDs and serial numbers from system APIs
  - Improve camera stability and identification across platforms

#### Data Models and ML (Python → Rust Migration Needed)
- **Shell Data Models**: Port from `shell_sorter/shell.py` to Rust
- **ML Training Pipeline**: Port from `shell_sorter/ml_trainer.py` to Rust
- **Case Type Management**: Port case type system to Rust
- **Training Data Organization**: Port training image management to Rust

## Migration Notes

- **Primary Implementation**: Rust-based with Axum web server
- **Legacy Code**: Python FastAPI implementation exists but is deprecated
- **Frontend Assets**: Shared between Rust and Python (served by Rust)
- **Data Storage**: Compatible between Python and Rust implementations
- **ESPHome Integration**: Fully migrated to Rust with feature parity
- **Next Priority**: ML training pipeline migration from Python to Rust
