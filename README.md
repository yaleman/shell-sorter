# Shell Sorter

An automated ammunition shell case sorting machine that uses computer vision and
machine learning to identify and sort different types of shell cases.

## System Overview

The Shell Sorter is a complete hardware and software solution for automatically
processing and cataloging ammunition shell cases. The system combines:

- **Computer Vision**: Multi-camera setup for high-quality shell case imaging
- **Machine Learning**: Automated case type identification and classification
- **Hardware Control**: ESP32-based controller for mechanical operations
- **Web Interface**: Modern dashboard for system control and monitoring
- **Data Management**: Structured storage of shell data with images

## Architecture

### Software Components

- **FastAPI Web Application**: REST API and web dashboard
- **Hardware Controller**: Async communication with ESP32 via HTTP
- **Camera Manager**: Multi-camera detection and streaming
- **ML Trainer**: Machine learning model training and inference
- **Data Storage**: JSON-based shell data with image references

### Hardware Components

- **ESP32 Controller**: Network-connected hardware controller
- **Multiple USB Cameras**: For shell case imaging from different angles
- **ESPHome Cameras**: Network-connected ESP32-S3 camera modules
- **Case Feeder System**: Automated case positioning and advancement
- **Sensors**: Detection of case positions throughout the system
- **Actuators**: Servo motors and vibration motor for case movement

## Hardware Setup

### ESP32 Controller Wiring

The system requires an ESP32 development board with the following connections:

### ESPHome Camera Setup

The system supports ESPHome-based ESP32-S3 camera modules as network cameras.
Testing has been performed with the
[Freenove ESP32-S3 WROOM Board](https://github.com/Freenove/Freenove_ESP32_S3_WROOM_Board).

#### ESP32-S3 Camera Configuration

Network cameras are automatically detected at startup and can be used alongside
USB cameras:

- **Hostname**: `esp32cam1.local` (configurable in ESPHome YAML)
- **Stream URL**: `http://esp32cam1.local/camera`
- **Web Interface**: Available at `http://esp32cam1.local` (admin/esp32cam)
- **Resolution**: 800x600 (configurable)
- **Integration**: Seamless mixing with USB cameras in the application

#### Features

- **Automatic Detection**: Network cameras are discovered on startup
- **High-Resolution Capture**: Full resolution capture for training data
- **EXIF Metadata**: Camera name and view type stored in image metadata
- **Async Operations**: Non-blocking network operations using aiohttp
- **Region Support**: Same region selection and overlay features as USB cameras

#### Required Components

- ESP32 development board (ESP32-DevKitC or similar)
- 2x Binary sensors (limit switches, proximity sensors, etc.)
- 1x Pushbutton (normally open, momentary)
- 2x Servo motors (standard 3-wire servos)
- 1x Vibration motor with driver circuit
- Breadboard and jumper wires
- External 5V power supply for servos (recommended)

#### GPIO Pin Assignments

```text
ESP32 Pin  | Component              | Connection Notes
-----------+------------------------+------------------------------------------
GPIO18     | Case Ready Sensor      | Binary sensor with internal pullup
GPIO19     | Camera View Sensor     | Binary sensor with internal pullup  
GPIO21     | Vibration Motor        | Digital output (requires driver circuit)
GPIO22     | Manual Trigger Button  | Pushbutton with internal pullup
GPIO16     | Case Feeder Servo      | PWM signal (3.3V logic level)
GPIO17     | Position Servo         | PWM signal (3.3V logic level)
GND        | Common Ground          | Connect to all component grounds
3.3V       | Logic Power            | For sensors and servo signal lines
```

#### Wiring Diagram

```text
ESP32                          Components
-----                          ----------

GPIO18 ----[PULLUP]---- Case Ready Sensor ---- GND
GPIO19 ----[PULLUP]---- Camera View Sensor --- GND
GPIO21 ---- Vibration Motor Driver ---- Vibration Motor
GPIO22 ----[PULLUP]---- Manual Button -------- GND

GPIO16 ---- Case Feeder Servo (Signal)
GPIO17 ---- Position Servo (Signal)

3.3V  ---- Servo Red Wires (or use external 5V supply)
GND   ---- Servo Brown/Black Wires
GND   ---- All component grounds
```

#### Important Notes

- **Servo Power**: While servos can run on 3.3V, they perform better with 5V
  external supply
- **Vibration Motor**: Requires appropriate driver circuit (transistor/relay)
  for higher current
- **Sensor Types**: Any normally-open binary sensors work (limit switches,
  proximity sensors)
- **WiFi Setup**: ESP32 will create fallback AP "Shell-Sorter-Fallback"
  (password: shellsorter123)

### Network Configuration

1. **Initial Setup**: Connect to fallback AP and configure WiFi credentials
2. **Device Access**: After WiFi connection, device available at
   `shell-sorter-controller.local`
3. **Web Interface**: ESPHome web server on port 80 (admin/shellsorter)

## Software Installation

### Prerequisites

- Python 3.12+
- Docker (for ESPHome development)
- USB cameras connected to development machine
- ESP32 flashed with provided configuration

### Installation Steps

```bash
# Clone repository
git clone <repository-url>
cd shell-sorter

# Install dependencies
uv sync

# Run linting and type checking
just check

# Start application
just run
```

The web interface will be available at `http://localhost:8000`

## ESPHome Development

### Flash ESP32 Configuration

```bash
# Start ESPHome dashboard
just esphome
# Open http://localhost:6052 in browser

# Flash main controller configuration to device (replace with your device path)
just esphome-flash /dev/ttyUSB0

# Flash camera module configuration (for ESP32-S3 camera boards)
# Replace with your camera device path
esphome run esphome-esp32cam1.yaml --device /dev/ttyUSB1
```

### Configuration Management

- **Main Controller**: `esphome-shell-sorter.yaml` - Hardware control and
  sensors
- **Camera Module**: `esphome-esp32cam1.yaml` - ESP32-S3 camera configuration
- Edit configurations in ESPHome dashboard or directly in files
- Support for over-the-air (OTA) updates after initial flash
- Network camera devices automatically discovered by the application

## Usage

### Basic Operation

1. **Start System**: Run `just run` to start the web interface
2. **Camera Setup**: Cameras are automatically detected and selected
3. **Hardware Connection**: Ensure ESP32 is connected and reachable
4. **Case Processing**:
   - Load case into feeder system
   - Click "Next Case" to advance case to camera position
   - Click "Capture & Tag Images" to photograph case
   - Fill in shell metadata (brand, type)
   - Save tagged data

### Manual Controls

- **Web Interface**: "Next Case" button for remote operation
- **Physical Button**: Manual trigger button on ESP32 for immediate operation
- **ESPHome Dashboard**: Real-time monitoring and direct hardware control

### Data Management

- **Images**: Stored in `images/` directory with UUID-based filenames
- **Metadata**: JSON files in `data/` directory with shell information
- **Training Data**: Organized by case type for ML model training

## API Reference

### Machine Control API

- `POST /api/machine/next-case` - Trigger complete case advancement sequence
- `GET /api/machine/sensors` - Get real-time sensor status
- `GET /api/machine/hardware-status` - Check ESP32 connectivity

### Camera Management API

- `GET /api/cameras` - List available cameras (USB and network)
- `GET /api/cameras/detect` - Detect available cameras including ESPHome devices
- `POST /api/cameras/capture` - Capture images from selected cameras with region
  metadata
- `GET /api/cameras/{index}/stream` - Live camera feed (USB and network cameras)

### Data Management API

- `GET /tagging/{session_id}` - Shell tagging interface
- `POST /api/shells/save` - Save tagged shell data

## Development

### Code Quality

All code must pass linting and type checking:

```bash
# Run all checks
just check

# Format code
just fmt
```

### Contributing

1. Ensure all tests pass: `just check`
2. Update documentation as needed
3. Commit changes with descriptive messages
4. Follow existing code patterns and conventions

## Troubleshooting

### Common Issues

1. **ESP32 Not Found**: Check network connectivity and device hostname
2. **Camera Issues**: Verify USB connections and permissions
3. **Servo Not Moving**: Check power supply and signal connections
4. **Sensor Not Triggering**: Verify wiring and pullup configuration

### Debug Tools

- **ESPHome Logs**: Real-time device logging via dashboard
- **API Testing**: Use browser dev tools or curl for API debugging
- **Hardware Testing**: Manual control via ESPHome dashboard

## Hardware Sequence

### Automated Case Processing

1. Case loaded into feeder system (sensor: Case Ready)
2. Vibration motor advances case (1.5 seconds)
3. Feeder servo moves case to feed position
4. Positioning servo moves case to camera view
5. System detects case in position (sensor: Camera View)
6. User captures images via web interface
7. User tags images with shell metadata
8. Data saved with image references

This sequence can be triggered via web interface or physical button for flexible
operation modes.

## TODO

- [ ] The composite image doesn't match the capture region
- [ ] load the default set of shell data when the training UI loads. it's Ok,
      because it's all over the LAN.
