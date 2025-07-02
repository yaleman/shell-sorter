"""Machine Controller for Shell Sorter"""

from typing import Any, Dict, List
from .status import Status


class MachineController:
    """Controls the sorting machine state and job management."""

    def __init__(self) -> None:
        self.machine_status: Status = Status()

        self.sorting_jobs: List[Dict[str, Any]] = []

    def get_status(self) -> Dict[str, Any]:
        """Get current machine status."""
        return {
            "status": self.machine_status.status,
            "last_update": self.machine_status.last_update,
        }

    def get_jobs(self) -> List[Dict[str, Any]]:
        """Get all sorting jobs."""
        return self.sorting_jobs

    def get_recent_jobs(self, limit: int = 10) -> List[Dict[str, Any]]:
        """Get recent jobs in reverse chronological order."""
        return self.sorting_jobs[-limit:][::-1]

    def start_sorting(self) -> None:
        """Start a new sorting job."""
        self.machine_status.set_running()

    def stop_sorting(self) -> None:
        """Stop the current sorting job."""
        self.machine_status.stop_sorting()
