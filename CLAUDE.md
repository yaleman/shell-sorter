## Project Overview

This application controls an ammunition shell case sorting machine that uses computer vision and machine learning to automatically identify and sort different types of shell cases.

## Development Guidelines

- It is mandatory that 'just check' finishes without warnings or errors before considering a task complete
- It is mandatory that the final step in completing a task is that all changes are commited to git
- Any time the design or implementation changes, CLAUDE.md must be updated
- It is mandatory that README.md is kept up to date with system design, hardware requirements, and setup instructions
- Never allow a bare "type: ignore" comment
- Never use global variables
- Never use inline javascript or css in a web page unless there's no other way to solve the problem
- If you're thinking about implementing backwards compatibility, check with the user first

## Architecture

### Core Components

1. **FastAPI Web Application** (`shell_sorter/app.py`)
   - REST API for machine control and monitoring
   - Web dashboard for user interaction
   - Image upload and case type management endpoints
   - Hardware controller integration

2. **Configuration Management** (`shell_sorter/config.py`)
   - Pydantic Settings for environment-based configuration
   - Automatic directory creation for data storage
   - Support for environment variables with `SHELL_SORTER_` prefix

3. **Machine Learning Module** (`shell_sorter/ml_trainer.py`)
   - Case type registration and management
   - Training image organization
   - Model training coordination
   - Reference image management

4. **Shell Data Models** (`shell_sorter/shell.py`)
   - Shell model for training data with metadata
   - CapturedImage model for camera and region information
   - CameraRegion model for region coordinates and view types
   - Support for region-based training data processing

5. **Hardware Controller** (`shell_sorter/hardware_controller.py`)
   - ESPHome device communication via HTTP API
   - Sensor monitoring (case ready, case in camera view)
   - Servo control (case feeder, case positioning)
   - Vibration motor control for case advancement
   - Complete next-case sequence automation

6. **ESPHome Controller** (`esphome-shell-sorter.yaml`)
   - ESP32-based hardware controller configuration
   - Two binary sensors for case detection (ready-to-feed, camera view)
   - Manual trigger button with automatic vibration sequence
   - Template switches for servo position control (home/feed/camera/drop positions)
   - Number sliders for fine servo control (0-100% positioning)
   - Vibration motor with template switch control
   - Test sequence button for system validation
   - Web server with HTTP API for remote control
   - Network communication over WiFi with fallback AP mode

### Directory Structure

```
data/
├── models/          # Trained ML models
├── images/          # Training images organized by case type
├── references/      # Reference images for case types
├── composites/      # Generated composite images for training
└── temp/           # Temporary uploads

images/              # Captured shell case images from cameras
├── *.jpg           # Camera capture images
└── *_metadata.json # Camera region metadata for capture sessions

~/.config/shell-sorter.json  # Camera configuration persistence
esphome-shell-sorter.yaml   # ESPHome hardware controller configuration
```

## System Capabilities

### Machine Control

- Next case advancement via ESPHome controller
- Hardware sensor monitoring (case ready-to-feed, case in camera view)
- Servo control with predefined positions:
  - Case feeder servo: home (0°), neutral (90°), feed (180°) positions
  - Case positioning servo: camera (45°), neutral (90°), drop (135°) positions
- Fine servo control with 0-100% positioning sliders
- Vibration motor control with automatic timing sequences
- Manual vibration trigger with 1-second activation
- Test sequence automation for system validation
- Real-time hardware status updates via web interface

### Machine Learning

- Multiple camera setup for shell case imaging with view type classification
- Camera view types: side_view (profile shots) and tail_view (case end shots)
- Interactive region selection for each camera to focus on shell case areas
- Region overlay display with toggle control for visual feedback
- Case type identification through computer vision
- Support for training custom models with annotated images
- Composite image generation using selected regions for training data
- Circular detection and processing for tail view cameras
- Case types can be identified by:
  - Designation only (e.g., 9mm Parabellum, 38 Special)
  - Designation and brand combination

### Configuration Management

- Web-based configuration interface accessible from dashboard
- Camera management with deletion and configuration reset capabilities
- Auto-start cameras setting for automatic startup when detected  
- Configuration persistence in `~/.config/shell-sorter.json`
- System settings management with confirmation dialogs
- Toast notifications for user feedback on configuration changes

### Data Management

- Capture images from multiple cameras simultaneously with region metadata
- Tag captured images with shell case metadata including camera regions
- Store camera view types and region selections in training data
- Upload and organize reference images
- Manage training datasets per case type
- Automatic model versioning
- Training progress tracking
- Save shell data as JSON with image references and region information

## API Endpoints

### Machine Control

- `GET /` - Web dashboard
- `POST /api/machine/next-case` - Trigger next case sequence
- `GET /api/machine/sensors` - Get hardware sensor status
- `GET /api/machine/hardware-status` - Get ESPHome device status

### Camera Management

- `GET /api/cameras` - Get detected cameras with view types and regions
- `GET /api/cameras/detect` - Detect available cameras
- `POST /api/cameras/select` - Select cameras for use
- `POST /api/cameras/start-selected` - Start selected camera streams
- `POST /api/cameras/stop-all` - Stop all camera streams
- `POST /api/cameras/capture` - Capture images from selected cameras with region metadata
- `GET /api/cameras/{index}/stream` - Live camera stream
- `POST /api/cameras/{index}/view-type` - Set camera view type (side_view/tail_view)
- `GET /region-selection/{index}` - Region selection interface for camera
- `POST /api/cameras/{index}/region` - Save camera region selection
- `DELETE /api/cameras/{index}/region` - Clear camera region selection

### Shell Data Management

- `GET /tagging/{session_id}` - Shell tagging interface
- `POST /api/shells/save` - Save tagged shell data

### ML Management

- `GET /api/ml/shells` - Get all training shells with region data
- `POST /api/ml/shells/{session_id}/toggle` - Toggle shell inclusion in training
- `POST /api/ml/generate-composites` - Generate composite images using region processing
- `GET /api/case-types` - List case types and training status
- `POST /api/case-types` - Create new case type
- `POST /api/case-types/{name}/reference-image` - Upload reference image
- `POST /api/case-types/{name}/training-image` - Upload training image
- `POST /api/train-model` - Train ML model

### Configuration API

- `GET /config` - Configuration management interface
- `GET /api/config` - Get current configuration settings
- `POST /api/config` - Save configuration settings
- `DELETE /api/config/cameras/{camera_index}` - Delete camera configuration
- `DELETE /api/config/cameras` - Clear all camera configurations
- `POST /api/config/reset` - Reset configuration to defaults

## Configuration

### Camera Setup and Region Configuration

Cameras must be configured with view types and regions for optimal training data:

1. **Camera Detection**: Use the "Detect Cameras" button to find available cameras
2. **View Type Assignment**: Set each camera as either:
   - `side_view`: For profile shots of shell cases showing the side/length
   - `tail_view`: For end-on shots showing the case mouth/primer end
3. **Region Selection**: Use the interactive region selection tool to:
   - Draw rectangles around shell case areas to exclude background
   - Ensure consistent framing across captures
   - Focus training data on relevant case features
4. **Region Overlays**: Toggle overlay display to verify region selections on live feeds

Camera configurations are automatically saved to `~/.config/shell-sorter.json` and persist across sessions.

### Application Settings

Settings can be configured via environment variables or `.env` file:

```bash
SHELL_SORTER_HOST=0.0.0.0
SHELL_SORTER_PORT=8000
SHELL_SORTER_DEBUG=false
SHELL_SORTER_ML_ENABLED=true
SHELL_SORTER_CONFIDENCE_THRESHOLD=0.8
SHELL_SORTER_CAMERA_COUNT=4
```

### ESPHome Hardware Configuration

The hardware controller requires an ESP32 device flashed with the provided ESPHome configuration:

**Hardware Connections:**

- GPIO18: Case ready-to-feed sensor (binary sensor with pullup, inverted)
- GPIO19: Case in camera view sensor (binary sensor with pullup, inverted)
- GPIO21: Vibration motor control (digital output)
- GPIO22: Manual vibration trigger button (binary sensor with pullup, inverted)
- GPIO16: Case feeder servo (PWM output, 50Hz)
- GPIO17: Case positioning servo (PWM output, 50Hz)

**ESPHome Features:**

- Template switches for servo position control with predefined angles
- Number entities for fine servo control (0-100% positioning)
- Vibration motor switch with restore mode ALWAYS_OFF
- Manual trigger button with automatic 1-second vibration sequence
- Test sequence button for complete system validation
- Servo auto-detach after 2 seconds with 1-second transition time
- Sensor debouncing with delayed_on/delayed_off filters
- Comprehensive logging for all sensor and actuator events

**Network Setup:**

- Device hostname: `shell-sorter-controller.local`
- Web server on port 80 with basic auth (admin/shellsorter)
- WiFi with fallback AP mode ("Shell-Sorter-Fallback") for initial configuration
- Home Assistant API integration with encryption support
- Over-the-air (OTA) updates with password protection

## Running the Application

```bash
# Install dependencies
uv sync

# Run linting and type checking
just check

# Start the application
just run
```

## ESPHome Hardware Setup

```bash
# Start ESPHome dashboard for configuration and monitoring
just esphome
# Dashboard will be available at http://localhost:6052

# Flash configuration to ESP32 device (replace /dev/ttyUSB0 with your device)
just esphome-flash /dev/ttyUSB0
```

The ESPHome dashboard allows you to:

- Edit and validate the configuration
- View device logs in real-time
- Monitor sensor states and control outputs
- Update firmware over-the-air (OTA)
```