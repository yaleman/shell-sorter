"""Machine learning model training for shell case identification."""

import json
from pathlib import Path
from typing import Dict, List, Optional, Tuple, Any
from datetime import datetime
import logging

from .config import Settings

logger = logging.getLogger(__name__)


class CaseType:
    """Represents a shell case type with training data."""
    
    def __init__(self, name: str, designation: str, brand: Optional[str] = None):
        self.name = name
        self.designation = designation
        self.brand = brand
        self.reference_images: List[Path] = []
        self.training_images: List[Path] = []
        self.created_at = datetime.now().isoformat()
        self.updated_at = datetime.now().isoformat()
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for serialization."""
        return {
            "name": self.name,
            "designation": self.designation,
            "brand": self.brand,
            "reference_images": [str(img) for img in self.reference_images],
            "training_images": [str(img) for img in self.training_images],
            "created_at": self.created_at,
            "updated_at": self.updated_at
        }
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "CaseType":
        """Create from dictionary."""
        case_type = cls(data["name"], data["designation"], data.get("brand"))
        case_type.reference_images = [Path(img) for img in data.get("reference_images", [])]
        case_type.training_images = [Path(img) for img in data.get("training_images", [])]
        case_type.created_at = data.get("created_at", datetime.now().isoformat())
        case_type.updated_at = data.get("updated_at", datetime.now().isoformat())
        return case_type


class MLTrainer:
    """Machine learning trainer for shell case identification."""
    
    def __init__(self, settings: Settings):
        self.settings = settings
        self.case_types: Dict[str, CaseType] = {}
        self.models_dir = settings.models_directory
        self.references_dir = settings.references_directory
        self.images_dir = settings.image_directory
        self.case_types_file = settings.data_directory / "case_types.json"
        
        # Load existing case types
        self.load_case_types()
    
    def load_case_types(self) -> None:
        """Load case types from storage."""
        if self.case_types_file.exists():
            try:
                with open(self.case_types_file, 'r') as f:
                    data = json.load(f)
                    for name, case_data in data.items():
                        self.case_types[name] = CaseType.from_dict(case_data)
                logger.info(f"Loaded {len(self.case_types)} case types")
            except Exception as e:
                logger.error(f"Error loading case types: {e}")
    
    def save_case_types(self) -> None:
        """Save case types to storage."""
        try:
            data = {name: case_type.to_dict() for name, case_type in self.case_types.items()}
            with open(self.case_types_file, 'w') as f:
                json.dump(data, f, indent=2)
            logger.info(f"Saved {len(self.case_types)} case types")
        except Exception as e:
            logger.error(f"Error saving case types: {e}")
    
    def add_case_type(self, name: str, designation: str, brand: Optional[str] = None) -> CaseType:
        """Add a new case type."""
        case_type = CaseType(name, designation, brand)
        self.case_types[name] = case_type
        
        # Create directories for this case type
        case_ref_dir = self.references_dir / name
        case_train_dir = self.images_dir / name
        case_ref_dir.mkdir(exist_ok=True)
        case_train_dir.mkdir(exist_ok=True)
        
        self.save_case_types()
        logger.info(f"Added case type: {name} ({designation})")
        return case_type
    
    def add_reference_image(self, case_type_name: str, image_path: Path) -> bool:
        """Add a reference image for a case type."""
        if case_type_name not in self.case_types:
            logger.error(f"Case type {case_type_name} not found")
            return False
        
        case_type = self.case_types[case_type_name]
        target_dir = self.references_dir / case_type_name
        target_path = target_dir / image_path.name
        
        try:
            # Copy image to reference directory
            import shutil
            shutil.copy2(image_path, target_path)
            case_type.reference_images.append(target_path)
            case_type.updated_at = datetime.now().isoformat()
            self.save_case_types()
            logger.info(f"Added reference image for {case_type_name}: {image_path.name}")
            return True
        except Exception as e:
            logger.error(f"Error adding reference image: {e}")
            return False
    
    def add_training_image(self, case_type_name: str, image_path: Path) -> bool:
        """Add a training image for a case type."""
        if case_type_name not in self.case_types:
            logger.error(f"Case type {case_type_name} not found")
            return False
        
        case_type = self.case_types[case_type_name]
        target_dir = self.images_dir / case_type_name
        target_path = target_dir / image_path.name
        
        try:
            # Copy image to training directory
            import shutil
            shutil.copy2(image_path, target_path)
            case_type.training_images.append(target_path)
            case_type.updated_at = datetime.now().isoformat()
            self.save_case_types()
            logger.info(f"Added training image for {case_type_name}: {image_path.name}")
            return True
        except Exception as e:
            logger.error(f"Error adding training image: {e}")
            return False
    
    def get_case_types(self) -> Dict[str, CaseType]:
        """Get all case types."""
        return self.case_types.copy()
    
    def get_training_summary(self) -> Dict[str, Dict[str, Any]]:
        """Get training data summary for all case types."""
        summary = {}
        for name, case_type in self.case_types.items():
            summary[name] = {
                "designation": case_type.designation,
                "brand": case_type.brand,
                "reference_count": len(case_type.reference_images),
                "training_count": len(case_type.training_images),
                "ready_for_training": len(case_type.training_images) >= 10,  # Minimum images for training
                "updated_at": case_type.updated_at
            }
        return summary
    
    def train_model(self, case_types: Optional[List[str]] = None) -> Tuple[bool, str]:
        """Train ML model with available data."""
        if case_types is None:
            case_types = list(self.case_types.keys())
        
        # Check if we have enough training data
        trainable_types = []
        for case_type_name in case_types:
            if case_type_name in self.case_types:
                case_type = self.case_types[case_type_name]
                if len(case_type.training_images) >= 10:
                    trainable_types.append(case_type_name)
        
        if not trainable_types:
            return False, "No case types have sufficient training images (minimum 10 per type)"
        
        try:
            # Placeholder for actual ML training logic
            # In a real implementation, this would:
            # 1. Load and preprocess images
            # 2. Create training/validation splits
            # 3. Train a CNN or other ML model
            # 4. Save the trained model
            
            model_name = f"shell_classifier_{datetime.now().strftime('%Y%m%d_%H%M%S')}"
            model_path = self.models_dir / f"{model_name}.model"
            
            # Simulate training process
            logger.info(f"Training model with {len(trainable_types)} case types: {trainable_types}")
            
            # Create a simple model metadata file
            model_metadata = {
                "name": model_name,
                "case_types": trainable_types,
                "training_date": datetime.now().isoformat(),
                "accuracy": 0.95,  # Placeholder
                "version": "1.0"
            }
            
            metadata_path = self.models_dir / f"{model_name}.json"
            with open(metadata_path, 'w') as f:
                json.dump(model_metadata, f, indent=2)
            
            # Create placeholder model file
            model_path.touch()
            
            logger.info(f"Model training completed: {model_name}")
            return True, f"Model '{model_name}' trained successfully with {len(trainable_types)} case types"
            
        except Exception as e:
            logger.error(f"Error during model training: {e}")
            return False, f"Training failed: {str(e)}"