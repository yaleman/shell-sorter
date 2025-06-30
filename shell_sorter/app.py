"""Main application module for the Shell Sorter Control Panel."""

from dataclasses import dataclass
from pathlib import Path
from typing import (
    List,
    Dict,
    Any,
    Optional,
    Generator,
    Literal,
    AsyncGenerator,
)
from datetime import datetime
import json
import logging
import signal
import sys
import uuid
from contextlib import asynccontextmanager

import cv2  # type: ignore[import-not-found]
import numpy as np  # type: ignore[import-not-found]

from fastapi import (
    FastAPI,
    Request,
    Form,
    File,
    UploadFile,
    HTTPException,
    Depends,
    BackgroundTasks,
    WebSocket,
    WebSocketDisconnect,
)
from pydantic import BaseModel
from fastapi.responses import HTMLResponse, StreamingResponse
from fastapi.staticfiles import StaticFiles
from fastapi.templating import Jinja2Templates
from fastapi.middleware.cors import CORSMiddleware
import uvicorn

from .config import Settings
from .ml_trainer import MLTrainer
from .camera_manager import CameraManager
from .shell import Shell
from .hardware_controller import HardwareController
from .esphome_monitor import ESPHomeMonitor

from .middleware import NoCacheMiddleware


@dataclass
class Status:
    """Data class to represent the status of the sorting machine."""

    status: str = "idle"
    last_update: str = datetime.now().isoformat()

    def set_running(self) -> bool:
        """Set the status to running and update the last update time.
        Returns:
            bool: True if status was changed, False if already running.
        """
        if self.status == "running":
            return False
        self.status = "running"
        self.last_update = datetime.now().isoformat()
        return True

    def stop_sorting(self) -> None:
        """Stop the sorting process and set status to idle."""
        self.status = "idle"
        self.last_update = datetime.now().isoformat()


class MachineController:
    """Controls the sorting machine state and job management."""

    def __init__(self) -> None:
        self.machine_status: Status = Status()

        self.sorting_jobs: List[Dict[str, Any]] = []

    def get_status(self) -> Dict[str, Any]:
        """Get current machine status."""
        return {
            "status": self.machine_status.status,
            "last_update": self.machine_status.last_update,
        }

    def get_jobs(self) -> List[Dict[str, Any]]:
        """Get all sorting jobs."""
        return self.sorting_jobs

    def get_recent_jobs(self, limit: int = 10) -> List[Dict[str, Any]]:
        """Get recent jobs in reverse chronological order."""
        return self.sorting_jobs[-limit:][::-1]

    def start_sorting(self) -> None:
        """Start a new sorting job."""
        self.machine_status.set_running()

    def stop_sorting(self) -> None:
        """Stop the current sorting job."""
        self.machine_status.stop_sorting()


# Initialize settings on startup
settings = Settings.new()

# Initialize ML trainer
ml_trainer = MLTrainer(settings)

# Initialize machine controller
machine_controller = MachineController()


# Initialize debug WebSocket manager early (needed for hardware controller callback)
class DebugWebSocketManager:
    """Manage WebSocket connections for debug console."""

    def __init__(self) -> None:
        self.active_connections: List[WebSocket] = []

    async def connect(self, websocket: WebSocket) -> None:
        """Accept a new WebSocket connection."""
        await websocket.accept()
        self.active_connections.append(websocket)
        logger.info(
            "Debug WebSocket connected, total connections: %d",
            len(self.active_connections),
        )

    def disconnect(self, websocket: WebSocket) -> None:
        """Remove a WebSocket connection."""
        if websocket in self.active_connections:
            self.active_connections.remove(websocket)
        logger.info(
            "Debug WebSocket disconnected, total connections: %d",
            len(self.active_connections),
        )

    async def broadcast_command(self, command_data: Dict[str, Any]) -> None:
        """Broadcast command data to all connected clients."""
        if not self.active_connections:
            return

        disconnected_connections = []
        for connection in self.active_connections:
            try:
                await connection.send_json(command_data)
            except Exception as e:
                logger.warning("Failed to send to WebSocket connection: %s", e)
                disconnected_connections.append(connection)

        # Remove failed connections
        for connection in disconnected_connections:
            self.disconnect(connection)


# Global WebSocket manager for debug console
debug_ws_manager = DebugWebSocketManager()

# Initialize camera manager
camera_manager = CameraManager(settings)

# Initialize hardware controller
hardware_controller = HardwareController()

# Set up WebSocket broadcasting for debug commands
hardware_controller.set_command_broadcast_callback(debug_ws_manager.broadcast_command)

# Initialize ESPHome monitor
esphome_monitor = ESPHomeMonitor(settings.esphome_hostname)

# Setup logging with timestamps including milliseconds
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s.%(msecs)03d - %(name)s - %(levelname)s - %(message)s",
    datefmt="%Y-%m-%d %H:%M:%S",
)
logger = logging.getLogger(__name__)


@asynccontextmanager
async def lifespan(_app: FastAPI) -> AsyncGenerator[None, None]:
    """Manage application lifespan events."""
    # Startup
    logger.info("Starting ESPHome connectivity monitoring")
    esphome_monitor.start_monitoring()

    yield

    # Shutdown
    logger.info("Stopping ESPHome connectivity monitoring")
    esphome_monitor.stop_monitoring()


app = FastAPI(title="Shell Sorter Control Panel", version="1.0.0", lifespan=lifespan)

# Add cache-busting middleware (add first to ensure it runs on all responses)
app.add_middleware(NoCacheMiddleware)

# Add CORS middleware
app.add_middleware(
    CORSMiddleware,
    allow_origins=[settings.host],  # In production, specify actual origins
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# Setup templates and static files
templates_dir = Path(__file__).parent / "templates"
static_dir = Path(__file__).parent / "static"

templates = Jinja2Templates(directory=str(templates_dir))
app.mount("/static", StaticFiles(directory=str(static_dir)), name="static")
app.mount("/images", StaticFiles(directory="images"), name="images")
app.mount("/data", StaticFiles(directory=str(settings.data_directory)), name="data")


def get_machine_controller() -> MachineController:
    """Dependency to get the machine controller instance."""
    return machine_controller


def get_ml_trainer() -> MLTrainer:
    """Dependency to get the ML trainer instance."""
    return ml_trainer


def get_settings() -> Settings:
    """Dependency to get the settings instance."""
    return settings


def get_hardware_controller() -> HardwareController:
    """Dependency to get the hardware controller instance."""
    return hardware_controller


def get_esphome_monitor() -> ESPHomeMonitor:
    """Dependency to get the ESPHome monitor instance."""
    return esphome_monitor


@app.get("/", response_class=HTMLResponse)
async def dashboard(
    request: Request,
    controller: MachineController = Depends(get_machine_controller),
    app_settings: Settings = Depends(get_settings),
) -> HTMLResponse:
    """Render the main dashboard page with machine status and camera information."""
    return templates.TemplateResponse(
        "dashboard.html",
        {
            "request": request,
            "machine_status": controller.get_status(),
            "supported_case_types": app_settings.supported_case_types,
            "cameras": camera_manager.get_cameras(),
        },
    )


@app.get("/config", response_class=HTMLResponse)
async def config_page(
    request: Request,
    app_settings: Settings = Depends(get_settings),
) -> HTMLResponse:
    """Render the configuration page."""
    # Get network camera hostnames from user config
    try:
        user_config_data = app_settings.load_user_config()
        from .config import UserConfig

        user_config = UserConfig(**user_config_data)
        network_camera_hostnames = user_config.network_camera_hostnames
    except Exception:
        # Fall back to application settings
        network_camera_hostnames = app_settings.network_camera_hostnames

    return templates.TemplateResponse(
        "config.html",
        {
            "request": request,
            "cameras": camera_manager.get_cameras(),
            "esphome_hostname": app_settings.esphome_hostname,
            "network_camera_hostnames": network_camera_hostnames,
        },
    )


@app.get("/api/status")
async def get_status(
    controller: MachineController = Depends(get_machine_controller),
) -> Dict[str, Any]:
    """Get the current machine status."""
    return controller.get_status()


@app.post("/api/start-sorting")
async def start_sorting(
    case_type: str = Form(...),
    quantity: int = Form(...),
    controller: MachineController = Depends(get_machine_controller),
) -> Dict[str, str]:
    """Start a sorting job for the specified case type and quantity."""
    controller.start_sorting()
    return {"message": f"Started sorting {quantity} {case_type} cases"}


@app.post("/api/stop-sorting")
async def stop_sorting(
    controller: MachineController = Depends(get_machine_controller),
) -> Dict[str, str]:
    """Stop the current sorting job."""
    controller.stop_sorting()
    return {"message": "Sorting stopped"}


@app.get("/api/jobs")
async def get_jobs(
    controller: MachineController = Depends(get_machine_controller),
) -> List[Dict[str, Any]]:
    """Get a list of all sorting jobs."""
    return controller.get_jobs()


# ML Training and Case Type Management Endpoints


@app.get("/api/case-types")
async def get_case_types(
    trainer: MLTrainer = Depends(get_ml_trainer),
) -> Dict[str, Dict[str, Any]]:
    """Get all registered case types with training status."""
    return trainer.get_training_summary()


@app.post("/api/case-types")
async def create_case_type(
    name: str = Form(...),
    designation: str = Form(...),
    brand: Optional[str] = Form(None),
    trainer: MLTrainer = Depends(get_ml_trainer),
) -> Dict[str, Any]:
    """Create a new case type."""
    try:
        case_type = trainer.add_case_type(name, designation, brand)
        return {
            "message": f"Case type '{name}' created successfully",
            "case_type": case_type.to_dict(),
        }
    except Exception as e:
        raise HTTPException(status_code=400, detail=str(e)) from e


@app.post("/api/case-types/{case_type_name}/reference-image")
async def upload_reference_image(
    case_type_name: str,
    file: UploadFile = File(...),
    trainer: MLTrainer = Depends(get_ml_trainer),
    app_settings: Settings = Depends(get_settings),
) -> Dict[str, str]:
    """Upload a reference image for a case type."""
    if not file.content_type or not file.content_type.startswith("image/"):
        raise HTTPException(status_code=400, detail="File must be an image")

    try:
        # Save uploaded file
        filename = file.filename or f"upload_{datetime.now().timestamp()}"
        temp_path = app_settings.data_directory / "temp" / filename
        temp_path.parent.mkdir(exist_ok=True)

        with open(temp_path, "wb") as buffer:
            content = await file.read()
            buffer.write(content)

        # Add to ML trainer
        success = trainer.add_reference_image(case_type_name, temp_path)
        if success:
            return {"message": f"Reference image uploaded for {case_type_name}"}
        raise HTTPException(status_code=400, detail="Failed to add reference image")

    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e)) from e


@app.post("/api/case-types/{case_type_name}/training-image")
async def upload_training_image(
    case_type_name: str,
    file: UploadFile = File(...),
    trainer: MLTrainer = Depends(get_ml_trainer),
    app_settings: Settings = Depends(get_settings),
) -> Dict[str, str]:
    """Upload a training image for a case type."""
    if not file.content_type or not file.content_type.startswith("image/"):
        raise HTTPException(status_code=400, detail="File must be an image")

    try:
        # Save uploaded file
        filename = file.filename or f"upload_{datetime.now().timestamp()}"
        temp_path = app_settings.data_directory / "temp" / filename
        temp_path.parent.mkdir(exist_ok=True)

        with open(temp_path, "wb") as buffer:
            content = await file.read()
            buffer.write(content)

        # Add to ML trainer
        success = trainer.add_training_image(case_type_name, temp_path)
        if success:
            return {"message": f"Training image uploaded for {case_type_name}"}
        raise HTTPException(status_code=400, detail="Failed to add training image")

    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e)) from e


class TrainModelRequest(BaseModel):
    """Request model for training ML model."""

    case_types: Optional[List[str]] = None


@app.post("/api/train-model")
async def train_model(
    request: TrainModelRequest,
    trainer: MLTrainer = Depends(get_ml_trainer),
) -> Dict[str, Any]:
    """Train the ML model with available data."""
    try:
        success, message = trainer.train_model(request.case_types)
        if success:
            return {"message": message, "success": True}
        raise HTTPException(status_code=400, detail=message)
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e)) from e


# Camera Management Endpoints


@app.get("/api/cameras/detect")
async def detect_cameras() -> List[Dict[str, Any]]:
    """Detect all available cameras."""
    cameras = camera_manager.detect_cameras()
    return [
        {
            "index": cam.index,
            "name": cam.name,
            "resolution": cam.resolution,
            "is_active": cam.is_active,
            "is_selected": cam.is_selected,
            "view_type": cam.view_type,
            "region_x": cam.region_x,
            "region_y": cam.region_y,
            "region_width": cam.region_width,
            "region_height": cam.region_height,
        }
        for cam in cameras
    ]


@app.get("/api/cameras")
async def get_cameras() -> List[Dict[str, Any]]:
    """Get list of detected cameras."""
    cameras = camera_manager.get_cameras()
    return [
        {
            "index": cam.index,
            "name": cam.name,
            "resolution": cam.resolution,
            "is_active": cam.is_active,
            "is_selected": cam.is_selected,
            "view_type": cam.view_type,
            "region_x": cam.region_x,
            "region_y": cam.region_y,
            "region_width": cam.region_width,
            "region_height": cam.region_height,
        }
        for cam in cameras
    ]


@app.post("/api/cameras/select")
async def select_cameras(
    request: Request,
) -> Dict[str, Any]:
    """Select which cameras to use for sorting."""
    try:
        body = await request.json()
        camera_indices = body if isinstance(body, list) else []
        success = camera_manager.select_cameras(camera_indices)
        if success:
            return {
                "message": f"Selected cameras: {camera_indices}",
                "selected_cameras": camera_indices,
            }
        raise HTTPException(status_code=400, detail="Failed to select cameras")
    except Exception as e:
        raise HTTPException(
            status_code=400, detail=f"Invalid request body: {str(e)}"
        ) from e


def start_camera_background(camera_index: int) -> None:
    """Background task to start a camera."""
    logger.info("Starting camera %d in background", camera_index)
    success = camera_manager.start_camera_stream(camera_index)
    if success:
        logger.info("Successfully started camera %d", camera_index)
    else:
        logger.error("Failed to start camera %d", camera_index)


@app.post("/api/cameras/{camera_index}/start")
async def start_camera(
    camera_index: int,
    background_tasks: BackgroundTasks,
) -> Dict[str, str]:
    """Start streaming from a specific camera."""
    if camera_index not in camera_manager.cameras:
        raise HTTPException(status_code=400, detail=f"Camera {camera_index} not found")

    background_tasks.add_task(start_camera_background, camera_index)
    return {"message": f"Starting camera {camera_index} in background"}


@app.post("/api/cameras/{camera_index}/stop")
async def stop_camera(
    camera_index: int,
) -> Dict[str, str]:
    """Stop streaming from a specific camera."""
    camera_manager.stop_camera_stream(camera_index)
    return {"message": f"Stopped camera {camera_index}"}


def start_selected_cameras_background() -> None:
    """Background task to start all selected cameras."""
    logger.info("Starting selected cameras in background")
    started = camera_manager.start_selected_cameras()
    logger.info("Background task completed - started cameras: %s", started)


@app.post("/api/cameras/start-selected")
async def start_selected_cameras(background_tasks: BackgroundTasks) -> Dict[str, Any]:
    """Start streaming from all selected cameras."""
    selected_cameras = camera_manager.get_selected_cameras()
    if not selected_cameras:
        raise HTTPException(status_code=400, detail="No cameras selected")

    camera_indices = [cam.index for cam in selected_cameras]
    background_tasks.add_task(start_selected_cameras_background)

    return {
        "message": f"Starting {len(selected_cameras)} selected cameras in background",
        "selected_cameras": camera_indices,
    }


@app.post("/api/cameras/stop-all")
async def stop_all_cameras() -> Dict[str, str]:
    """Stop all camera streams."""
    camera_manager.stop_all_cameras()
    return {"message": "Stopped all cameras"}


@app.post("/api/cameras/{camera_index}/view-type")
async def set_camera_view_type(
    camera_index: int,
    view_type: Optional[str] = Form(None),
) -> Dict[str, Any]:
    """Set the view type for a camera."""
    # Validate and cast view_type
    if view_type == "side":
        validated_view_type: Optional[Literal["side", "tail"]] = "side"
    elif view_type == "tail":
        validated_view_type = "tail"
    elif view_type is None or view_type == "":
        validated_view_type = None
    else:
        raise HTTPException(
            status_code=400,
            detail="Invalid view type. Must be 'side', 'tail', or empty",
        )

    success = camera_manager.set_camera_view_type(camera_index, validated_view_type)
    if success:
        return {
            "message": f"Set camera {camera_index} view type to {validated_view_type}",
            "camera_index": camera_index,
            "view_type": validated_view_type,
        }
    raise HTTPException(status_code=404, detail=f"Camera {camera_index} not found")


@app.post("/api/cameras/{camera_index}/region")
async def set_camera_region(
    camera_index: int,
    x: int = Form(...),
    y: int = Form(...),
    width: int = Form(...),
    height: int = Form(...),
) -> Dict[str, Any]:
    """Set the region of interest for a camera."""
    # Validate region parameters
    if width <= 0 or height <= 0:
        raise HTTPException(
            status_code=400, detail="Width and height must be positive values"
        )
    if x < 0 or y < 0:
        raise HTTPException(
            status_code=400, detail="X and Y coordinates must be non-negative"
        )

    success = camera_manager.set_camera_region(camera_index, x, y, width, height)
    if success:
        return {
            "message": f"Set camera {camera_index} region to ({x},{y}) {width}x{height}",
            "camera_index": camera_index,
            "region": {"x": x, "y": y, "width": width, "height": height},
        }
    raise HTTPException(status_code=404, detail=f"Camera {camera_index} not found")


@app.delete("/api/cameras/{camera_index}/region")
async def clear_camera_region(
    camera_index: int,
) -> Dict[str, Any]:
    """Clear the region of interest for a camera."""
    success = camera_manager.clear_camera_region(camera_index)
    if success:
        return {
            "message": f"Cleared camera {camera_index} region",
            "camera_index": camera_index,
        }
    raise HTTPException(status_code=404, detail=f"Camera {camera_index} not found")


@app.post("/api/cameras/{camera_index}/autofocus")
async def trigger_camera_autofocus(camera_index: int) -> Dict[str, Any]:
    """Trigger autofocus for a camera, focusing on region center if set."""
    success = camera_manager.trigger_autofocus(camera_index)
    if success:
        camera_info = camera_manager.cameras.get(camera_index)
        if camera_info and (
            camera_info.region_x is not None
            and camera_info.region_y is not None
            and camera_info.region_width is not None
            and camera_info.region_height is not None
        ):
            center_x = camera_info.region_x + camera_info.region_width // 2
            center_y = camera_info.region_y + camera_info.region_height // 2
            return {
                "message": f"Triggered autofocus for camera {camera_index} at region center",
                "camera_index": camera_index,
                "focus_point": {"x": center_x, "y": center_y},
            }
        else:
            return {
                "message": f"Triggered autofocus for camera {camera_index}",
                "camera_index": camera_index,
                "focus_point": None,
            }
    raise HTTPException(
        status_code=404, detail=f"Camera {camera_index} not found or autofocus failed"
    )


# Machine Control Endpoints


@app.post("/api/machine/next-case")
async def trigger_next_case() -> Dict[str, str]:
    """Trigger next case sequence via ESPHome hardware controller."""
    try:
        # Run the complete next case sequence
        success = await hardware_controller.run_next_case_sequence()

        if success:
            logger.info("Next case sequence completed successfully")
            return {
                "message": "Next case sequence completed - case advanced to camera position"
            }
        logger.warning("Next case sequence failed")
        raise HTTPException(
            status_code=500, detail="Failed to complete next case sequence"
        )

    except Exception as e:
        logger.error("Error triggering next case: %s", e)
        raise HTTPException(status_code=500, detail=str(e)) from e


@app.get("/api/machine/sensors")
async def get_sensor_status() -> Dict[str, Any]:
    """Get current status of hardware sensors."""
    try:
        sensors = await hardware_controller.get_sensor_states()
        return {
            "case_ready_to_feed": sensors.get("case_ready", False),
            "case_in_camera_view": sensors.get("case_in_camera", False),
            "timestamp": datetime.now().isoformat(),
        }
    except Exception as e:
        logger.error("Error getting sensor status: %s", e)
        raise HTTPException(status_code=500, detail=str(e)) from e


@app.get("/api/machine/hardware-status")
async def get_hardware_status() -> Dict[str, Any]:
    """Get ESPHome device status and connection info."""
    try:
        connection_ok = await hardware_controller.test_connection()
        device_info = await hardware_controller.get_device_info()

        return {
            "connected": connection_ok,
            "device_info": device_info,
            "timestamp": datetime.now().isoformat(),
        }
    except Exception as e:
        logger.error("Error getting hardware status: %s", e)
        raise HTTPException(status_code=500, detail=str(e)) from e


@app.get("/api/machine/esphome-status")
async def get_esphome_status(
    monitor: ESPHomeMonitor = Depends(get_esphome_monitor),
) -> Dict[str, Any]:
    """Get ESPHome device connectivity status."""
    return monitor.get_status()


@app.get("/api/cameras/{camera_index}/stream")
async def camera_stream(
    camera_index: int,
) -> StreamingResponse:
    """Stream live video from a camera."""

    def generate_frames() -> Generator[bytes, None, None]:
        while True:
            frame_data = camera_manager.get_latest_frame(camera_index)
            if frame_data:
                yield (
                    b"--frame\r\n"
                    b"Content-Type: image/jpeg\r\n\r\n" + frame_data + b"\r\n"
                )
            else:
                # Return a placeholder if no frame available
                yield (
                    b"--frame\r\nContent-Type: text/plain\r\n\r\nNo frame available\r\n"
                )

    return StreamingResponse(
        generate_frames(), media_type="multipart/x-mixed-replace; boundary=frame"
    )


@app.post("/api/cameras/capture")
async def capture_images() -> Dict[str, Any]:
    """Capture images from all selected cameras and return capture session ID."""
    selected_cameras = camera_manager.get_selected_cameras()
    if not selected_cameras:
        raise HTTPException(status_code=400, detail="No cameras selected")

    # Generate unique session ID
    session_id = str(uuid.uuid4())
    captured_images = []

    # Create images directory if it doesn't exist
    images_dir = Path("images")
    images_dir.mkdir(exist_ok=True)

    for camera in selected_cameras:
        if camera.is_active:
            # Capture high-resolution frame with EXIF metadata
            frame_data = camera_manager.capture_high_resolution_image(camera.index)
            if frame_data:
                # Save image with camera index in filename
                filename = f"{session_id}_camera_{camera.index}.jpg"
                image_path = images_dir / filename

                with open(image_path, "wb") as f:
                    f.write(frame_data)

                captured_images.append(
                    {
                        "camera_index": camera.index,
                        "filename": filename,
                        "camera_name": camera.name,
                        "view_type": camera.view_type,
                        "region_x": camera.region_x,
                        "region_y": camera.region_y,
                        "region_width": camera.region_width,
                        "region_height": camera.region_height,
                    }
                )

                logger.info("Captured image from camera %d: %s", camera.index, filename)
            else:
                logger.warning("No frame available from camera %d", camera.index)

    if not captured_images:
        raise HTTPException(
            status_code=400, detail="No images could be captured from selected cameras"
        )

    # Save capture metadata for later use during shell data saving
    metadata_path = images_dir / f"{session_id}_metadata.json"
    with open(metadata_path, "w", encoding="utf-8") as f:
        json.dump(captured_images, f, indent=2)

    logger.info("Captured %d images for session %s", len(captured_images), session_id)

    return {
        "session_id": session_id,
        "captured_images": captured_images,
        "message": f"Captured {len(captured_images)} images from selected cameras",
    }


@app.get("/ml-training", response_class=HTMLResponse)
async def ml_training_page(
    request: Request,
) -> HTMLResponse:
    """Display the ML training interface."""
    return templates.TemplateResponse(
        "ml_training.html",
        {
            "request": request,
        },
    )


@app.get("/shell-edit/{session_id}", response_class=HTMLResponse)
async def shell_edit_page(
    request: Request,
    session_id: str,
    app_settings: Settings = Depends(get_settings),
) -> HTMLResponse:
    """Display the shell editing interface for a specific shell."""
    try:
        data_dir = app_settings.data_directory
        shell_file = data_dir / f"{session_id}.json"

        if not shell_file.exists():
            raise HTTPException(status_code=404, detail="Shell data not found")

        # Load shell data
        with open(shell_file, "r", encoding="utf-8") as f:
            shell_data = json.load(f)

        return templates.TemplateResponse(
            "shell_edit.html",
            {
                "request": request,
                "shell": shell_data,
                "session_id": session_id,
            },
        )

    except HTTPException:
        raise
    except Exception as e:
        logger.error("Error loading shell edit page for %s: %s", session_id, e)
        raise HTTPException(status_code=500, detail=str(e)) from e


@app.get("/region-selection/{camera_index}", response_class=HTMLResponse)
async def region_selection_page(
    request: Request,
    camera_index: int,
) -> HTMLResponse:
    """Display the region selection interface for a camera."""
    if camera_index not in camera_manager.cameras:
        raise HTTPException(status_code=404, detail=f"Camera {camera_index} not found")

    camera = camera_manager.cameras[camera_index]

    return templates.TemplateResponse(
        "region_selection.html",
        {
            "request": request,
            "camera": {
                "index": camera.index,
                "name": camera.name,
                "resolution": camera.resolution,
                "is_active": camera.is_active,
                "view_type": camera.view_type,
                "region_x": camera.region_x,
                "region_y": camera.region_y,
                "region_width": camera.region_width,
                "region_height": camera.region_height,
            },
        },
    )


@app.get("/tagging/{session_id}", response_class=HTMLResponse)
async def tagging_page(
    request: Request,
    session_id: str,
    app_settings: Settings = Depends(get_settings),
) -> HTMLResponse:
    """Display the tagging interface for captured images."""
    # Find captured images for this session
    images_dir = Path("images")
    captured_images = []

    if images_dir.exists():
        for image_file in images_dir.glob(f"{session_id}_camera_*.jpg"):
            # Extract camera index from filename
            filename_parts = image_file.stem.split("_")
            if len(filename_parts) >= 3:
                camera_index = int(filename_parts[2])

                # Try to get the camera name from the camera manager
                camera_name = f"Camera {camera_index}"
                if camera_index in camera_manager.cameras:
                    camera_name = camera_manager.cameras[camera_index].name

                captured_images.append(
                    {
                        "filename": image_file.name,
                        "camera_index": str(camera_index),
                        "camera_name": camera_name,
                    }
                )

    if not captured_images:
        raise HTTPException(
            status_code=404, detail="No captured images found for this session"
        )

    return templates.TemplateResponse(
        "tagging.html",
        {
            "request": request,
            "session_id": session_id,
            "captured_images": captured_images,
            "supported_case_types": app_settings.supported_case_types,
        },
    )


@app.post("/api/shells/save")
async def save_shell_data(
    request: Request,
    app_settings: Settings = Depends(get_settings),
) -> Dict[str, Any]:
    """Save tagged shell data to JSON file."""
    try:
        # Parse JSON body
        body = await request.json()
        session_id = body.get("session_id")
        brand = body.get("brand")
        shell_type = body.get("shell_type")
        filenames_list = body.get("image_filenames")

        # Validate required fields
        if not session_id:
            raise HTTPException(status_code=400, detail="session_id is required")
        if not brand:
            raise HTTPException(status_code=400, detail="brand is required")
        if not shell_type:
            raise HTTPException(status_code=400, detail="shell_type is required")
        if not filenames_list or not isinstance(filenames_list, list):
            raise HTTPException(
                status_code=400, detail="image_filenames must be a non-empty list"
            )

        # Load capture metadata if available
        captured_images_data = None
        try:
            metadata_path = Path("images") / f"{session_id}_metadata.json"
            if metadata_path.exists():
                with open(metadata_path, "r", encoding="utf-8") as f:
                    metadata = json.load(f)
                    # Convert to CapturedImage objects
                    from .shell import CapturedImage

                    captured_images_data = [
                        CapturedImage(**img_data) for img_data in metadata
                    ]
        except Exception as e:
            logger.warning(
                "Could not load capture metadata for session %s: %s", session_id, e
            )

        # Create Shell object
        shell = Shell(
            date_captured=datetime.now(),
            brand=brand,
            shell_type=shell_type,
            image_filenames=filenames_list,
            captured_images=captured_images_data,
        )

        # Create data directory if it doesn't exist
        data_dir = app_settings.data_directory
        data_dir.mkdir(exist_ok=True)

        # Save shell data as JSON
        json_filename = f"{session_id}.json"
        json_path = data_dir / json_filename

        with open(json_path, "w", encoding="utf-8") as f:
            f.write(shell.model_dump_json(indent=2))

        logger.info(
            "Saved shell data for session %s: brand=%s, type=%s, images=%d",
            session_id,
            brand,
            shell_type,
            len(filenames_list),
        )

        return {
            "message": "Shell data saved successfully",
            "session_id": session_id,
            "filename": json_filename,
            "shell_data": shell.model_dump(),
        }

    except Exception as e:
        logger.error("Error saving shell data: %s", e)
        raise HTTPException(status_code=500, detail=str(e)) from e


# Configuration Management Endpoints


@app.get("/api/config")
async def get_configuration(
    app_settings: Settings = Depends(get_settings),
) -> Dict[str, Any]:
    """Get current system configuration."""
    try:
        # Get configuration from user config
        try:
            user_config_data = app_settings.load_user_config()
            from .config import UserConfig

            user_config = UserConfig(**user_config_data)
            network_camera_hostnames = user_config.network_camera_hostnames
            auto_detect_cameras = user_config.auto_detect_cameras
        except Exception:
            # Fall back to application settings
            network_camera_hostnames = app_settings.network_camera_hostnames
            auto_detect_cameras = app_settings.auto_detect_cameras

        config_data = {
            "auto_start_cameras": getattr(camera_manager, "auto_start_cameras", False),
            "auto_detect_cameras": auto_detect_cameras,
            "esphome_hostname": app_settings.esphome_hostname,
            "network_camera_hostnames": network_camera_hostnames,
            "cameras": [
                {
                    "index": cam.index,
                    "name": cam.name,
                    "resolution": cam.resolution,
                    "is_active": cam.is_active,
                    "is_selected": cam.is_selected,
                    "view_type": cam.view_type,
                    "region_x": cam.region_x,
                    "region_y": cam.region_y,
                    "region_width": cam.region_width,
                    "region_height": cam.region_height,
                }
                for cam in camera_manager.get_cameras()
            ],
        }
        return config_data
    except Exception as e:
        logger.error("Error getting configuration: %s", e)
        raise HTTPException(status_code=500, detail=str(e)) from e


@app.post("/api/config")
async def save_configuration(
    config: Dict[str, Any],
    app_settings: Settings = Depends(get_settings),
    monitor: ESPHomeMonitor = Depends(get_esphome_monitor),
) -> Dict[str, str]:
    """Save system configuration."""
    try:
        # Save auto-start setting
        if "auto_start_cameras" in config:
            camera_manager.auto_start_cameras = config["auto_start_cameras"]

        # Save ESPHome hostname setting
        if "esphome_hostname" in config:
            new_hostname = config["esphome_hostname"].strip()
            if new_hostname and new_hostname != app_settings.esphome_hostname:
                # Update settings
                app_settings.esphome_hostname = new_hostname
                # Update monitor hostname
                monitor.update_hostname(new_hostname)
                logger.info("Updated ESPHome hostname to: %s", new_hostname)

        # Save network camera hostnames and auto-detect settings
        if "network_camera_hostnames" in config or "auto_detect_cameras" in config:
            # Load current user config
            try:
                user_config_data = app_settings.load_user_config()
                from .config import UserConfig

                user_config = UserConfig(**user_config_data)
            except Exception:
                user_config = UserConfig()

            # Update network camera hostnames
            if "network_camera_hostnames" in config:
                hostnames = config["network_camera_hostnames"]
                if isinstance(hostnames, list):
                    user_config.network_camera_hostnames = [
                        hostname.strip() for hostname in hostnames if hostname.strip()
                    ]
                    logger.info(
                        "Updated network camera hostnames: %s",
                        user_config.network_camera_hostnames,
                    )

            # Update auto-detect cameras setting
            if "auto_detect_cameras" in config:
                user_config.auto_detect_cameras = bool(config["auto_detect_cameras"])
                logger.info(
                    "Updated auto-detect cameras: %s", user_config.auto_detect_cameras
                )

            # Save to user config file
            app_settings.save_user_config(user_config.model_dump())

        # Save configuration to file
        camera_manager.save_config()

        logger.info("Configuration saved successfully")
        return {"message": "Configuration saved successfully"}
    except Exception as e:
        logger.error("Error saving configuration: %s", e)
        raise HTTPException(status_code=500, detail=str(e)) from e


@app.delete("/api/config/cameras/{camera_index}")
async def delete_camera_config(camera_index: int) -> Dict[str, str]:
    """Delete a specific camera from configuration."""
    try:
        # Remove camera from manager
        removed = camera_manager.remove_camera(camera_index)

        if removed:
            # Save updated configuration
            camera_manager.save_config()
            logger.info("Deleted camera %d from configuration", camera_index)
            return {"message": f"Camera {camera_index} deleted successfully"}
        else:
            raise HTTPException(
                status_code=404, detail=f"Camera {camera_index} not found"
            )
    except HTTPException:
        raise
    except Exception as e:
        logger.error("Error deleting camera %d: %s", camera_index, e)
        raise HTTPException(status_code=500, detail=str(e)) from e


@app.delete("/api/config/cameras")
async def clear_all_cameras() -> Dict[str, str]:
    """Clear all cameras from configuration."""
    try:
        # Clear all cameras
        camera_manager.clear_cameras()

        # Save configuration
        camera_manager.save_config()

        logger.info("Cleared all cameras from configuration")
        return {"message": "All cameras cleared successfully"}
    except Exception as e:
        logger.error("Error clearing cameras: %s", e)
        raise HTTPException(status_code=500, detail=str(e)) from e


@app.post("/api/config/reset")
async def reset_configuration() -> Dict[str, str]:
    """Reset configuration to defaults."""
    try:
        # Reset camera manager to defaults
        camera_manager.reset_to_defaults()

        # Save default configuration
        camera_manager.save_config()

        logger.info("Configuration reset to defaults")
        return {"message": "Configuration reset to defaults"}
    except Exception as e:
        logger.error("Error resetting configuration: %s", e)
        raise HTTPException(status_code=500, detail=str(e)) from e


# ML Training Endpoints


@app.get("/api/ml/shells")
async def get_training_shells(
    app_settings: Settings = Depends(get_settings),
) -> Dict[str, Any]:
    """Load all Shell objects from the data directory for training."""
    try:
        shells = []
        data_dir = app_settings.data_directory

        if not data_dir.exists():
            return {
                "shells": [],
                "summary": {"total": 0, "included": 0, "unique_types": 0},
            }

        # Load all JSON files in data directory
        for json_file in data_dir.glob("*.json"):
            # Skip the case_types.json file
            if json_file.name == "case_types.json":
                continue

            try:
                with open(json_file, "r", encoding="utf-8") as f:
                    shell_data = json.load(f)
                    shell = Shell(**shell_data)

                    # Add session_id from filename
                    shell_dict = shell.model_dump()
                    shell_dict["session_id"] = json_file.stem
                    shells.append(shell_dict)

            except Exception as e:
                logger.warning("Error loading shell data from %s: %s", json_file, e)
                continue

        # Calculate summary statistics
        included_shells = [s for s in shells if s.get("include", True)]
        unique_types = len(set(f"{s['brand']}_{s['shell_type']}" for s in shells))

        summary = {
            "total": len(shells),
            "included": len(included_shells),
            "unique_types": unique_types,
        }

        return {"shells": shells, "summary": summary}

    except Exception as e:
        logger.error("Error loading training shells: %s", e)
        raise HTTPException(status_code=500, detail=str(e)) from e


@app.post("/api/ml/shells/{session_id}/toggle")
async def toggle_shell_include(
    session_id: str,
    app_settings: Settings = Depends(get_settings),
) -> Dict[str, Any]:
    """Toggle the include flag for a shell in training."""
    try:
        data_dir = app_settings.data_directory
        json_file = data_dir / f"{session_id}.json"

        if not json_file.exists():
            raise HTTPException(status_code=404, detail="Shell data not found")

        # Load shell data
        with open(json_file, "r", encoding="utf-8") as f:
            shell_data = json.load(f)

        # Toggle include flag
        shell_data["include"] = not shell_data.get("include", True)

        # Save updated data
        with open(json_file, "w", encoding="utf-8") as f:
            json.dump(shell_data, f, indent=2)

        return {
            "session_id": session_id,
            "include": shell_data["include"],
            "message": f"Shell {'included' if shell_data['include'] else 'excluded'} from training",
        }

    except Exception as e:
        logger.error("Error toggling shell include: %s", e)
        raise HTTPException(status_code=500, detail=str(e)) from e


@app.post("/api/ml/generate-composites")
async def generate_composite_images(
    app_settings: Settings = Depends(get_settings),
) -> Dict[str, Any]:
    """Generate composite images for all included shells."""
    try:
        data_dir = app_settings.data_directory
        composites_dir = data_dir / "composites"
        composites_dir.mkdir(exist_ok=True)

        generated_count = 0
        error_count = 0

        # Process all shell files
        for json_file in data_dir.glob("*.json"):
            if json_file.name == "case_types.json":
                continue

            try:
                with open(json_file, "r", encoding="utf-8") as f:
                    shell_data = json.load(f)

                # Skip if not included in training
                if not shell_data.get("include", True):
                    continue

                # Generate composite image
                session_id = json_file.stem
                composite_path = await _generate_composite_image(
                    session_id, shell_data, composites_dir
                )

                if composite_path:
                    generated_count += 1
                else:
                    error_count += 1

            except Exception as e:
                logger.warning("Error processing shell %s: %s", json_file, e)
                error_count += 1
                continue

        return {
            "generated": generated_count,
            "errors": error_count,
            "message": f"Generated {generated_count} composite images ({error_count} errors)",
        }

    except Exception as e:
        logger.error("Error generating composite images: %s", e)
        raise HTTPException(status_code=500, detail=str(e)) from e


@app.get("/api/composites/{session_id}")
async def get_composite_image(
    session_id: str,
    app_settings: Settings = Depends(get_settings),
) -> StreamingResponse:
    """Get composite image for a session, creating it if it doesn't exist."""
    try:
        data_dir = app_settings.data_directory
        composites_dir = data_dir / "composites"
        composites_dir.mkdir(exist_ok=True)

        composite_path = composites_dir / f"{session_id}_composite.jpg"

        # If composite doesn't exist, try to create it
        if not composite_path.exists():
            shell_file = data_dir / f"{session_id}.json"
            if shell_file.exists():
                try:
                    with open(shell_file, "r", encoding="utf-8") as f:
                        shell_data = json.load(f)

                    # Generate composite image
                    created_path = await _generate_composite_image(
                        session_id, shell_data, composites_dir
                    )

                    if not created_path:
                        raise HTTPException(
                            status_code=404, detail="Could not create composite image"
                        )

                except Exception as e:
                    logger.error(
                        "Error auto-creating composite for %s: %s", session_id, e
                    )
                    raise HTTPException(
                        status_code=500, detail="Failed to create composite image"
                    ) from e
            else:
                raise HTTPException(status_code=404, detail="Shell data not found")

        # Serve the composite image
        if composite_path.exists():

            def iterfile() -> Generator[bytes, None, None]:
                with open(composite_path, "rb") as f:
                    yield from f

            return StreamingResponse(
                iterfile(),
                media_type="image/jpeg",
                headers={"Cache-Control": "no-cache"},
            )
        else:
            raise HTTPException(status_code=404, detail="Composite image not found")

    except HTTPException:
        raise
    except Exception as e:
        logger.error("Error serving composite image for %s: %s", session_id, e)
        raise HTTPException(status_code=500, detail=str(e)) from e


@app.post("/api/ml/shells/{session_id}/view-type")
async def update_shell_image_view_type(
    session_id: str,
    view_type_data: Dict[str, str],
    app_settings: Settings = Depends(get_settings),
) -> Dict[str, Any]:
    """Update the view type for a specific shell image."""
    try:
        from .shell import ViewType

        filename = view_type_data.get("filename")
        new_view_type = view_type_data.get("view_type")

        if not filename:
            raise HTTPException(status_code=400, detail="Filename is required")

        # Validate view type
        valid_view_types = [ViewType.SIDE, ViewType.TAIL, ViewType.UNKNOWN]
        if new_view_type not in valid_view_types:
            raise HTTPException(
                status_code=400,
                detail=f"Invalid view type. Must be one of: {[vt.value for vt in valid_view_types]}",
            )

        # Load shell data from data directory (not image directory)
        shell_file = app_settings.data_directory / f"{session_id}.json"
        if not shell_file.exists():
            raise HTTPException(
                status_code=404, detail=f"Shell data not found for session {session_id}"
            )

        with open(shell_file, "r", encoding="utf-8") as f:
            shell_data = json.load(f)

        # Update the view type for the specific image
        updated = False
        if "captured_images" in shell_data:
            for image in shell_data["captured_images"]:
                if image.get("filename") == filename:
                    image["view_type"] = new_view_type
                    updated = True
                    break

        if not updated:
            raise HTTPException(
                status_code=404,
                detail=f"Image {filename} not found in session {session_id}",
            )

        # Save updated shell data
        with open(shell_file, "w", encoding="utf-8") as f:
            json.dump(shell_data, f, indent=2, default=str)

        logger.info(
            "Updated view type for %s in session %s to %s",
            filename,
            session_id,
            new_view_type,
        )
        return {
            "session_id": session_id,
            "filename": filename,
            "view_type": new_view_type,
            "message": "View type updated successfully",
        }

    except HTTPException:
        raise
    except Exception as e:
        logger.error("Error updating shell image view type: %s", e)
        raise HTTPException(status_code=500, detail=str(e)) from e


class ShellUpdateRequest(BaseModel):
    """Request model for updating shell data."""

    brand: str
    shell_type: str
    include: bool
    view_type_updates: List[Dict[str, str]] = []


@app.post("/api/ml/shells/{session_id}/update")
async def update_shell(
    session_id: str,
    request: ShellUpdateRequest,
    app_settings: Settings = Depends(get_settings),
) -> Dict[str, Any]:
    """Update shell data including metadata and view types."""
    try:
        data_dir = app_settings.data_directory
        shell_file = data_dir / f"{session_id}.json"

        if not shell_file.exists():
            raise HTTPException(status_code=404, detail="Shell data not found")

        # Load shell data
        with open(shell_file, "r", encoding="utf-8") as f:
            shell_data = json.load(f)

        # Update basic shell metadata
        shell_data["brand"] = request.brand.strip()
        shell_data["shell_type"] = request.shell_type.strip()
        shell_data["include"] = request.include

        # Update view types for specific images
        if request.view_type_updates and "captured_images" in shell_data:
            for update in request.view_type_updates:
                filename = update.get("filename")
                new_view_type = update.get("view_type")

                # Update in captured_images
                for image in shell_data["captured_images"]:
                    if image.get("filename") == filename:
                        image["view_type"] = new_view_type
                        break

        # Save updated shell data
        with open(shell_file, "w", encoding="utf-8") as f:
            json.dump(shell_data, f, indent=2, default=str)

        logger.info(
            "Updated shell data for session %s: brand=%s, type=%s, include=%s, view_type_updates=%d",
            session_id,
            request.brand,
            request.shell_type,
            request.include,
            len(request.view_type_updates),
        )

        return {"session_id": session_id, "message": "Shell updated successfully"}

    except HTTPException:
        raise
    except Exception as e:
        logger.error("Error updating shell: %s", e)
        raise HTTPException(status_code=500, detail=str(e)) from e


@app.delete("/api/ml/shells/{session_id}")
async def delete_shell(
    session_id: str,
    app_settings: Settings = Depends(get_settings),
) -> Dict[str, str]:
    """Delete a shell and all its associated images."""
    try:
        data_dir = app_settings.data_directory
        shell_file = data_dir / f"{session_id}.json"
        images_dir = Path("images")

        if not shell_file.exists():
            raise HTTPException(status_code=404, detail="Shell data not found")

        # Load shell data to get image filenames
        with open(shell_file, "r", encoding="utf-8") as f:
            shell_data = json.load(f)

        # Delete all associated image files
        deleted_images = 0
        image_filenames = shell_data.get("image_filenames", [])
        for filename in image_filenames:
            image_path = images_dir / filename
            if image_path.exists():
                try:
                    image_path.unlink()
                    deleted_images += 1
                    logger.info("Deleted image file: %s", filename)
                except Exception as e:
                    logger.warning("Could not delete image %s: %s", filename, e)

        # Delete metadata file if it exists
        metadata_file = images_dir / f"{session_id}_metadata.json"
        if metadata_file.exists():
            try:
                metadata_file.unlink()
                logger.info("Deleted metadata file: %s", metadata_file.name)
            except Exception as e:
                logger.warning(
                    "Could not delete metadata file %s: %s", metadata_file.name, e
                )

        # Delete composite image if it exists
        composites_dir = data_dir / "composites"
        composite_file = composites_dir / f"{session_id}_composite.jpg"
        if composite_file.exists():
            try:
                composite_file.unlink()
                logger.info("Deleted composite image: %s", composite_file.name)
            except Exception as e:
                logger.warning(
                    "Could not delete composite %s: %s", composite_file.name, e
                )

        # Delete shell data file
        shell_file.unlink()
        logger.info("Deleted shell data file: %s", shell_file.name)

        return {
            "message": f"Shell deleted successfully (removed {deleted_images} images)"
        }

    except HTTPException:
        raise
    except Exception as e:
        logger.error("Error deleting shell %s: %s", session_id, e)
        raise HTTPException(status_code=500, detail=str(e)) from e


@app.delete("/api/ml/shells/{session_id}/images/{filename}")
async def delete_shell_image(
    session_id: str,
    filename: str,
    app_settings: Settings = Depends(get_settings),
) -> Dict[str, str]:
    """Delete a specific image from a shell."""
    try:
        data_dir = app_settings.data_directory
        shell_file = data_dir / f"{session_id}.json"
        images_dir = Path("images")

        if not shell_file.exists():
            raise HTTPException(status_code=404, detail="Shell data not found")

        # Load shell data
        with open(shell_file, "r", encoding="utf-8") as f:
            shell_data = json.load(f)

        # Check if image exists in shell data
        image_filenames = shell_data.get("image_filenames", [])
        if filename not in image_filenames:
            raise HTTPException(status_code=404, detail="Image not found in shell data")

        # Remove from image_filenames list
        shell_data["image_filenames"] = [f for f in image_filenames if f != filename]

        # Remove from captured_images if it exists
        if "captured_images" in shell_data:
            shell_data["captured_images"] = [
                img
                for img in shell_data["captured_images"]
                if img.get("filename") != filename
            ]

        # Delete the actual image file
        image_path = images_dir / filename
        if image_path.exists():
            try:
                image_path.unlink()
                logger.info("Deleted image file: %s", filename)
            except Exception as e:
                logger.warning("Could not delete image file %s: %s", filename, e)

        # Save updated shell data
        with open(shell_file, "w", encoding="utf-8") as f:
            json.dump(shell_data, f, indent=2, default=str)

        logger.info("Removed image %s from shell %s", filename, session_id)

        return {"message": f"Image {filename} deleted successfully"}

    except HTTPException:
        raise
    except Exception as e:
        logger.error(
            "Error deleting image %s from shell %s: %s", filename, session_id, e
        )
        raise HTTPException(status_code=500, detail=str(e)) from e


class RegionUpdateRequest(BaseModel):
    """Request model for updating region data on an image."""

    region_x: int
    region_y: int
    region_width: int
    region_height: int


@app.post("/api/ml/shells/{session_id}/images/{filename}/region")
async def update_image_region(
    session_id: str,
    filename: str,
    request: RegionUpdateRequest,
    app_settings: Settings = Depends(get_settings),
) -> Dict[str, str]:
    """Update region data for a specific image in a shell."""
    try:
        data_dir = app_settings.data_directory
        shell_file = data_dir / f"{session_id}.json"

        if not shell_file.exists():
            raise HTTPException(status_code=404, detail="Shell data not found")

        # Load shell data
        with open(shell_file, "r", encoding="utf-8") as f:
            shell_data = json.load(f)

        # Check if image exists in shell data
        image_filenames = shell_data.get("image_filenames", [])
        if filename not in image_filenames:
            raise HTTPException(status_code=404, detail="Image not found in shell data")

        # Update region in captured_images if it exists
        if "captured_images" in shell_data:
            for image in shell_data["captured_images"]:
                if image.get("filename") == filename:
                    image["region_x"] = request.region_x
                    image["region_y"] = request.region_y
                    image["region_width"] = request.region_width
                    image["region_height"] = request.region_height
                    break
            else:
                # Image not found in captured_images, create new entry
                shell_data["captured_images"].append(
                    {
                        "filename": filename,
                        "region_x": request.region_x,
                        "region_y": request.region_y,
                        "region_width": request.region_width,
                        "region_height": request.region_height,
                        "view_type": "unknown",
                        "camera_index": 0,
                        "camera_name": "unknown",
                    }
                )
        else:
            # Create captured_images structure
            shell_data["captured_images"] = [
                {
                    "filename": filename,
                    "region_x": request.region_x,
                    "region_y": request.region_y,
                    "region_width": request.region_width,
                    "region_height": request.region_height,
                    "view_type": "unknown",
                    "camera_index": 0,
                    "camera_name": "unknown",
                }
            ]

        # Save updated shell data
        with open(shell_file, "w", encoding="utf-8") as f:
            json.dump(shell_data, f, indent=2, default=str)

        logger.info(
            "Updated region for image %s in shell %s: (%d,%d) %dx%d",
            filename,
            session_id,
            request.region_x,
            request.region_y,
            request.region_width,
            request.region_height,
        )

        return {"message": f"Region updated for image {filename}"}

    except HTTPException:
        raise
    except Exception as e:
        logger.error(
            "Error updating region for image %s in shell %s: %s",
            filename,
            session_id,
            e,
        )
        raise HTTPException(status_code=500, detail=str(e)) from e


@app.delete("/api/ml/shells/{session_id}/images/{filename}/region")
async def clear_image_region(
    session_id: str,
    filename: str,
    app_settings: Settings = Depends(get_settings),
) -> Dict[str, str]:
    """Clear region data for a specific image in a shell."""
    try:
        data_dir = app_settings.data_directory
        shell_file = data_dir / f"{session_id}.json"

        if not shell_file.exists():
            raise HTTPException(status_code=404, detail="Shell data not found")

        # Load shell data
        with open(shell_file, "r", encoding="utf-8") as f:
            shell_data = json.load(f)

        # Check if image exists in shell data
        image_filenames = shell_data.get("image_filenames", [])
        if filename not in image_filenames:
            raise HTTPException(status_code=404, detail="Image not found in shell data")

        # Clear region in captured_images if it exists
        if "captured_images" in shell_data:
            for image in shell_data["captured_images"]:
                if image.get("filename") == filename:
                    image["region_x"] = None
                    image["region_y"] = None
                    image["region_width"] = None
                    image["region_height"] = None
                    break

        # Save updated shell data
        with open(shell_file, "w", encoding="utf-8") as f:
            json.dump(shell_data, f, indent=2, default=str)

        logger.info("Cleared region for image %s in shell %s", filename, session_id)

        return {"message": f"Region cleared for image {filename}"}

    except HTTPException:
        raise
    except Exception as e:
        logger.error(
            "Error clearing region for image %s in shell %s: %s",
            filename,
            session_id,
            e,
        )
        raise HTTPException(status_code=500, detail=str(e)) from e


@app.post("/api/ml/shells/{session_id}/composite")
async def regenerate_shell_composite(
    session_id: str,
    app_settings: Settings = Depends(get_settings),
) -> Dict[str, str]:
    """Regenerate composite image for a specific shell."""
    try:
        data_dir = app_settings.data_directory
        shell_file = data_dir / f"{session_id}.json"
        composites_dir = data_dir / "composites"

        if not shell_file.exists():
            raise HTTPException(status_code=404, detail="Shell data not found")

        # Ensure composites directory exists
        composites_dir.mkdir(exist_ok=True)

        # Load shell data
        with open(shell_file, "r", encoding="utf-8") as f:
            shell_data = json.load(f)

        # Check if shell is included in training
        if shell_data.get("include", True) is False:
            raise HTTPException(
                status_code=400, detail="Shell is not included in training"
            )

        # Generate composite for this shell
        composite_path = await _generate_composite_image(
            session_id, shell_data, composites_dir
        )

        if composite_path:
            logger.info(
                "Regenerated composite image for shell %s: %s",
                session_id,
                composite_path,
            )
            return {"message": f"Composite image regenerated for shell {session_id}"}
        else:
            raise HTTPException(
                status_code=500, detail="Failed to generate composite image"
            )

    except HTTPException:
        raise
    except Exception as e:
        logger.error("Error regenerating composite for shell %s: %s", session_id, e)
        raise HTTPException(status_code=500, detail=str(e)) from e


async def _generate_composite_image(
    session_id: str, shell_data: Dict[str, Any], output_dir: Path
) -> Optional[Path]:
    """Generate a composite image from multiple shell images using region data."""
    if cv2 is None or np is None:
        logger.error("OpenCV or NumPy not available for composite image generation")
        return None

    try:
        images_dir = Path("images")
        composite_path = output_dir / f"{session_id}_composite.jpg"
        image_filenames = shell_data.get("image_filenames", [])
        captured_images = shell_data.get("captured_images", [])

        # Create a lookup for region data by filename
        region_lookup = {}
        if captured_images:
            for img_info in captured_images:
                region_lookup[img_info["filename"]] = img_info

        # Load all images and apply region processing
        loaded_images = []
        for filename in image_filenames:
            image_path = images_dir / filename
            if image_path.exists():
                img = cv2.imread(str(image_path))
                if img is not None:
                    # Apply region cropping if available
                    if filename in region_lookup:
                        img = _apply_region_processing(img, region_lookup[filename])

                    # Resize to standard size
                    img = cv2.resize(img, (200, 200))
                    loaded_images.append(img)

        if not loaded_images:
            return None

        # Create composite layout
        if len(loaded_images) == 1:
            composite = loaded_images[0]
        elif len(loaded_images) == 2:
            composite = np.hstack(loaded_images)
        elif len(loaded_images) == 3:
            top = loaded_images[0]
            bottom = np.hstack(loaded_images[1:3])
            # Pad bottom if needed
            if bottom.shape[1] > top.shape[1]:
                pad_width = (bottom.shape[1] - top.shape[1]) // 2
                top = cv2.copyMakeBorder(
                    top, 0, 0, pad_width, pad_width, cv2.BORDER_CONSTANT
                )
            composite = np.vstack([top, bottom])
        else:
            # 4+ images: create 2x2 grid
            rows = []
            for i in range(0, len(loaded_images), 2):
                row_images = loaded_images[i : i + 2]
                if len(row_images) == 1:
                    # Pad with blank image
                    blank = np.zeros_like(row_images[0])
                    row_images.append(blank)
                row = np.hstack(row_images)
                rows.append(row)
            composite = np.vstack(rows)

        # Save composite image
        cv2.imwrite(str(composite_path), composite)
        return composite_path

    except Exception as e:
        logger.error("Error creating composite image for %s: %s", session_id, e)
        return None


def _apply_region_processing(img: Any, region_info: Dict[str, Any]) -> Any:
    """Apply region processing to an image based on camera view type and region data."""
    if cv2 is None or np is None:
        return img

    view_type = region_info.get("view_type")
    region_x = region_info.get("region_x")
    region_y = region_info.get("region_y")
    region_width = region_info.get("region_width")
    region_height = region_info.get("region_height")

    # If region data is available, crop to region
    if all(x is not None for x in [region_x, region_y, region_width, region_height]):
        h, w = img.shape[:2]

        # Convert to int (we know they're not None due to the check above)
        assert region_x is not None
        assert region_y is not None
        assert region_width is not None
        assert region_height is not None
        region_x_int = int(region_x)
        region_y_int = int(region_y)
        region_width_int = int(region_width)
        region_height_int = int(region_height)

        # Ensure region is within image bounds
        x1 = max(0, min(region_x_int, w))
        y1 = max(0, min(region_y_int, h))
        x2 = max(0, min(region_x_int + region_width_int, w))
        y2 = max(0, min(region_y_int + region_height_int, h))

        if x2 > x1 and y2 > y1:
            img = img[y1:y2, x1:x2]

    # Apply view-specific processing
    if view_type == "tail":
        # For tail view, try to detect and focus on circular features
        img = _apply_tail_view_processing(img)

    return img


def _apply_tail_view_processing(img: Any) -> Any:
    """Apply circular detection and filtering for tail view cameras."""
    if cv2 is None or np is None:
        return img

    try:
        # Convert to grayscale for circle detection
        gray = cv2.cvtColor(img, cv2.COLOR_BGR2GRAY)

        # Apply Gaussian blur to reduce noise
        blurred = cv2.GaussianBlur(gray, (9, 9), 2)

        # Detect circles using HoughCircles
        circles = cv2.HoughCircles(
            blurred,
            cv2.HOUGH_GRADIENT,
            dp=1,
            minDist=30,
            param1=50,
            param2=30,
            minRadius=10,
            maxRadius=min(img.shape[:2]) // 2,
        )

        if circles is not None:
            circles = np.round(circles[0, :]).astype("int")

            # Find the largest circle (likely the case end)
            largest_circle = max(circles, key=lambda c: c[2])
            x, y, r = largest_circle

            # Create a mask for the circular region
            mask = np.zeros(gray.shape, dtype=np.uint8)
            cv2.circle(mask, (x, y), r, 255, -1)

            # Apply the mask to the original image
            result = cv2.bitwise_and(img, img, mask=mask)

            # Crop to the circle bounding box with some padding
            padding = 10
            x1 = max(0, x - r - padding)
            y1 = max(0, y - r - padding)
            x2 = min(img.shape[1], x + r + padding)
            y2 = min(img.shape[0], y + r + padding)

            result = result[y1:y2, x1:x2]

            # If the result is too small, return the original
            if result.shape[0] > 20 and result.shape[1] > 20:
                return result

    except Exception as e:
        logger.debug("Tail view processing failed, using original image: %s", e)

    return img


# Debug Endpoints


@app.websocket("/ws/debug/esp-commands")
async def debug_esp_commands_websocket(
    websocket: WebSocket,
    hardware: HardwareController = Depends(get_hardware_controller),
) -> None:
    """WebSocket endpoint for real-time ESP command debugging."""
    await debug_ws_manager.connect(websocket)

    try:
        # Send existing command history on connection
        commands = hardware.get_command_history()
        for cmd in commands:
            command_data = {
                "timestamp": cmd.timestamp,
                "command": cmd.command,
                "url": cmd.url,
                "status": cmd.status,
                "response": cmd.response,
            }
            await websocket.send_json(command_data)

        # Keep connection alive and handle client messages
        while True:
            # Wait for client messages (ping/pong to keep connection alive)
            try:
                await websocket.receive_text()
            except WebSocketDisconnect:
                break

    except WebSocketDisconnect:
        pass
    except Exception as e:
        logger.error("Error in debug WebSocket: %s", e)
    finally:
        debug_ws_manager.disconnect(websocket)


@app.get("/api/debug/esp-commands")
async def get_esp_command_history(
    hardware: HardwareController = Depends(get_hardware_controller),
) -> List[Dict[str, Any]]:
    """Get ESP command history for debugging (REST fallback)."""
    try:
        commands = hardware.get_command_history()
        return [
            {
                "timestamp": cmd.timestamp,
                "command": cmd.command,
                "url": cmd.url,
                "status": cmd.status,
                "response": cmd.response,
            }
            for cmd in commands
        ]
    except Exception as e:
        logger.error("Error getting ESP command history: %s", e)
        raise HTTPException(status_code=500, detail=str(e)) from e


def signal_handler(signum: int, _frame: Any) -> None:
    """Handle shutdown signals gracefully."""
    print(f"\nReceived signal {signum}, shutting down...")
    camera_manager.cleanup()
    sys.exit(0)


def main() -> None:
    """Main application entry point - start the FastAPI server with graceful shutdown handling."""
    # Set up signal handlers for graceful shutdown
    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)

    print("Shell Sorter Machine Control Panel")
    print(f"Web interface available at: http://{settings.host}:{settings.port}")
    print("Press Ctrl+C to stop the server")

    try:
        uvicorn.run(app, host=settings.host, port=settings.port)
    except KeyboardInterrupt:
        print("\nKeyboard interrupt received, shutting down...")
        camera_manager.cleanup()
        sys.exit(0)


if __name__ == "__main__":
    main()
