"""Status module for the shell sorter application."""

from datetime import datetime
from dataclasses import dataclass


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
