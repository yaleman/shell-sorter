"""Handles the WebSocket connections for the debug console."""

from typing import Any, Dict, List
from logging import getLogger
from fastapi import WebSocket

logger = getLogger(__name__)


# Initialize debug WebSocket manager early (needed for hardware controller callback)
class DebugWebSocketManager:
    """Manage WebSocket connections for debug console."""

    def __init__(self) -> None:
        self.active_connections: List[WebSocket] = []

    async def connect(self, websocket: WebSocket) -> None:
        """Accept a new WebSocket connection."""
        await websocket.accept()
        self.active_connections.append(websocket)
        logger.info(
            "Debug WebSocket connected, total connections: %d",
            len(self.active_connections),
        )

    def disconnect(self, websocket: WebSocket) -> None:
        """Remove a WebSocket connection."""
        if websocket in self.active_connections:
            self.active_connections.remove(websocket)
        logger.info(
            "Debug WebSocket disconnected, total connections: %d",
            len(self.active_connections),
        )

    async def broadcast_command(self, command_data: Dict[str, Any]) -> None:
        """Broadcast command data to all connected clients."""
        if not self.active_connections:
            return

        disconnected_connections = []
        for connection in self.active_connections:
            try:
                await connection.send_json(command_data)
            except Exception as e:
                logger.warning("Failed to send to WebSocket connection: %s", e)
                disconnected_connections.append(connection)

        # Remove failed connections
        for connection in disconnected_connections:
            self.disconnect(connection)
