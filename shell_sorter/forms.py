"""Request models for the shell sorter application."""

from typing import List, Optional, Dict
from pydantic import BaseModel


class RegionUpdateRequest(BaseModel):
    """Request model for updating region data on an image."""

    region_x: int
    region_y: int
    region_width: int
    region_height: int


class ShellUpdateRequest(BaseModel):
    """Request model for updating shell data."""

    brand: str
    shell_type: str
    include: bool
    view_type_updates: List[Dict[str, str]] = []


class TrainModelRequest(BaseModel):
    """Request model for training ML model."""

    case_types: Optional[List[str]] = None
