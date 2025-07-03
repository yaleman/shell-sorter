"""Configuration management for the Shell Sorter application.

This module provides configuration classes for managing application settings,
camera configurations, and user preferences. It uses Pydantic for validation
and supports environment variable overrides.
"""

import json
import logging
import os
import sys
import tempfile
from pathlib import Path
from typing import Annotated, Any, Dict, List, Literal, Optional

from pydantic import BaseModel, Field
from pydantic_settings import BaseSettings, SettingsConfigDict  # type: ignore

logger = logging.getLogger(__name__)


class Settings(BaseSettings):  # type: ignore
    """Configuration settings for the Shell Sorter application."""

    model_config = SettingsConfigDict(
        env_file=".env",
        env_file_encoding="utf-8",
        env_nested_delimiter="__",
        env_prefix="SHELL_SORTER_",
    )

    # Server configuration
    host: str = Field(default="127.0.0.1", description="Server host address")
    port: int = Field(default=8000, description="Server port")
    debug: bool = Field(default=False, description="Enable debug mode")

    # Machine configuration
    machine_name: str = Field(default="Shell Sorter v1.0", description="Machine identifier")
    max_sorting_jobs: int = Field(default=10, description="Maximum concurrent sorting jobs")

    # Camera configuration
    cameras: list[str] = Field(default_factory=list, description="Camera device paths")
    camera_count: int = Field(default=4, description="Number of cameras on the machine")
    camera_resolution: str = Field(default="1920x1080", description="Camera resolution")

    # File storage paths
    image_directory: Path = Field(default=Path("./images"), description="Training images directory")
    data_directory: Path = Field(default=Path("./data"), description="Data directory for uploads and models")
    models_directory: Path = Field(default=Path("./data/models"), description="ML models directory")
    references_directory: Path = Field(default=Path("./data/references"), description="Reference images directory")

    # Machine Learning configuration
    ml_enabled: bool = Field(default=True, description="Enable ML case identification")
    confidence_threshold: float = Field(default=0.8, description="ML confidence threshold")
    model_name: Optional[str] = Field(default=None, description="Active ML model name")

    # Supported case types
    supported_case_types: list[str] = Field(
        default=[
            "9mm",
            "40sw",
            "45acp",
            "223rem",
            "308win",
            "3006spr",
            "38special",
            "357mag",
        ],
        description="Supported ammunition case types",
    )

    # ESPHome configuration
    esphome_hostname: str = Field(
        default="shell-sorter-controller.local",
        description="ESPHome device hostname for API communication",
    )

    # Network camera configuration
    network_camera_hostnames: List[str] = Field(
        default=["esp32cam1.local"],
        description="List of ESPHome camera hostnames to detect",
    )
    auto_detect_cameras: bool = Field(
        default=False,
        description="Automatically detect and configure cameras on startup",
    )
    auto_start_esp32_cameras: bool = Field(
        default=True,
        description="Automatically start configured ESP32 cameras when they come online",
    )

    def get_config_path(self) -> Path:
        """Get the path to the user config file."""
        # Allow override via environment variable for testing
        config_path_override = os.getenv("SHELL_SORTER_CONFIG_PATH")
        if config_path_override:
            config_path = Path(config_path_override)
            config_path.parent.mkdir(parents=True, exist_ok=True)
            return config_path

        # Auto-detect pytest and use temporary config to prevent touching live config
        if "pytest" in sys.modules or "PYTEST_CURRENT_TEST" in os.environ:
            # Use a temporary directory for pytest runs
            temp_dir = Path(tempfile.gettempdir()) / "shell-sorter-pytest"
            temp_dir.mkdir(exist_ok=True)
            return temp_dir / "test-config.json"

        # Default to ~/.config/shell-sorter.json for production
        config_dir = Path.home() / ".config"
        config_dir.mkdir(exist_ok=True)
        return config_dir / "shell-sorter.json"

    def load_user_config(self) -> Dict[str, Any]:
        """Load user configuration from shell-sorter.json."""
        config_path = self.get_config_path()
        if not config_path.exists():
            return {}

        try:
            with open(config_path, "r", encoding="utf-8") as f:
                config_data: Dict[str, Any] = json.load(f)
                return config_data
        except Exception as e:
            logger.warning("Failed to load user config from %s: %s", config_path, e)
            return {}

    def save_user_config(self, config_data: Dict[str, Any]) -> bool:
        """Save user configuration to shell-sorter.json."""
        config_path = self.get_config_path()

        try:
            # Ensure directory exists
            config_path.parent.mkdir(exist_ok=True)

            with open(config_path, "w", encoding="utf-8") as f:
                json.dump(config_data, f, indent=2)

            logger.info("Saved user config to %s", config_path)
            return True
        except Exception as e:
            logger.error("Failed to save user config to %s: %s", config_path, e)
            return False

    @classmethod
    def new(cls, **kwargs: Any) -> "Settings":
        """Create a new instance of Settings with the provided keyword arguments."""
        retval = cls.model_validate(kwargs)

        # Create all necessary directories
        directories = [
            retval.image_directory,
            retval.data_directory,
            retval.models_directory,
            retval.references_directory,
        ]

        for directory in directories:
            if not directory.exists():
                directory.mkdir(parents=True, exist_ok=True)

        return retval  # type: ignore


class CameraConfig(BaseModel):
    """Configuration for a specific camera."""

    view_type: Optional[Literal["side", "tail"]] = None
    region_x: Optional[int] = None
    region_y: Optional[int] = None
    region_width: Optional[int] = None
    region_height: Optional[int] = None
    # Resolution configuration for ESP cameras
    detected_resolution_width: Optional[int] = None
    detected_resolution_height: Optional[int] = None
    manual_resolution_width: Optional[int] = None
    manual_resolution_height: Optional[int] = None
    resolution_detection_timestamp: Optional[float] = None


class UserConfig(BaseModel):
    """User configuration that persists across application restarts."""

    camera_configs: Annotated[Dict[str, CameraConfig], Field(default={}, description="Camera configurations by name")]
    network_camera_hostnames: List[str] = Field(
        default=["esp32cam1.local"],
        description="List of ESPHome camera hostnames to detect",
    )
    auto_detect_cameras: bool = Field(
        default=False,
        description="Automatically detect and configure cameras on startup",
    )

    def get_camera_config(self, camera_name: str) -> CameraConfig:
        """Get configuration for a camera by name."""
        return self.camera_configs.get(camera_name, CameraConfig())

    def set_camera_config(self, camera_name: str, config: CameraConfig) -> None:
        """Set configuration for a camera by name."""
        self.camera_configs[camera_name] = config

    def clear_camera_config(self, camera_name: str) -> None:
        """Clear configuration for a camera by name."""
        if camera_name in self.camera_configs:
            del self.camera_configs[camera_name]

    def remove_camera_config(self, camera_name: str) -> None:
        """Remove configuration for a camera by name (alias for clear_camera_config)."""
        self.clear_camera_config(camera_name)
