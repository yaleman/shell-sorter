"""Camera management for shell sorting machine."""

import threading
from typing import Dict, List, Optional, Tuple, Literal, TYPE_CHECKING, Any
from dataclasses import dataclass
import logging
import concurrent.futures
import subprocess
import platform
import json
from io import BytesIO

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

    def detect_cameras(self) -> List[CameraInfo]:
        """Detect all available cameras."""
        cameras = []

        # Try camera indices 0-9
        for i in range(10):
            cap = cv2.VideoCapture(i)
            if cap.isOpened():
                # Get camera resolution
                width = int(cap.get(cv2.CAP_PROP_FRAME_WIDTH))
                height = int(cap.get(cv2.CAP_PROP_FRAME_HEIGHT))

                # Get actual device name/model
                camera_name = self._get_camera_device_name(i)

                camera_info = CameraInfo(
                    index=i,
                    name=camera_name,
                    resolution=(width, height),
                    is_selected=True,
                )

                # Load camera configuration from user config if available
                if self.settings:
                    self._load_camera_config(camera_info)

                cameras.append(camera_info)
                self.cameras[i] = camera_info

                logger.info(
                    "Detected camera %d with resolution %dx%d", i, width, height
                )

            cap.release()

        return cameras

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

            # Stop existing stream if running
            self.stop_camera_stream(camera_index)

            # Create new capture with timeout
            logger.info("Opening camera %d...", camera_index)
            cap = self._open_camera_with_timeout(camera_index, timeout=3.0)
            if cap is None:
                logger.error("Failed to open camera %d within timeout", camera_index)
                return False

            # Set camera properties for better performance
            cap.set(cv2.CAP_PROP_FRAME_WIDTH, 640)
            cap.set(cv2.CAP_PROP_FRAME_HEIGHT, 480)
            cap.set(cv2.CAP_PROP_FPS, 30)

            self.active_captures[camera_index] = cap
            self.stop_streaming[camera_index] = threading.Event()
            self.latest_frames[camera_index] = None

            # Start streaming thread
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

        logger.info("Started streaming thread for camera %d", camera_index)

        while not stop_event.is_set():
            try:
                ret, frame = cap.read()
                if not ret:
                    logger.warning("Failed to read frame from camera %d", camera_index)
                    if stop_event.wait(
                        0.1
                    ):  # Wait with timeout for responsive shutdown
                        break
                    continue

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
            # Open camera for high-resolution capture
            cap = self._open_camera_with_timeout(camera_index, timeout=5.0)
            if cap is None:
                logger.error(
                    "Failed to open camera %d for high-resolution capture", camera_index
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
            pil_image = Image.fromarray(frame_rgb)

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
        """Load camera configuration from user config."""
        if not self.settings:
            return

        try:
            user_config_data = self.settings.load_user_config()
            user_config = UserConfig(**user_config_data)

            camera_config = user_config.get_camera_config(camera_info.name)

            # Apply saved configuration
            camera_info.view_type = camera_config.view_type
            camera_info.region_x = camera_config.region_x
            camera_info.region_y = camera_config.region_y
            camera_info.region_width = camera_config.region_width
            camera_info.region_height = camera_config.region_height

            if camera_config.view_type:
                logger.info(
                    "Loaded camera %s config: view_type=%s",
                    camera_info.name,
                    camera_config.view_type,
                )

        except Exception as e:
            logger.warning(
                "Failed to load camera config for %s: %s", camera_info.name, e
            )

    def _save_camera_config(self, camera_info: CameraInfo) -> None:
        """Save camera configuration to user config."""
        if not self.settings:
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

            user_config.set_camera_config(camera_info.name, camera_config)

            # Save to file
            success = self.settings.save_user_config(user_config.model_dump())
            if success:
                logger.info("Saved camera %s config to user config", camera_info.name)

        except Exception as e:
            logger.error("Failed to save camera config for %s: %s", camera_info.name, e)

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
                    user_config.remove_camera_config(camera_info.name)
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
            camera_names = [cam.name for cam in self.cameras.values()]
            self.cameras.clear()

            # Clear user config if settings available
            if self.settings:
                try:
                    user_config_data = self.settings.load_user_config()
                    user_config = UserConfig(**user_config_data)
                    for camera_name in camera_names:
                        user_config.remove_camera_config(camera_name)
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
