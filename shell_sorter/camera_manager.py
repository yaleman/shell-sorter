"""Camera management for shell sorting machine."""

import asyncio
import concurrent.futures
from dataclasses import dataclass
from io import BytesIO
import json
import logging
import platform
import subprocess
import threading
import time
from typing import Dict, List, Optional, Tuple, Literal, TYPE_CHECKING, Any

import aiohttp  # type: ignore[import-not-found]

import cv2  # type: ignore[import-not-found]
from PIL import Image
from PIL.ExifTags import TAGS
import piexif  # type: ignore[import-not-found]

if TYPE_CHECKING:
    from .config import Settings, UserConfig, CameraConfig
else:
    # Import these at runtime to avoid circular imports
    from .config import UserConfig, CameraConfig

logger = logging.getLogger(__name__)


@dataclass
class CameraInfo:
    """Information about a connected camera."""

    index: int
    name: str
    resolution: Tuple[int, int]
    is_active: bool = False
    is_selected: bool = False
    view_type: Optional[Literal["side", "tail"]] = None
    region_x: Optional[int] = None
    region_y: Optional[int] = None
    region_width: Optional[int] = None
    region_height: Optional[int] = None
    # Network camera support
    is_network_camera: bool = False
    stream_url: Optional[str] = None
    hostname: Optional[str] = None
    # Hardware identification for stable mapping
    device_path: Optional[str] = None  # e.g., /dev/video0
    vendor_id: Optional[str] = None
    product_id: Optional[str] = None
    serial_number: Optional[str] = None
    hardware_id: Optional[str] = None  # Composite stable identifier


class CameraManager:
    """Manages camera detection, selection, and streaming."""

    def __init__(self, settings: Optional["Settings"] = None) -> None:
        self.cameras: Dict[int, CameraInfo] = {}
        self.active_captures: Dict[int, cv2.VideoCapture] = {}
        self.streaming_threads: Dict[int, threading.Thread] = {}
        self.stop_streaming: Dict[int, threading.Event] = {}
        self.latest_frames: Dict[int, Optional[bytes]] = {}
        self._lock = threading.Lock()
        self.settings = settings
        self.auto_start_cameras: bool = False

    def _get_camera_device_name(self, camera_index: int) -> str:
        """Get the actual device name/model for a camera."""
        try:
            system = platform.system()

            if system == "Linux":
                # Try to get device name from v4l2
                try:
                    result = subprocess.run(
                        ["v4l2-ctl", "--device", f"/dev/video{camera_index}", "--info"],
                        capture_output=True,
                        text=True,
                        timeout=5,
                        check=False,
                    )
                    if result.returncode == 0:
                        lines = result.stdout.split("\n")
                        for line in lines:
                            if "Card type" in line or "Device name" in line:
                                # Extract device name after the colon
                                name = line.split(":", 1)[-1].strip()
                                if name:
                                    return name
                except (
                    subprocess.TimeoutExpired,
                    subprocess.CalledProcessError,
                    FileNotFoundError,
                ):
                    pass

                # Fallback: try to read from udev
                try:
                    result = subprocess.run(
                        [
                            "udevadm",
                            "info",
                            "--name",
                            f"/dev/video{camera_index}",
                            "--query=property",
                        ],
                        capture_output=True,
                        text=True,
                        timeout=5,
                        check=False,
                    )
                    if result.returncode == 0:
                        lines = result.stdout.split("\n")
                        for line in lines:
                            if line.startswith("ID_MODEL="):
                                model = line.split("=", 1)[-1].strip().replace("_", " ")
                                if model:
                                    return model
                            elif line.startswith("ID_V4L_PRODUCT="):
                                product = line.split("=", 1)[-1].strip()
                                if product:
                                    return product
                except (
                    subprocess.TimeoutExpired,
                    subprocess.CalledProcessError,
                    FileNotFoundError,
                ):
                    pass

            elif system == "Darwin":  # macOS
                # Try to get device name from system_profiler
                try:
                    result = subprocess.run(
                        ["system_profiler", "SPCameraDataType", "-json"],
                        capture_output=True,
                        text=True,
                        timeout=10,
                        check=False,
                    )
                    if result.returncode == 0:
                        data = json.loads(result.stdout)
                        cameras = data.get("SPCameraDataType", [])
                        if camera_index < len(cameras):
                            camera_info = cameras[camera_index]
                            name = camera_info.get("_name", "")
                            if name:
                                return str(name)
                except (
                    subprocess.TimeoutExpired,
                    subprocess.CalledProcessError,
                    FileNotFoundError,
                    json.JSONDecodeError,
                ):
                    pass

            elif system == "Windows":
                # Try to get device name from DirectShow
                try:
                    powershell_command = " | ".join(
                        [
                            "Get-CimInstance -ClassName Win32_PnPEntity",
                            "Where-Object {$_.Name -like '*camera*' -or $_.Name -like '*webcam*'} ",
                            " Select-Object Name",
                        ]
                    )
                    result = subprocess.run(
                        [
                            "powershell",
                            "-Command",
                            powershell_command,
                        ],
                        capture_output=True,
                        text=True,
                        timeout=10,
                        check=False,
                    )
                    if result.returncode == 0:
                        lines = result.stdout.split("\n")
                        camera_names = [
                            line.strip()
                            for line in lines
                            if line.strip()
                            and "Name" not in line
                            and "----" not in line
                        ]
                        if camera_index < len(camera_names):
                            return camera_names[camera_index]
                except (
                    subprocess.TimeoutExpired,
                    subprocess.CalledProcessError,
                    FileNotFoundError,
                ):
                    pass

            # Try OpenCV backend info if available
            cap = cv2.VideoCapture(camera_index)
            if cap.isOpened():
                # Try to get backend info
                backend = cap.getBackendName()
                if backend:
                    cap.release()
                    return f"Camera {camera_index} ({backend})"
                cap.release()

        except Exception as e:
            logger.debug(
                "Error getting camera device name for camera %d: %s", camera_index, e
            )

        # Final fallback
        return f"Camera {camera_index}"

    def _get_camera_hardware_info(self, camera_index: int) -> Dict[str, Optional[str]]:
        """Get hardware identification information for a camera."""
        hardware_info: Dict[str, Optional[str]] = {
            "device_path": None,
            "vendor_id": None,
            "product_id": None,
            "serial_number": None,
        }

        try:
            system = platform.system()
            device_path = f"/dev/video{camera_index}"
            hardware_info["device_path"] = device_path

            if system == "Linux":
                # Try to get hardware info from udev
                try:
                    result = subprocess.run(
                        [
                            "udevadm",
                            "info",
                            "--name",
                            device_path,
                            "--query=property",
                        ],
                        capture_output=True,
                        text=True,
                        timeout=5,
                        check=False,
                    )
                    if result.returncode == 0:
                        lines = result.stdout.split("\n")
                        for line in lines:
                            if line.startswith("ID_VENDOR_ID="):
                                hardware_info["vendor_id"] = line.split("=", 1)[-1].strip()
                            elif line.startswith("ID_MODEL_ID="):
                                hardware_info["product_id"] = line.split("=", 1)[-1].strip()
                            elif line.startswith("ID_SERIAL_SHORT="):
                                hardware_info["serial_number"] = line.split("=", 1)[-1].strip()
                except (
                    subprocess.TimeoutExpired,
                    subprocess.CalledProcessError,
                    FileNotFoundError,
                ):
                    pass

                # Alternative: try lsusb to get more USB device info
                if not hardware_info["vendor_id"]:
                    try:
                        result = subprocess.run(
                            ["lsusb", "-v"],
                            capture_output=True,
                            text=True,
                            timeout=10,
                            check=False,
                        )
                        if result.returncode == 0:
                            # This is complex parsing - for now just try basic approach
                            # TODO: Parse lsusb output to match video device to USB device
                            pass
                    except (
                        subprocess.TimeoutExpired,
                        subprocess.CalledProcessError,
                        FileNotFoundError,
                    ):
                        pass

            elif system == "Darwin":  # macOS
                # Try to get device info from system_profiler
                try:
                    result = subprocess.run(
                        ["system_profiler", "SPUSBDataType", "-json"],
                        capture_output=True,
                        text=True,
                        timeout=15,
                        check=False,
                    )
                    if result.returncode == 0:
                        # TODO: Match camera to USB device by parsing the tree
                        # This is complex because we need to correlate video device with USB device
                        # data = json.loads(result.stdout)
                        # usb_devices = data.get("SPUSBDataType", [])
                        pass
                except (
                    subprocess.TimeoutExpired,
                    subprocess.CalledProcessError,
                    FileNotFoundError,
                    json.JSONDecodeError,
                ):
                    pass

            elif system == "Windows":
                # Try to get device info from Windows
                try:
                    # Use wmic to get USB device info
                    result = subprocess.run(
                        [
                            "wmic",
                            "path",
                            "Win32_USBHub",
                            "get",
                            "DeviceID,Description",
                            "/format:csv",
                        ],
                        capture_output=True,
                        text=True,
                        timeout=10,
                        check=False,
                    )
                    if result.returncode == 0:
                        # TODO: Parse Windows device info
                        pass
                except (
                    subprocess.TimeoutExpired,
                    subprocess.CalledProcessError,
                    FileNotFoundError,
                ):
                    pass

        except Exception as e:
            logger.debug(
                "Error getting camera hardware info for camera %d: %s", camera_index, e
            )

        return hardware_info

    def _generate_hardware_id(self, camera_info: "CameraInfo") -> str:
        """Generate a stable hardware identifier for a camera."""
        if camera_info.is_network_camera:
            # For network cameras, use hostname as stable identifier
            return f"network:{camera_info.hostname or 'unknown'}"

        # For USB cameras, try to create a stable identifier from hardware info
        parts = []
        
        if camera_info.vendor_id and camera_info.product_id:
            parts.append(f"usb:{camera_info.vendor_id}:{camera_info.product_id}")
            
        if camera_info.serial_number:
            parts.append(f"serial:{camera_info.serial_number}")
        
        if camera_info.device_path:
            parts.append(f"path:{camera_info.device_path}")
            
        # Always include the camera name as a fallback component
        parts.append(f"name:{camera_info.name}")
        
        # If we have vendor/product ID, that's our primary identifier
        if camera_info.vendor_id and camera_info.product_id:
            if camera_info.serial_number:
                primary_id = f"{camera_info.vendor_id}:{camera_info.product_id}:{camera_info.serial_number}"
            else:
                primary_id = f"{camera_info.vendor_id}:{camera_info.product_id}:{camera_info.name}"
        else:
            # Fallback to device path + name
            primary_id = f"{camera_info.device_path or 'unknown'}:{camera_info.name}"
            
        return primary_id

    def detect_esphome_cameras(self) -> List[CameraInfo]:
        """Detect ESPHome cameras on the network."""
        # Run async detection in event loop
        try:
            loop = asyncio.get_event_loop()
            if loop.is_running():
                # If we're already in an event loop, create a task
                with concurrent.futures.ThreadPoolExecutor() as executor:
                    future = executor.submit(
                        asyncio.run, self._detect_esphome_cameras_async()
                    )
                    return future.result(timeout=10)
            else:
                return asyncio.run(self._detect_esphome_cameras_async())
        except Exception as e:
            logger.warning("Failed to detect ESPHome cameras: %s", e)
            return []

    async def _detect_esphome_cameras_async(self) -> List[CameraInfo]:
        """Async method to detect ESPHome cameras on the network."""
        esphome_cameras = []

        # Get ESPHome camera hostnames from configuration
        esphome_hosts = []
        if self.settings:
            # Try to get from user config first
            try:
                user_config_data = self.settings.load_user_config()
                user_config = UserConfig(**user_config_data)
                esphome_hosts = user_config.network_camera_hostnames.copy()
            except Exception as e:
                logger.debug(
                    "Failed to load network camera hostnames from user config: %s", e
                )
                # Fall back to application settings
                esphome_hosts = self.settings.network_camera_hostnames.copy()
        else:
            # Default fallback
            esphome_hosts = ["esp32cam1.local"]

        # Also check main controller in case it has camera
        if self.settings and self.settings.esphome_hostname not in esphome_hosts:
            esphome_hosts.append(self.settings.esphome_hostname)

        timeout = aiohttp.ClientTimeout(total=5)
        async with aiohttp.ClientSession(timeout=timeout) as session:
            for i, hostname in enumerate(esphome_hosts):
                try:
                    # Check if the device is reachable and has a camera
                    camera_url = f"http://{hostname}/camera"
                    async with session.get(camera_url) as response:
                        if response.status == 200 and "image" in response.headers.get(
                            "content-type", ""
                        ):
                            # This is a valid camera stream
                            # Try to get image dimensions from the first frame
                            image_data = await response.read()

                            # Default resolution for ESPHome cameras
                            width, height = 800, 600

                            # Try to parse image headers to get actual dimensions
                            try:
                                img = Image.open(BytesIO(image_data))
                                width, height = img.size
                            except Exception:
                                # Keep default resolution
                                pass

                            camera_info = CameraInfo(
                                index=1000
                                + i,  # Use high indices to avoid conflicts with USB cameras
                                name=f"ESPHome Camera ({hostname})",
                                resolution=(width, height),
                                is_active=False,
                                is_selected=True,  # Auto-select network cameras
                                is_network_camera=True,
                                stream_url=camera_url,
                                hostname=hostname,
                            )
                            
                            # Generate stable hardware ID for network camera
                            camera_info.hardware_id = self._generate_hardware_id(camera_info)

                            # Load camera configuration from user config if available
                            if self.settings:
                                self._load_camera_config(camera_info)

                            esphome_cameras.append(camera_info)
                            logger.info(
                                "Detected ESPHome camera: %s at %s",
                                camera_info.name,
                                hostname,
                            )

                except (aiohttp.ClientError, asyncio.TimeoutError) as e:
                    logger.debug("ESPHome camera not found at %s: %s", hostname, e)
                    continue
                except Exception as e:
                    logger.debug("Error checking ESPHome camera at %s: %s", hostname, e)
                    continue

        return esphome_cameras

    def detect_cameras(self) -> List[CameraInfo]:
        """Detect all available cameras (USB and network)."""
        cameras = []

        # Detect USB cameras first
        # Try camera indices 0-9
        for i in range(10):
            cap = cv2.VideoCapture(i)
            if cap.isOpened():
                # Get camera resolution
                width = int(cap.get(cv2.CAP_PROP_FRAME_WIDTH))
                height = int(cap.get(cv2.CAP_PROP_FRAME_HEIGHT))

                # Get actual device name/model
                camera_name = self._get_camera_device_name(i)
                
                # Get hardware identification information
                hardware_info = self._get_camera_hardware_info(i)

                camera_info = CameraInfo(
                    index=i,
                    name=camera_name,
                    resolution=(width, height),
                    is_selected=True,
                    device_path=hardware_info["device_path"],
                    vendor_id=hardware_info["vendor_id"],
                    product_id=hardware_info["product_id"],
                    serial_number=hardware_info["serial_number"],
                )
                
                # Generate stable hardware ID
                camera_info.hardware_id = self._generate_hardware_id(camera_info)

                # Load camera configuration from user config if available
                if self.settings:
                    self._load_camera_config(camera_info)

                cameras.append(camera_info)
                self.cameras[i] = camera_info

                logger.info(
                    "Detected camera %d with resolution %dx%d", i, width, height
                )

            cap.release()

        # Detect ESPHome network cameras
        esphome_cameras = self.detect_esphome_cameras()
        cameras.extend(esphome_cameras)

        logger.info(
            "Detected %d cameras (%d USB, %d ESPHome)",
            len(cameras),
            len(cameras) - len(esphome_cameras),
            len(esphome_cameras),
        )
        
        # Perform camera remapping to maintain stable assignments
        self._remap_cameras_by_hardware_id()
        
        return cameras

    def _remap_cameras_by_hardware_id(self) -> None:
        """Remap camera indices to maintain stable hardware ID assignments."""
        if not self.settings:
            return

        try:
            # Load existing user config to see what hardware IDs we know about
            user_config_data = self.settings.load_user_config()
            user_config = UserConfig(**user_config_data)
            
            # Get all known hardware IDs with configurations
            known_hardware_ids = set(user_config.camera_configs.keys())
            
            # Filter to only include hardware IDs (not legacy name-based configs)
            known_hardware_ids = {
                hid for hid in known_hardware_ids 
                if ':' in hid and (hid.startswith('usb:') or hid.startswith('network:'))
            }
            
            if not known_hardware_ids:
                logger.debug("No known hardware IDs found, skipping camera remapping")
                return
            
            # Create mapping from hardware ID to new camera info
            hardware_id_to_camera = {}
            for camera in self.cameras.values():
                if camera.hardware_id:
                    hardware_id_to_camera[camera.hardware_id] = camera
            
            # Check for hardware ID matches
            matched_hardware_ids = known_hardware_ids.intersection(hardware_id_to_camera.keys())
            
            if matched_hardware_ids:
                logger.info("Found %d cameras with known hardware IDs: %s", 
                          len(matched_hardware_ids), list(matched_hardware_ids))
                
                # For USB cameras, we might need to create a stable index mapping
                # For now, we just log the successful matches
                for hardware_id in matched_hardware_ids:
                    camera = hardware_id_to_camera[hardware_id]
                    logger.info("Camera at index %d matched known hardware ID: %s (%s)",
                              camera.index, hardware_id, camera.name)
            else:
                logger.info("No cameras matched known hardware IDs")
                
            # Report any previously known cameras that are now missing
            missing_hardware_ids = known_hardware_ids - set(hardware_id_to_camera.keys())
            if missing_hardware_ids:
                logger.warning("Previously configured cameras not found: %s", 
                             list(missing_hardware_ids))
                
        except Exception as e:
            logger.warning("Error during camera remapping: %s", e)

    def get_cameras(self) -> List[CameraInfo]:
        """Get list of detected cameras."""
        return list(self.cameras.values())

    def select_cameras(self, camera_indices: List[int]) -> bool:
        """Select which cameras to use for sorting."""
        try:
            # Deselect all cameras first
            for camera in self.cameras.values():
                camera.is_selected = False

            # Select specified cameras
            for index in camera_indices:
                if index in self.cameras:
                    self.cameras[index].is_selected = True
                    logger.info("Selected camera %d", index)
                else:
                    logger.warning("Camera %d not found", index)
                    return False

            return True
        except Exception as e:
            logger.error("Error selecting cameras: %s", e)
            return False

    def get_selected_cameras(self) -> List[CameraInfo]:
        """Get list of selected cameras."""
        return [cam for cam in self.cameras.values() if cam.is_selected]

    def set_camera_view_type(
        self, camera_index: int, view_type: Optional[Literal["side", "tail"]]
    ) -> bool:
        """Set the view type for a camera."""
        if camera_index not in self.cameras:
            logger.warning("Camera %d not found", camera_index)
            return False

        camera = self.cameras[camera_index]
        camera.view_type = view_type
        logger.info("Set camera %d view type to %s", camera_index, view_type)

        # Save to user config
        if self.settings:
            self._save_camera_config(camera)

        return True

    def set_camera_region(
        self, camera_index: int, x: int, y: int, width: int, height: int
    ) -> bool:
        """Set the region of interest for a camera."""
        if camera_index not in self.cameras:
            logger.warning("Camera %d not found", camera_index)
            return False

        camera = self.cameras[camera_index]
        camera.region_x = x
        camera.region_y = y
        camera.region_width = width
        camera.region_height = height

        logger.info(
            "Set camera %d region to (%d,%d) %dx%d", camera_index, x, y, width, height
        )

        # Save to user config
        if self.settings:
            self._save_camera_config(camera)

        return True

    def clear_camera_region(self, camera_index: int) -> bool:
        """Clear the region of interest for a camera."""
        if camera_index not in self.cameras:
            logger.warning("Camera %d not found", camera_index)
            return False

        camera = self.cameras[camera_index]
        camera.region_x = None
        camera.region_y = None
        camera.region_width = None
        camera.region_height = None

        logger.info("Cleared camera %d region", camera_index)

        # Save to user config
        if self.settings:
            self._save_camera_config(camera)

        return True

    def trigger_autofocus(self, camera_index: int) -> bool:
        """Trigger autofocus for a camera, focusing on the center of the region if set."""
        if camera_index not in self.cameras:
            logger.warning("Camera %d not found", camera_index)
            return False

        camera_info = self.cameras[camera_index]

        # Network cameras don't support autofocus control
        if camera_info.is_network_camera:
            logger.info("Autofocus not supported for network camera %d", camera_index)
            return False

        try:
            # Use existing capture if active, otherwise open temporarily
            if camera_index in self.active_captures:
                cap = self.active_captures[camera_index]
                should_close = False
            else:
                cap = self._open_camera_with_timeout(camera_index, timeout=3.0)
                if cap is None:
                    logger.error("Failed to open camera %d for autofocus", camera_index)
                    return False
                should_close = True

            # Disable autofocus briefly then re-enable to trigger
            cap.set(cv2.CAP_PROP_AUTOFOCUS, 0)

            time.sleep(0.1)
            cap.set(cv2.CAP_PROP_AUTOFOCUS, 1)

            # If we have a region, try to set focus point (not all cameras support this)
            if (
                camera_info.region_x is not None
                and camera_info.region_y is not None
                and camera_info.region_width is not None
                and camera_info.region_height is not None
            ):
                # Calculate center of region
                center_x = camera_info.region_x + camera_info.region_width // 2
                center_y = camera_info.region_y + camera_info.region_height // 2

                # Try to set focus point (may not be supported by all cameras)
                try:
                    # Some cameras support CAP_PROP_FOCUS_POINT_X/Y
                    cap.set(cv2.CAP_PROP_FOCUS_POINT_X, center_x)
                    cap.set(cv2.CAP_PROP_FOCUS_POINT_Y, center_y)
                except Exception as error:
                    # Focus point setting not supported, just use general autofocus
                    logger.warning(
                        "Focus point setting not supported for camera %d - %s",
                        camera_index,
                        error,
                    )

                logger.info(
                    "Triggered autofocus for camera %d at region center (%d, %d)",
                    camera_index,
                    center_x,
                    center_y,
                )
            else:
                logger.info(
                    "Triggered autofocus for camera %d (no region set)", camera_index
                )

            # Give camera time to focus
            time.sleep(1.0)

            if should_close:
                cap.release()

            return True

        except Exception as e:
            logger.error(
                "Error triggering autofocus for camera %d: %s", camera_index, e
            )
            return False

    def _open_camera_with_timeout(
        self, camera_index: int, timeout: float = 3.0
    ) -> Optional[cv2.VideoCapture]:
        """Open camera with timeout to prevent hanging."""

        def open_camera() -> Optional[cv2.VideoCapture]:
            cap = cv2.VideoCapture(camera_index)
            if cap.isOpened():
                return cap
            cap.release()
            return None

        with concurrent.futures.ThreadPoolExecutor() as executor:
            future = executor.submit(open_camera)
            try:
                return future.result(timeout=timeout)
            except concurrent.futures.TimeoutError:
                logger.warning(
                    "Camera %d open timed out after %gs", camera_index, timeout
                )
                return None

    def start_camera_stream(self, camera_index: int) -> bool:
        """Start streaming from a specific camera."""
        try:
            if camera_index not in self.cameras:
                logger.warning("Camera %d not found in detected cameras", camera_index)
                return False

            camera_info = self.cameras[camera_index]

            # Stop existing stream if running
            self.stop_camera_stream(camera_index)

            if camera_info.is_network_camera:
                # Handle network camera (ESPHome)
                logger.info(
                    "Starting network camera stream %d (%s)...",
                    camera_index,
                    camera_info.hostname,
                )

                # For network cameras, we don't use OpenCV capture, just prepare for streaming
                self.stop_streaming[camera_index] = threading.Event()
                self.latest_frames[camera_index] = None

                # Start network streaming thread
                thread = threading.Thread(
                    target=self._stream_network_camera,
                    args=(camera_index,),
                    daemon=True,
                )
                self.streaming_threads[camera_index] = thread
                thread.start()

            else:
                # Handle USB camera
                logger.info("Opening USB camera %d...", camera_index)
                cap = self._open_camera_with_timeout(camera_index, timeout=3.0)
                if cap is None:
                    logger.error(
                        "Failed to open camera %d within timeout", camera_index
                    )
                    return False

                # Set camera properties for better performance and autofocus
                cap.set(cv2.CAP_PROP_FRAME_WIDTH, 640)
                cap.set(cv2.CAP_PROP_FRAME_HEIGHT, 480)
                cap.set(cv2.CAP_PROP_FPS, 30)

                # Enable autofocus if supported
                cap.set(cv2.CAP_PROP_AUTOFOCUS, 1)

                # Allow camera to stabilize

                time.sleep(2.0)

                # Test camera by reading a few frames
                for _ in range(5):
                    ret, _ = cap.read()
                    if ret:
                        break
                    time.sleep(0.1)
                else:
                    logger.error("Camera %d failed initial frame test", camera_index)
                    cap.release()
                    return False

                self.active_captures[camera_index] = cap
                self.stop_streaming[camera_index] = threading.Event()
                self.latest_frames[camera_index] = None

                # Start USB camera streaming thread
                thread = threading.Thread(
                    target=self._stream_camera, args=(camera_index,), daemon=True
                )
                self.streaming_threads[camera_index] = thread
                thread.start()

            self.cameras[camera_index].is_active = True
            logger.info("Started streaming from camera %d", camera_index)
            return True

        except Exception as e:
            logger.error("Error starting camera %d: %s", camera_index, e)
            return False

    def stop_camera_stream(self, camera_index: int) -> None:
        """Stop streaming from a specific camera."""
        try:
            with self._lock:
                # Signal stop
                if camera_index in self.stop_streaming:
                    self.stop_streaming[camera_index].set()

                # Wait for thread to finish
                if camera_index in self.streaming_threads:
                    thread = self.streaming_threads[camera_index]
                    if thread.is_alive():
                        thread.join(timeout=2.0)
                    del self.streaming_threads[camera_index]

                # Release capture
                if camera_index in self.active_captures:
                    self.active_captures[camera_index].release()
                    del self.active_captures[camera_index]

                # Clean up
                if camera_index in self.stop_streaming:
                    del self.stop_streaming[camera_index]
                if camera_index in self.latest_frames:
                    del self.latest_frames[camera_index]

                if camera_index in self.cameras:
                    self.cameras[camera_index].is_active = False

                logger.info("Stopped streaming from camera %d", camera_index)

        except Exception as e:
            logger.error("Error stopping camera %d: %s", camera_index, e)

    def _stream_camera(self, camera_index: int) -> None:
        """Internal method to stream frames from a camera."""
        cap = self.active_captures[camera_index]
        stop_event = self.stop_streaming[camera_index]
        consecutive_failures = 0
        max_failures = 10

        logger.info("Started streaming thread for camera %d", camera_index)

        while not stop_event.is_set():
            try:
                ret, frame = cap.read()
                if not ret:
                    consecutive_failures += 1
                    logger.warning(
                        "Failed to read frame from camera %d (failure %d/%d)",
                        camera_index,
                        consecutive_failures,
                        max_failures,
                    )

                    if consecutive_failures >= max_failures:
                        logger.error(
                            "Camera %d reached maximum consecutive failures, stopping stream",
                            camera_index,
                        )
                        break

                    if stop_event.wait(
                        0.5
                    ):  # Longer wait on failure for camera recovery
                        break
                    continue
                else:
                    # Reset failure counter on successful read
                    consecutive_failures = 0

                # Encode frame as JPEG
                _, buffer = cv2.imencode(".jpg", frame, [cv2.IMWRITE_JPEG_QUALITY, 85])
                self.latest_frames[camera_index] = buffer.tobytes()

                # Small delay to limit CPU usage, but check for stop signal
                if stop_event.wait(1 / 30):  # ~30 FPS but responsive to stop
                    break

            except Exception as e:
                logger.error("Error in camera %d stream: %s", camera_index, e)
                break

        logger.info("Stopped streaming thread for camera %d", camera_index)

    def _stream_network_camera(self, camera_index: int) -> None:
        """Stream frames from a network camera (ESPHome) in a separate thread."""
        camera_info = self.cameras[camera_index]
        stop_event = self.stop_streaming[camera_index]

        logger.info(
            "Started network streaming thread for camera %d (%s)",
            camera_index,
            camera_info.hostname,
        )

        # Run async streaming in this thread
        try:
            asyncio.run(
                self._async_stream_network_camera(camera_index, camera_info, stop_event)
            )
        except Exception as e:
            logger.error("Error in network camera %d streaming: %s", camera_index, e)

        logger.info("Stopped network streaming thread for camera %d", camera_index)

    async def _async_stream_network_camera(
        self, camera_index: int, camera_info: CameraInfo, stop_event: threading.Event
    ) -> None:
        """Async method to stream frames from a network camera."""
        timeout = aiohttp.ClientTimeout(total=10)
        async with aiohttp.ClientSession(timeout=timeout) as session:
            while not stop_event.is_set():
                try:
                    # Fetch frame from ESPHome camera
                    async with session.get(camera_info.stream_url) as response:
                        if response.status == 200:
                            frame_data = await response.read()

                            # Store latest frame
                            self.latest_frames[camera_index] = frame_data
                        else:
                            logger.warning(
                                "Failed to fetch frame from network camera %d: HTTP %d",
                                camera_index,
                                response.status,
                            )

                    # Delay for network cameras (slower refresh rate) with stop check
                    await asyncio.sleep(0.2)  # ~5fps for network cameras
                    if stop_event.is_set():
                        break

                except (aiohttp.ClientError, asyncio.TimeoutError) as e:
                    logger.warning("Network error for camera %d: %s", camera_index, e)
                    await asyncio.sleep(2.0)  # Wait longer on network errors
                    if stop_event.is_set():
                        break
                except Exception as e:
                    logger.error(
                        "Error in async network camera %d streaming: %s",
                        camera_index,
                        e,
                    )
                    break

    async def _capture_network_image_async(
        self, camera_info: CameraInfo
    ) -> Optional[bytes]:
        """Async method to capture an image from a network camera."""
        timeout = aiohttp.ClientTimeout(total=15)
        try:
            async with aiohttp.ClientSession(timeout=timeout) as session:
                async with session.get(camera_info.stream_url) as response:
                    if response.status == 200:
                        image_bytes: bytes = await response.read()
                        return image_bytes
                    else:
                        logger.error(
                            "Failed to capture from network camera: HTTP %d",
                            response.status,
                        )
                        return None
        except (aiohttp.ClientError, asyncio.TimeoutError) as e:
            logger.error("Network error capturing image: %s", e)
            return None
        except Exception as e:
            logger.error("Error capturing network image: %s", e)
            return None

    def get_latest_frame(self, camera_index: int) -> Optional[bytes]:
        """Get the latest frame from a camera as JPEG bytes."""
        return self.latest_frames.get(camera_index)

    def start_selected_cameras(self) -> List[int]:
        """Start streaming from all selected cameras."""
        selected_cameras = self.get_selected_cameras()
        if not selected_cameras:
            logger.warning("No cameras selected for streaming")
            return []

        logger.info(
            "Attempting to start %d selected cameras: %s",
            len(selected_cameras),
            [c.index for c in selected_cameras],
        )

        started = []
        failed = []

        for camera in selected_cameras:
            logger.info("Starting camera %d...", camera.index)
            if self.start_camera_stream(camera.index):
                started.append(camera.index)
                logger.info("Successfully started camera %d", camera.index)
            else:
                failed.append(camera.index)
                logger.error("Failed to start camera %d", camera.index)

        if failed:
            logger.warning("Failed to start cameras: %s", failed)

        logger.info("Started %d out of %d cameras", len(started), len(selected_cameras))
        return started

    def stop_all_cameras(self) -> None:
        """Stop all active camera streams."""
        camera_indices = list(self.active_captures.keys())
        for camera_index in camera_indices:
            self.stop_camera_stream(camera_index)

    def capture_high_resolution_image(self, camera_index: int) -> Optional[bytes]:
        """Capture a high-resolution image from the specified camera with EXIF metadata."""
        if camera_index not in self.cameras:
            logger.warning("Camera %d not found", camera_index)
            return None

        camera_info = self.cameras[camera_index]

        try:
            if camera_info.is_network_camera:
                # Handle network camera capture
                logger.info(
                    "Capturing high-resolution image from network camera %d",
                    camera_index,
                )

                # Get image from ESPHome camera using async method
                try:
                    loop = asyncio.get_event_loop()
                    if loop.is_running():
                        # If we're already in an event loop, create a task

                        with concurrent.futures.ThreadPoolExecutor() as executor:
                            future = executor.submit(
                                asyncio.run,
                                self._capture_network_image_async(camera_info),
                            )
                            image_data = future.result(timeout=15)
                    else:
                        image_data = asyncio.run(
                            self._capture_network_image_async(camera_info)
                        )

                    if image_data is None:
                        logger.error(
                            "Failed to capture from network camera %d", camera_index
                        )
                        return None

                    # Convert response to PIL Image
                    pil_image = Image.open(BytesIO(image_data))

                    # Convert to RGB if needed
                    if pil_image.mode != "RGB":
                        pil_image = pil_image.convert("RGB")  # type: ignore[assignment]

                except Exception as e:
                    logger.error(
                        "Error capturing from network camera %d: %s", camera_index, e
                    )
                    return None

            else:
                # Handle USB camera capture
                # Open camera for high-resolution capture
                cap = self._open_camera_with_timeout(camera_index, timeout=5.0)
                if cap is None:
                    logger.error(
                        "Failed to open camera %d for high-resolution capture",
                        camera_index,
                    )
                    return None

                # Get maximum resolution supported by the camera
                max_width = int(cap.get(cv2.CAP_PROP_FRAME_WIDTH))
                max_height = int(cap.get(cv2.CAP_PROP_FRAME_HEIGHT))

                # Try to set to maximum resolution if different from current
                if (
                    max_width != camera_info.resolution[0]
                    or max_height != camera_info.resolution[1]
                ):
                    cap.set(cv2.CAP_PROP_FRAME_WIDTH, camera_info.resolution[0])
                    cap.set(cv2.CAP_PROP_FRAME_HEIGHT, camera_info.resolution[1])

                # Capture frame
                ret, frame = cap.read()
                cap.release()

                if not ret or frame is None:
                    logger.error("Failed to capture frame from camera %d", camera_index)
                    return None

                # Convert BGR to RGB for PIL
                frame_rgb = cv2.cvtColor(frame, cv2.COLOR_BGR2RGB)

                # Create PIL Image
                pil_image = Image.fromarray(frame_rgb)  # type: ignore[assignment]

            # Add EXIF metadata
            exif_dict: Dict[str, Any] = {
                "0th": {},
                "Exif": {},
                "GPS": {},
                "1st": {},
                "thumbnail": None,
            }

            # Add camera name to ImageDescription
            # Find the tag number for ImageDescription
            image_desc_tag = None
            for tag_num, tag_name in TAGS.items():
                if tag_name == "ImageDescription":
                    image_desc_tag = tag_num
                    break

            if image_desc_tag and exif_dict["0th"] is not None:
                exif_dict["0th"][image_desc_tag] = camera_info.name

            # Add view type to UserComment if available
            user_comment_tag = None
            for tag_num, tag_name in TAGS.items():
                if tag_name == "UserComment":
                    user_comment_tag = tag_num
                    break

            if (
                user_comment_tag
                and camera_info.view_type
                and exif_dict["Exif"] is not None
            ):
                # UserComment needs special encoding
                comment = f"view_type:{camera_info.view_type}"
                # Prefix with character code (ASCII)
                encoded_comment = b"ASCII\x00\x00\x00" + comment.encode("ascii")
                exif_dict["Exif"][user_comment_tag] = encoded_comment

            # Save image with EXIF to bytes
            output = BytesIO()

            # Save with EXIF metadata
            exif_bytes = piexif.dump(exif_dict)
            pil_image.save(output, format="JPEG", quality=95, exif=exif_bytes)

            return output.getvalue()

        except Exception as e:
            logger.error(
                "Error capturing high-resolution image from camera %d: %s",
                camera_index,
                e,
            )
            return None

    def capture_all_selected_high_resolution(self) -> Dict[int, Optional[bytes]]:
        """Capture high-resolution images from all selected cameras."""
        selected_cameras = self.get_selected_cameras()
        if not selected_cameras:
            logger.warning("No cameras selected for capture")
            return {}

        results = {}
        for camera in selected_cameras:
            logger.info("Capturing high-resolution image from camera %d", camera.index)
            image_data = self.capture_high_resolution_image(camera.index)
            results[camera.index] = image_data

            if image_data:
                logger.info(
                    "Successfully captured high-resolution image from camera %d (%d bytes)",
                    camera.index,
                    len(image_data),
                )
            else:
                logger.error(
                    "Failed to capture high-resolution image from camera %d",
                    camera.index,
                )

        return results

    def cleanup(self) -> None:
        """Clean up all camera resources."""
        logger.info("Cleaning up camera resources...")
        self.stop_all_cameras()

        # Wait for all threads to finish
        for camera_index, thread in list(self.streaming_threads.items()):
            if thread.is_alive():
                logger.info("Waiting for camera %d thread to finish...", camera_index)
                thread.join(timeout=2.0)
                if thread.is_alive():
                    logger.warning(
                        "Camera %d thread did not finish cleanly", camera_index
                    )

        # Clear all data structures
        self.cameras.clear()
        self.active_captures.clear()
        self.streaming_threads.clear()
        self.stop_streaming.clear()
        self.latest_frames.clear()

        logger.info("Camera cleanup completed")

    def _load_camera_config(self, camera_info: CameraInfo) -> None:
        """Load camera configuration from user config using hardware ID."""
        if not self.settings or not camera_info.hardware_id:
            return

        try:
            user_config_data = self.settings.load_user_config()
            user_config = UserConfig(**user_config_data)

            # Try to load by hardware ID first (new stable method)
            camera_config = user_config.get_camera_config(camera_info.hardware_id)
            
            # If no config found by hardware ID, try legacy name-based lookup
            if not any([camera_config.view_type, camera_config.region_x, camera_config.region_y]):
                legacy_config = user_config.get_camera_config(camera_info.name)
                if any([legacy_config.view_type, legacy_config.region_x, legacy_config.region_y]):
                    camera_config = legacy_config
                    logger.info("Migrating camera config from name-based to hardware ID: %s -> %s", 
                              camera_info.name, camera_info.hardware_id)
                    # Save the config under the new hardware ID
                    user_config.set_camera_config(camera_info.hardware_id, camera_config)
                    # Remove the old name-based config
                    if camera_info.name in user_config.camera_configs:
                        del user_config.camera_configs[camera_info.name]
                    # Save the updated config
                    self.settings.save_user_config(user_config.model_dump())

            # Apply saved configuration
            camera_info.view_type = camera_config.view_type
            camera_info.region_x = camera_config.region_x
            camera_info.region_y = camera_config.region_y
            camera_info.region_width = camera_config.region_width
            camera_info.region_height = camera_config.region_height

            if camera_config.view_type:
                logger.info(
                    "Loaded camera %s (ID: %s) config: view_type=%s",
                    camera_info.name,
                    camera_info.hardware_id,
                    camera_config.view_type,
                )

        except Exception as e:
            logger.warning(
                "Failed to load camera config for %s (ID: %s): %s", 
                camera_info.name, camera_info.hardware_id, e
            )

    def _save_camera_config(self, camera_info: CameraInfo) -> None:
        """Save camera configuration to user config using hardware ID."""
        if not self.settings or not camera_info.hardware_id:
            return

        try:
            # Load current user config
            user_config_data = self.settings.load_user_config()
            user_config = UserConfig(**user_config_data)

            # Update camera configuration
            camera_config = CameraConfig(
                view_type=camera_info.view_type,
                region_x=camera_info.region_x,
                region_y=camera_info.region_y,
                region_width=camera_info.region_width,
                region_height=camera_info.region_height,
            )

            # Save using hardware ID for stable identification
            user_config.set_camera_config(camera_info.hardware_id, camera_config)
            
            # Remove any legacy name-based config if it exists
            if camera_info.name in user_config.camera_configs:
                del user_config.camera_configs[camera_info.name]
                logger.debug("Removed legacy camera config for name: %s", camera_info.name)

            # Save to file
            success = self.settings.save_user_config(user_config.model_dump())
            if success:
                logger.info("Saved camera %s (ID: %s) config to user config", 
                          camera_info.name, camera_info.hardware_id)

        except Exception as e:
            logger.error("Failed to save camera config for %s (ID: %s): %s", 
                        camera_info.name, camera_info.hardware_id, e)

    def remove_camera(self, camera_index: int) -> bool:
        """Remove a camera from the configuration."""
        try:
            if camera_index not in self.cameras:
                logger.warning("Camera %d not found for removal", camera_index)
                return False

            # Stop camera if it's active
            self.stop_camera_stream(camera_index)

            # Remove from cameras dict
            camera_info = self.cameras[camera_index]
            del self.cameras[camera_index]

            # Remove from user config if settings available
            if self.settings:
                try:
                    user_config_data = self.settings.load_user_config()
                    user_config = UserConfig(**user_config_data)
                    
                    # Remove by hardware ID (primary method)
                    if camera_info.hardware_id and camera_info.hardware_id in user_config.camera_configs:
                        del user_config.camera_configs[camera_info.hardware_id]
                        logger.debug("Removed camera config by hardware ID: %s", camera_info.hardware_id)
                    
                    # Also remove any legacy name-based config
                    if camera_info.name in user_config.camera_configs:
                        del user_config.camera_configs[camera_info.name]
                        logger.debug("Removed legacy camera config by name: %s", camera_info.name)
                    
                    self.settings.save_user_config(user_config.model_dump())
                except Exception as e:
                    logger.warning("Failed to remove camera from user config: %s", e)

            logger.info(
                "Removed camera %d (%s) from configuration",
                camera_index,
                camera_info.name,
            )
            return True

        except Exception as e:
            logger.error("Error removing camera %d: %s", camera_index, e)
            return False

    def clear_cameras(self) -> None:
        """Clear all cameras from configuration."""
        try:
            # Stop all active cameras
            self.stop_all_cameras()

            # Clear cameras dict
            camera_configs_to_remove = []
            for cam in self.cameras.values():
                if cam.hardware_id:
                    camera_configs_to_remove.append(cam.hardware_id)
                camera_configs_to_remove.append(cam.name)  # Also remove legacy name-based configs
            
            self.cameras.clear()

            # Clear user config if settings available
            if self.settings:
                try:
                    user_config_data = self.settings.load_user_config()
                    user_config = UserConfig(**user_config_data)
                    
                    # Remove all camera configurations (both hardware ID and name-based)
                    for config_key in camera_configs_to_remove:
                        if config_key in user_config.camera_configs:
                            del user_config.camera_configs[config_key]
                            logger.debug("Removed camera config: %s", config_key)
                    
                    self.settings.save_user_config(user_config.model_dump())
                except Exception as e:
                    logger.warning("Failed to clear cameras from user config: %s", e)

            logger.info("Cleared all cameras from configuration")

        except Exception as e:
            logger.error("Error clearing cameras: %s", e)

    def reset_to_defaults(self) -> None:
        """Reset camera manager to default settings."""
        try:
            # Stop all cameras and clear
            self.clear_cameras()

            # Reset settings
            self.auto_start_cameras = False

            # Clear user config entirely if settings available
            if self.settings:
                try:
                    default_config = UserConfig()
                    self.settings.save_user_config(default_config.model_dump())
                except Exception as e:
                    logger.warning("Failed to reset user config: %s", e)

            logger.info("Reset camera manager to defaults")

        except Exception as e:
            logger.error("Error resetting to defaults: %s", e)

    def save_config(self) -> None:
        """Save current configuration including auto-start setting."""
        try:
            if not self.settings:
                logger.warning("No settings available to save config")
                return

            # Save all current camera configs
            for camera in self.cameras.values():
                self._save_camera_config(camera)

            logger.info("Saved camera manager configuration")

        except Exception as e:
            logger.error("Error saving config: %s", e)
