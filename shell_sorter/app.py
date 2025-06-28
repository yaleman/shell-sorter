"""Main application module for the Shell Sorter Control Panel."""

from dataclasses import dataclass
from pathlib import Path
from typing import List, Dict, Any, Optional, Generator, Callable, Awaitable
from datetime import datetime
import logging
import signal
import sys
import uuid
from fastapi import (
    FastAPI,
    Request,
    Form,
    File,
    UploadFile,
    HTTPException,
    Depends,
    BackgroundTasks,
)
from fastapi.responses import HTMLResponse, StreamingResponse
from fastapi.staticfiles import StaticFiles
from fastapi.templating import Jinja2Templates
from fastapi.middleware.cors import CORSMiddleware
from starlette.middleware.base import BaseHTTPMiddleware
from starlette.responses import Response
import uvicorn

from .config import Settings
from .ml_trainer import MLTrainer
from .camera_manager import CameraManager
from .shell import Shell
from .hardware_controller import HardwareController


class NoCacheMiddleware(BaseHTTPMiddleware):
    """Middleware to add no-cache headers to prevent browser caching."""

    async def dispatch(
        self, request: Request, call_next: Callable[[Request], Awaitable[Response]]
    ) -> Response:
        """Process request and add no-cache headers to prevent browser caching."""
        response = await call_next(request)

        # Add no-cache headers for all responses
        response.headers["Cache-Control"] = (
            "no-cache, no-store, must-revalidate, max-age=0"
        )
        response.headers["Pragma"] = "no-cache"
        response.headers["Expires"] = "0"

        # Additional headers for static files (JS, CSS, HTML)
        if any(
            request.url.path.endswith(ext) for ext in [".js", ".css", ".html", ".htm"]
        ):
            response.headers["Cache-Control"] = (
                "no-cache, no-store, must-revalidate, max-age=0, private"
            )
            response.headers["ETag"] = f'"{datetime.now().timestamp()}"'

        return response


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
        return self.machine_status

    def get_jobs(self) -> List[Dict[str, Any]]:
        """Get all sorting jobs."""
        return self.sorting_jobs

    def get_recent_jobs(self, limit: int = 10) -> List[Dict[str, Any]]:
        """Get recent jobs in reverse chronological order."""
        return self.sorting_jobs[-limit:][::-1]

    def start_sorting(self) -> Dict[str, Any]:
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

# Initialize camera manager
camera_manager = CameraManager()

# Initialize hardware controller
hardware_controller = HardwareController()

# Setup logging with timestamps including milliseconds
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s.%(msecs)03d - %(name)s - %(levelname)s - %(message)s",
    datefmt="%Y-%m-%d %H:%M:%S",
)
logger = logging.getLogger(__name__)

app = FastAPI(title="Shell Sorter Control Panel", version="1.0.0")

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


def get_machine_controller() -> MachineController:
    """Dependency to get the machine controller instance."""
    return machine_controller


def get_ml_trainer() -> MLTrainer:
    """Dependency to get the ML trainer instance."""
    return ml_trainer


def get_settings() -> Settings:
    """Dependency to get the settings instance."""
    return settings


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
) -> Dict[str, Any]:
    """Start a sorting job for the specified case type and quantity."""
    new_job = controller.start_sorting(case_type, quantity)
    return {"message": f"Started sorting {quantity} {case_type} cases", "job": new_job}


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
        else:
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
        else:
            raise HTTPException(status_code=400, detail="Failed to add training image")

    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e)) from e


@app.post("/api/train-model")
async def train_model(
    case_types: Optional[List[str]] = None,
    trainer: MLTrainer = Depends(get_ml_trainer),
) -> Dict[str, Any]:
    """Train the ML model with available data."""
    try:
        success, message = trainer.train_model(case_types)
        if success:
            return {"message": message, "success": True}
        else:
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
        else:
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
        else:
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
            # Get the latest frame from the camera
            frame_data = camera_manager.get_latest_frame(camera.index)
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
                    }
                )

                logger.info("Captured image from camera %d: %s", camera.index, filename)
            else:
                logger.warning("No frame available from camera %d", camera.index)

    if not captured_images:
        raise HTTPException(
            status_code=400, detail="No images could be captured from selected cameras"
        )

    logger.info("Captured %d images for session %s", len(captured_images), session_id)

    return {
        "session_id": session_id,
        "captured_images": captured_images,
        "message": f"Captured {len(captured_images)} images from selected cameras",
    }


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

        # Create Shell object
        shell = Shell(
            date_captured=datetime.now(),
            brand=brand,
            shell_type=shell_type,
            image_filenames=filenames_list,
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
