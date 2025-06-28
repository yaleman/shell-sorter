## Project Overview

This application controls an ammunition shell case sorting machine that uses computer vision and machine learning to automatically identify and sort different types of shell cases.

## Development Guidelines

- It is mandatory that 'just check' finishes without warnings or errors before considering a task complete
- It is mandatory that the final step in completing a task is that all changes are commited to git
- Any time the design or implementation changes, CLAUDE.md must be updated
- It is mandatory that README.md is kept up to date with system design, hardware requirements, and setup instructions
- Never allow a bare "type: ignore" comment
- Never use global variables

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

4. **Hardware Controller** (`shell_sorter/hardware_controller.py`)
   - ESPHome device communication via HTTP API
   - Sensor monitoring (case ready, case in camera view)
   - Servo control (case feeder, case positioning)
   - Vibration motor control for case advancement
   - Complete next-case sequence automation

5. **ESPHome Controller** (`esphome-shell-sorter.yaml`)
   - ESP32-based hardware controller configuration
   - Two binary sensors for case detection
   - One manual trigger button for vibration motor
   - Two servo controls for case manipulation
   - One switch for vibration motor control
   - Web server with HTTP API for remote control
   - Network communication over WiFi

### Directory Structure

```
data/
├── models/          # Trained ML models
├── images/          # Training images organized by case type
├── references/      # Reference images for case types
└── temp/           # Temporary uploads

images/              # Captured shell case images from cameras
esphome-shell-sorter.yaml  # ESPHome hardware controller configuration
```

## System Capabilities

### Machine Control
- Next case advancement via ESPHome controller
- Hardware sensor monitoring (case detection)
- Servo control for case positioning and feeding
- Vibration motor control for case advancement
- Real-time hardware status updates via web interface

### Machine Learning
- Multiple camera setup for shell case imaging
- Case type identification through computer vision
- Support for training custom models with annotated images
- Case types can be identified by:
  - Designation only (e.g., 9mm Parabellum, 38 Special)
  - Designation and brand combination

### Data Management
- Capture images from multiple cameras simultaneously
- Tag captured images with shell case metadata
- Upload and organize reference images
- Manage training datasets per case type
- Automatic model versioning
- Training progress tracking
- Save shell data as JSON with image references

## API Endpoints

### Machine Control
- `GET /` - Web dashboard
- `POST /api/machine/next-case` - Trigger next case sequence
- `GET /api/machine/sensors` - Get hardware sensor status
- `GET /api/machine/hardware-status` - Get ESPHome device status

### Camera Management
- `GET /api/cameras` - Get detected cameras
- `GET /api/cameras/detect` - Detect available cameras
- `POST /api/cameras/select` - Select cameras for use
- `POST /api/cameras/start-selected` - Start selected camera streams
- `POST /api/cameras/stop-all` - Stop all camera streams
- `POST /api/cameras/capture` - Capture images from selected cameras
- `GET /api/cameras/{index}/stream` - Live camera stream

### Shell Data Management
- `GET /tagging/{session_id}` - Shell tagging interface
- `POST /api/shells/save` - Save tagged shell data

### ML Management
- `GET /api/case-types` - List case types and training status
- `POST /api/case-types` - Create new case type
- `POST /api/case-types/{name}/reference-image` - Upload reference image
- `POST /api/case-types/{name}/training-image` - Upload training image
- `POST /api/train-model` - Train ML model

## Configuration

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
- GPIO18: Case ready sensor (binary sensor with pullup)
- GPIO19: Case in camera view sensor (binary sensor with pullup)
- GPIO21: Vibration motor control (digital output)
- GPIO22: Manual vibration trigger button (binary sensor with pullup)
- GPIO16: Case feeder servo (PWM output)
- GPIO17: Case positioning servo (PWM output)

**Network Setup:**
- Device hostname: `shell-sorter-controller.local`
- Web server on port 80 with basic auth (admin/shellsorter)
- WiFi with fallback AP mode for initial configuration

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