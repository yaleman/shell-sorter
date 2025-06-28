"""Shell model for shell sorting application."""

from datetime import datetime
from pydantic import BaseModel


class Shell(BaseModel):
    date_captured: datetime
    brand: str
    shell_type: str
    image_filename: list[str]
