from pathlib import Path
from typing import Optional, Any
from pydantic import Field
from pydantic_settings import BaseSettings, SettingsConfigDict  # type: ignore


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
    machine_name: str = Field(
        default="Shell Sorter v1.0", description="Machine identifier"
    )
    max_sorting_jobs: int = Field(
        default=10, description="Maximum concurrent sorting jobs"
    )

    # Camera configuration
    cameras: list[str] = Field(default_factory=list, description="Camera device paths")
    camera_count: int = Field(default=4, description="Number of cameras on the machine")
    camera_resolution: str = Field(default="1920x1080", description="Camera resolution")

    # File storage paths
    image_directory: Path = Field(
        default=Path("./images"), description="Training images directory"
    )
    data_directory: Path = Field(
        default=Path("./data"), description="Data directory for uploads and models"
    )
    models_directory: Path = Field(
        default=Path("./data/models"), description="ML models directory"
    )
    references_directory: Path = Field(
        default=Path("./data/references"), description="Reference images directory"
    )

    # Machine Learning configuration
    ml_enabled: bool = Field(default=True, description="Enable ML case identification")
    confidence_threshold: float = Field(
        default=0.8, description="ML confidence threshold"
    )
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
