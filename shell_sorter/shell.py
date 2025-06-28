"""Shell model for shell sorting application."""

from datetime import datetime
from pydantic import BaseModel, Field


class Shell(BaseModel):
    date_captured: datetime = Field(
        default_factory=datetime.now, description="Date when the shell was captured"
    )
    brand: str
    shell_type: str
    image_filenames: list[str]
    include: bool = Field(
        default=True, description="Whether to include this shell in the training set."
    )
