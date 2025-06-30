"""ESPHome device connectivity monitoring."""

import asyncio
import logging
from threading import Lock
from typing import Any, Optional

try:
    import aiohttp  # type: ignore[import-not-found]
except ImportError:
    aiohttp = None

logger = logging.getLogger(__name__)


class ESPHomeMonitor:
    """Monitors connectivity to an ESPHome device."""
    
    def __init__(self, hostname: str, check_interval: int = 30):
        """Initialize the ESPHome monitor.
        
        Args:
            hostname: ESPHome device hostname
            check_interval: Seconds between connectivity checks
        """
        self.hostname = hostname
        self.check_interval = check_interval
        self._is_online = False
        self._last_check_time: Optional[float] = None
        self._monitoring_task: Optional[asyncio.Task[None]] = None
        self._lock = Lock()
        self._session: Optional[aiohttp.ClientSession] = None
    
    @property
    def is_online(self) -> bool:
        """Check if the ESPHome device is currently online."""
        with self._lock:
            return self._is_online
    
    @property
    def last_check_time(self) -> Optional[float]:
        """Get the timestamp of the last connectivity check."""
        with self._lock:
            return self._last_check_time
    
    def update_hostname(self, new_hostname: str) -> None:
        """Update the hostname and restart monitoring if needed."""
        if self.hostname != new_hostname:
            self.hostname = new_hostname
            logger.info("Updated ESPHome hostname to: %s", new_hostname)
            # Restart monitoring with new hostname if currently running
            if self._monitoring_task and not self._monitoring_task.done():
                self._monitoring_task.cancel()
                asyncio.create_task(self._start_monitoring())
    
    async def _check_connectivity(self) -> bool:
        """Check if the ESPHome device is reachable."""
        if not aiohttp:
            logger.error("aiohttp not available, cannot check ESPHome connectivity")
            return False
            
        try:
            # Use ESPHome web server endpoint for basic connectivity check
            url = f"http://{self.hostname}/"
            
            if not self._session:
                timeout = aiohttp.ClientTimeout(total=5.0)
                self._session = aiohttp.ClientSession(timeout=timeout)
            
            async with self._session.get(url) as response:
                # ESPHome web server should return 200 or 401 (auth required)
                online = response.status in [200, 401]
                
                with self._lock:
                    self._is_online = online
                    self._last_check_time = asyncio.get_event_loop().time()
                
                if online:
                    logger.debug("ESPHome device %s is online", self.hostname)
                else:
                    logger.warning("ESPHome device %s returned status %d", 
                                 self.hostname, response.status)
                
                return online
                
        except asyncio.TimeoutError:
            logger.debug("Timeout connecting to ESPHome device %s", self.hostname)
        except aiohttp.ClientError as e:
            logger.debug("Connection error to ESPHome device %s: %s", self.hostname, e)
        except Exception as e:
            logger.error("Unexpected error checking ESPHome connectivity: %s", e)
        
        with self._lock:
            self._is_online = False
            self._last_check_time = asyncio.get_event_loop().time()
        
        return False
    
    async def _start_monitoring(self) -> None:
        """Start the monitoring loop."""
        logger.info("Starting ESPHome connectivity monitoring for %s", self.hostname)
        
        try:
            while True:
                await self._check_connectivity()
                await asyncio.sleep(self.check_interval)
                
        except asyncio.CancelledError:
            logger.info("ESPHome monitoring cancelled")
            raise
        except Exception as e:
            logger.error("Error in ESPHome monitoring loop: %s", e)
        finally:
            if self._session:
                await self._session.close()
                self._session = None
    
    def start_monitoring(self) -> None:
        """Start monitoring in the background."""
        if self._monitoring_task and not self._monitoring_task.done():
            logger.warning("ESPHome monitoring already running")
            return
        
        loop = asyncio.get_event_loop()
        self._monitoring_task = loop.create_task(self._start_monitoring())
        logger.info("ESPHome monitoring task started")
    
    def stop_monitoring(self) -> None:
        """Stop background monitoring."""
        if self._monitoring_task and not self._monitoring_task.done():
            self._monitoring_task.cancel()
            logger.info("ESPHome monitoring stopped")
    
    async def check_once(self) -> bool:
        """Perform a single connectivity check."""
        return await self._check_connectivity()
    
    def get_status(self) -> dict[str, Any]:
        """Get current status information."""
        with self._lock:
            return {
                "online": self._is_online,
                "hostname": self.hostname,
                "last_check_time": self._last_check_time,
                "monitoring_active": (
                    self._monitoring_task is not None 
                    and not self._monitoring_task.done()
                )
            }