"""Hardware controller for shell sorter machine using ESPHome."""

import asyncio
import logging
from typing import Optional, Dict, Any
import aiohttp  # type: ignore[import-not-found]
from pydantic import BaseModel

logger = logging.getLogger(__name__)


class ESPHomeConfig(BaseModel):
    """Configuration for ESPHome device."""
    host: str = "shell-sorter-controller.local"
    port: int = 80
    username: str = "admin"
    password: str = "shellsorter"


class HardwareController:
    """Controls shell sorter hardware via ESPHome device."""

    def __init__(self, config: Optional[ESPHomeConfig] = None) -> None:
        self.config = config or ESPHomeConfig()
        self.base_url = f"http://{self.config.host}:{self.config.port}"
        self.auth = aiohttp.BasicAuth(self.config.username, self.config.password)
        
    async def _make_request(self, endpoint: str, method: str = "GET", data: Optional[Dict[str, Any]] = None) -> Optional[Dict[str, Any]]:
        """Make HTTP request to ESPHome device."""
        try:
            async with aiohttp.ClientSession(timeout=aiohttp.ClientTimeout(total=5)) as session:
                url = f"{self.base_url}{endpoint}"
                
                if method == "GET":
                    async with session.get(url, auth=self.auth) as response:
                        if response.status == 200:
                            return await response.json()  # type: ignore[no-any-return]
                        else:
                            logger.error("ESPHome request failed: %s %s", response.status, await response.text())
                            return None
                            
                elif method == "POST":
                    async with session.post(url, auth=self.auth, json=data) as response:
                        if response.status == 200:
                            return await response.json()  # type: ignore[no-any-return]
                        else:
                            logger.error("ESPHome request failed: %s %s", response.status, await response.text())
                            return None
                            
                return None
                            
        except asyncio.TimeoutError:
            logger.error("ESPHome request timed out: %s", endpoint)
            return None
        except Exception as e:
            logger.error("ESPHome request error: %s", e)
            return None

    async def get_sensor_states(self) -> Dict[str, bool]:
        """Get current state of all sensors."""
        try:
            # ESPHome provides sensor states via the /sensor endpoint
            result = await self._make_request("/sensor")
            if result:
                return {
                    "case_ready": result.get("case_ready_to_feed", {}).get("state", False),
                    "case_in_camera": result.get("case_in_camera_view", {}).get("state", False)
                }
            return {"case_ready": False, "case_in_camera": False}
        except Exception as e:
            logger.error("Error getting sensor states: %s", e)
            return {"case_ready": False, "case_in_camera": False}

    async def is_case_ready_to_feed(self) -> bool:
        """Check if a case is ready to be fed into the system."""
        try:
            result = await self._make_request("/binary_sensor/case_ready_to_feed")
            if result and "state" in result:
                return bool(result["state"] == "ON")
            return False
        except Exception as e:
            logger.error("Error checking case ready sensor: %s", e)
            return False

    async def is_case_in_camera_view(self) -> bool:
        """Check if a case is currently in the camera view."""
        try:
            result = await self._make_request("/binary_sensor/case_in_camera_view")
            if result and "state" in result:
                return bool(result["state"] == "ON")
            return False
        except Exception as e:
            logger.error("Error checking camera view sensor: %s", e)
            return False

    async def activate_vibration_motor(self, duration_seconds: float = 2.0) -> bool:
        """Activate vibration motor for specified duration."""
        try:
            # Turn on vibration motor
            result = await self._make_request("/switch/vibration_motor/turn_on", method="POST")
            if not result:
                return False
                
            logger.info("Vibration motor activated for %gs", duration_seconds)
            
            # Wait for specified duration
            await asyncio.sleep(duration_seconds)
            
            # Turn off vibration motor
            result = await self._make_request("/switch/vibration_motor/turn_off", method="POST")
            if result:
                logger.info("Vibration motor deactivated")
                return True
            else:
                logger.error("Failed to deactivate vibration motor")
                return False
                
        except Exception as e:
            logger.error("Error controlling vibration motor: %s", e)
            return False

    async def set_case_feeder_servo(self, position: str) -> bool:
        """Set case feeder servo to specific position."""
        try:
            if position == "home":
                endpoint = "/switch/case_feeder_servo_home_position/turn_on"
            elif position == "feed":
                endpoint = "/switch/case_feeder_servo_feed_position/turn_on"
            else:
                logger.error("Invalid feeder servo position: %s", position)
                return False
                
            result = await self._make_request(endpoint, method="POST")
            if result:
                logger.info("Case feeder servo moved to %s position", position)
                return True
            return False
            
        except Exception as e:
            logger.error("Error controlling case feeder servo: %s", e)
            return False

    async def set_case_position_servo(self, position: str) -> bool:
        """Set case positioning servo to specific position."""
        try:
            if position == "camera":
                endpoint = "/switch/case_position_servo_camera_position/turn_on"
            elif position == "drop":
                endpoint = "/switch/case_position_servo_drop_position/turn_on"
            else:
                logger.error("Invalid position servo position: %s", position)
                return False
                
            result = await self._make_request(endpoint, method="POST")
            if result:
                logger.info("Case position servo moved to %s position", position)
                return True
            return False
            
        except Exception as e:
            logger.error("Error controlling case position servo: %s", e)
            return False

    async def run_next_case_sequence(self) -> bool:
        """Run the complete sequence to advance to next case."""
        try:
            logger.info("Starting next case sequence")
            
            # Check if a case is ready to feed
            if not await self.is_case_ready_to_feed():
                logger.warning("No case ready to feed")
                return False
            
            # Activate vibration motor to advance case
            if not await self.activate_vibration_motor(duration_seconds=1.5):
                logger.error("Failed to activate vibration motor")
                return False
            
            # Wait a moment for case to settle
            await asyncio.sleep(0.5)
            
            # Move feeder servo to feed position
            if not await self.set_case_feeder_servo("feed"):
                logger.error("Failed to move feeder servo")
                return False
            
            # Wait for servo movement
            await asyncio.sleep(1.0)
            
            # Return feeder servo to home
            if not await self.set_case_feeder_servo("home"):
                logger.error("Failed to return feeder servo to home")
                return False
            
            # Move case to camera position
            if not await self.set_case_position_servo("camera"):
                logger.error("Failed to move case to camera position")
                return False
            
            logger.info("Next case sequence completed successfully")
            return True
            
        except Exception as e:
            logger.error("Error in next case sequence: %s", e)
            return False

    async def test_connection(self) -> bool:
        """Test connection to ESPHome device."""
        try:
            result = await self._make_request("/")
            return result is not None
        except Exception as e:
            logger.error("ESPHome connection test failed: %s", e)
            return False

    async def get_device_info(self) -> Optional[Dict[str, Any]]:
        """Get device information from ESPHome."""
        try:
            return await self._make_request("/status")
        except Exception as e:
            logger.error("Error getting device info: %s", e)
            return None