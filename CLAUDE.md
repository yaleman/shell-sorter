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
- Never use `std::env::set_var`
- Unsafe code is a last resort, ask the user before continuing if that's the solution
- Use the tracing crate for logging in Rust, with the debug CLI flag enabling debug logging, and the default logging level set to "info"

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

7. **Camera Management System** (`shell_sorter/camera_manager.py`)
   - Hardware-based camera identification using vendor/product IDs and serial numbers
   - Stable camera mapping that persists across system reboots and device reordering
   - Support for both USB cameras and ESPHome network cameras
   - Autofocus control for USB cameras with region-based focusing
   - Camera configuration migration from legacy name-based to hardware ID-based system
   - Real-time frame capture with error recovery and consecutive failure tracking

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

tests/               # Comprehensive test suite
├── test_api.py             # API endpoint testing with mocked camera calls
├── test_camera_manager.py  # Camera management testing without hardware access
├── test_config.py          # Configuration system testing
├── conftest.py            # Pytest fixtures and mocking setup
└── __init__.py            # Test package initialization

~/.config/shell-sorter.json  # Camera configuration persistence
esphome-shell-sorter.yaml   # ESPHome hardware controller configuration
```

## Project Migration Notes

- Python code is legacy and being replaced with Rust