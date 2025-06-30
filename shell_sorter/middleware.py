"""NoCacheMiddleware"""

from datetime import datetime, UTC
from typing import Callable, Awaitable
from fastapi import Request

from starlette.middleware.base import BaseHTTPMiddleware
from starlette.responses import Response


class NoCacheMiddleware(BaseHTTPMiddleware):
    """Middleware to add no-cache headers to prevent browser caching."""

    async def dispatch(
        self, request: Request, call_next: Callable[[Request], Awaitable[Response]]
    ) -> Response:
        """Process request and add no-cache headers to prevent browser caching."""
        response = await call_next(request)

        # Add no-cache headers for all responses
        response.headers["Cache-Control"] = (
            "no-cache, no-store, must-revalidate, max-age=0"
        )
        response.headers["Pragma"] = "no-cache"
        response.headers["Expires"] = "0"

        # Additional headers for static files (JS, CSS, HTML)
        if any(
            request.url.path.endswith(ext) for ext in [".js", ".css", ".html", ".htm"]
        ):
            response.headers["Cache-Control"] = (
                "no-cache, no-store, must-revalidate, max-age=0, private"
            )
            response.headers["ETag"] = f'"{datetime.now(UTC).timestamp()}"'

        return response
