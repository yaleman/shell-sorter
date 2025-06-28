import uvicorn
from fastapi import FastAPI, Request, Form, File, UploadFile, HTTPException, Depends
from fastapi.responses import HTMLResponse
from fastapi.staticfiles import StaticFiles
from fastapi.templating import Jinja2Templates
from pathlib import Path
from typing import List, Dict, Any, Optional
from datetime import datetime
import logging

from .config import Settings
from .ml_trainer import MLTrainer


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
        self.machine_status.update({
            "status": "running",
            "current_job": new_job,
            "last_update": datetime.now().isoformat(),
        })
        
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
        
        self.machine_status.update({
            "status": "idle",
            "current_job": None,
            "last_update": datetime.now().isoformat(),
        })


# Initialize settings on startup
settings = Settings.new()

# Initialize ML trainer
ml_trainer = MLTrainer(settings)

# Initialize machine controller
machine_controller = MachineController()

# Setup logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

app = FastAPI(title="Shell Sorter Control Panel", version="1.0.0")

# Setup templates and static files
templates_dir = Path(__file__).parent / "templates"
static_dir = Path(__file__).parent / "static"
templates_dir.mkdir(exist_ok=True)
static_dir.mkdir(exist_ok=True)

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
        raise HTTPException(status_code=500, detail=str(e))


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
        raise HTTPException(status_code=500, detail=str(e))


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
        raise HTTPException(status_code=500, detail=str(e))


def simulate_sorting_machine() -> None:
    print("Shell Sorter Machine Control Panel")
    print(f"Web interface available at: http://{settings.host}:{settings.port}")


def main() -> None:
    simulate_sorting_machine()
    uvicorn.run(app, host=settings.host, port=settings.port)


if __name__ == "__main__":
    main()
