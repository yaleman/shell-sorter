from pathlib import Path
from typing import List, Dict, Any, Optional, Generator, Callable, Awaitable
from datetime import datetime
import logging
import signal
import sys
from fastapi import FastAPI, Request, Form, File, UploadFile, HTTPException, Depends, BackgroundTasks
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


class NoCacheMiddleware(BaseHTTPMiddleware):
    """Middleware to add no-cache headers to prevent browser caching."""

    async def dispatch(
        self, request: Request, call_next: Callable[[Request], Awaitable[Response]]
    ) -> Response:
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


class MachineController:
    """Controls the sorting machine state and job management."""

    def __init__(self) -> None:
        self.machine_status: Dict[str, Any] = {
            "status": "idle",
            "current_job": None,
            "total_sorted": 0,
            "last_update": datetime.now().isoformat(),
        }
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

    def start_sorting(self, case_type: str, quantity: int) -> Dict[str, Any]:
        """Start a new sorting job."""
        job_id = len(self.sorting_jobs) + 1
        new_job: Dict[str, Any] = {
            "id": job_id,
            "case_type": case_type,
            "quantity": quantity,
            "status": "running",
            "started_at": datetime.now().isoformat(),
            "completed_at": None,
        }

        self.sorting_jobs.append(new_job)
        self.machine_status.update(
            {
                "status": "running",
                "current_job": new_job,
                "last_update": datetime.now().isoformat(),
            }
        )

        return new_job

    def stop_sorting(self) -> None:
        """Stop the current sorting job."""
        if self.machine_status["current_job"]:
            current_job = self.machine_status["current_job"]
            if isinstance(current_job, dict) and "id" in current_job:
                for job in self.sorting_jobs:
                    if job["id"] == current_job["id"]:
                        job["status"] = "stopped"
                        job["completed_at"] = datetime.now().isoformat()
                        break

        self.machine_status.update(
            {
                "status": "idle",
                "current_job": None,
                "last_update": datetime.now().isoformat(),
            }
        )


# Initialize settings on startup
settings = Settings.new()

# Initialize ML trainer
ml_trainer = MLTrainer(settings)

# Initialize machine controller
machine_controller = MachineController()

# Initialize camera manager
camera_manager = CameraManager()

# Setup logging with timestamps including milliseconds
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s.%(msecs)03d - %(name)s - %(levelname)s - %(message)s',
    datefmt='%Y-%m-%d %H:%M:%S'
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
    return templates.TemplateResponse(
        "dashboard.html",
        {
            "request": request,
            "machine_status": controller.get_status(),
            "recent_jobs": controller.get_recent_jobs(),
            "supported_case_types": app_settings.supported_case_types,
            "cameras": camera_manager.get_cameras(),
        },
    )


@app.get("/api/status")
async def get_status(
    controller: MachineController = Depends(get_machine_controller),
) -> Dict[str, Any]:
    return controller.get_status()


@app.post("/api/start-sorting")
async def start_sorting(
    case_type: str = Form(...),
    quantity: int = Form(...),
    controller: MachineController = Depends(get_machine_controller),
) -> Dict[str, Any]:
    new_job = controller.start_sorting(case_type, quantity)
    return {"message": f"Started sorting {quantity} {case_type} cases", "job": new_job}


@app.post("/api/stop-sorting")
async def stop_sorting(
    controller: MachineController = Depends(get_machine_controller),
) -> Dict[str, str]:
    controller.stop_sorting()
    return {"message": "Sorting stopped"}


@app.get("/api/jobs")
async def get_jobs(
    controller: MachineController = Depends(get_machine_controller),
) -> List[Dict[str, Any]]:
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
        raise HTTPException(
            status_code=400, detail=f"Camera {camera_index} not found"
        )
    
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


def signal_handler(signum: int, _frame: Any) -> None:
    """Handle shutdown signals gracefully."""
    print(f"\nReceived signal {signum}, shutting down...")
    camera_manager.cleanup()
    sys.exit(0)


def main() -> None:
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
