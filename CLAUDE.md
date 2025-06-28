## Project Overview

This application controls an ammunition shell case sorting machine that uses computer vision and machine learning to automatically identify and sort different types of shell cases.

## Development Guidelines

- It is mandatory that 'just check' finishes without warnings or errors before considering a task complete
- It is mandatory that the final step in completing a task is that all changes are commited to git
- Any time the design or implementation changes, CLAUDE.md must be updated
- Never allow a bare "type: ignore" comment
- Never use global variables

## Architecture

### Core Components

1. **FastAPI Web Application** (`shell_sorter/app.py`)
   - REST API for machine control and monitoring
   - Web dashboard for user interaction
   - Image upload and case type management endpoints

2. **Configuration Management** (`shell_sorter/config.py`)
   - Pydantic Settings for environment-based configuration
   - Automatic directory creation for data storage
   - Support for environment variables with `SHELL_SORTER_` prefix

3. **Machine Learning Module** (`shell_sorter/ml_trainer.py`)
   - Case type registration and management
   - Training image organization
   - Model training coordination
   - Reference image management

### Directory Structure

```
data/
├── models/          # Trained ML models
├── images/          # Training images organized by case type
├── references/      # Reference images for case types
└── temp/           # Temporary uploads
```

## System Capabilities

### Machine Control
- Start/stop sorting operations
- Monitor machine status and job progress
- Real-time status updates via web interface

### Machine Learning
- Multiple camera setup for shell case imaging
- Case type identification through computer vision
- Support for training custom models with annotated images
- Case types can be identified by:
  - Designation only (e.g., 9mm Parabellum, 38 Special)
  - Designation and brand combination

### Data Management
- Upload and organize reference images
- Manage training datasets per case type
- Automatic model versioning
- Training progress tracking

## API Endpoints

### Machine Control
- `GET /` - Web dashboard
- `GET /api/status` - Machine status
- `POST /api/start-sorting` - Start sorting job
- `POST /api/stop-sorting` - Stop current job
- `GET /api/jobs` - List recent jobs

### ML Management
- `GET /api/case-types` - List case types and training status
- `POST /api/case-types` - Create new case type
- `POST /api/case-types/{name}/reference-image` - Upload reference image
- `POST /api/case-types/{name}/training-image` - Upload training image
- `POST /api/train-model` - Train ML model

## Configuration

Settings can be configured via environment variables or `.env` file:

```bash
SHELL_SORTER_HOST=0.0.0.0
SHELL_SORTER_PORT=8000
SHELL_SORTER_DEBUG=false
SHELL_SORTER_ML_ENABLED=true
SHELL_SORTER_CONFIDENCE_THRESHOLD=0.8
SHELL_SORTER_CAMERA_COUNT=4
```

## Running the Application

```bash
# Install dependencies
uv sync

# Run linting and type checking
just check

# Start the application
python main.py
# or
uv run python main.py
```