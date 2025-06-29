"""Shell model for shell sorting application."""

from datetime import datetime
from enum import StrEnum
from typing import Optional, List
from pydantic import BaseModel, Field


class ViewType(StrEnum):
    """Enumeration for camera view types."""
    SIDE = "side"
    TAIL = "tail" 
    UNKNOWN = "unknown"


class CameraRegion(BaseModel):
    """Camera region information for image processing."""
    view_type: ViewType = ViewType.UNKNOWN
    region_x: Optional[int] = None
    region_y: Optional[int] = None
    region_width: Optional[int] = None
    region_height: Optional[int] = None


class CapturedImage(BaseModel):
    """Information about a captured image including camera and region data."""
    camera_index: int
    filename: str
    camera_name: str
    view_type: ViewType = ViewType.UNKNOWN
    region_x: Optional[int] = None
    region_y: Optional[int] = None
    region_width: Optional[int] = None
    region_height: Optional[int] = None


class Shell(BaseModel):
    date_captured: datetime = Field(
        default_factory=datetime.now, description="Date when the shell was captured"
    )
    brand: str
    shell_type: str
    image_filenames: list[str]
    captured_images: Optional[List[CapturedImage]] = Field(
        default=None, description="Detailed information about captured images including camera regions"
    )
    include: bool = Field(
        default=True, description="Whether to include this shell in the training set."
    )
