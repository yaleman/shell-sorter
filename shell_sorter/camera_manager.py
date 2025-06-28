"""Camera management for shell sorting machine."""

import cv2  # type: ignore[import-not-found]
import threading
from typing import Dict, List, Optional, Tuple
from dataclasses import dataclass
import logging
import concurrent.futures

logger = logging.getLogger(__name__)


@dataclass
class CameraInfo:
    """Information about a connected camera."""
    index: int
    name: str
    resolution: Tuple[int, int]
    is_active: bool = False
    is_selected: bool = False


class CameraManager:
    """Manages camera detection, selection, and streaming."""
    
    def __init__(self) -> None:
        self.cameras: Dict[int, CameraInfo] = {}
        self.active_captures: Dict[int, cv2.VideoCapture] = {}
        self.streaming_threads: Dict[int, threading.Thread] = {}
        self.stop_streaming: Dict[int, threading.Event] = {}
        self.latest_frames: Dict[int, Optional[bytes]] = {}
        self._lock = threading.Lock()
        
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
                
                camera_info = CameraInfo(
                    index=i,
                    name=f"Camera {i}",
                    resolution=(width, height)
                )
                cameras.append(camera_info)
                self.cameras[i] = camera_info
                
                logger.info("Detected camera %d with resolution %dx%d", i, width, height)
            
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
    
    def _open_camera_with_timeout(self, camera_index: int, timeout: float = 3.0) -> Optional[cv2.VideoCapture]:
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
                logger.warning("Camera %d open timed out after %gs", camera_index, timeout)
                return None
    
    def start_camera_stream(self, camera_index: int) -> bool:
        """Start streaming from a specific camera."""
        try:
            if camera_index not in self.cameras:
                logger.warning("Camera %d not found in detected cameras", camera_index)
                return False
            
            with self._lock:
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
                    target=self._stream_camera,
                    args=(camera_index,),
                    daemon=True
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
                    if stop_event.wait(0.1):  # Wait with timeout for responsive shutdown
                        break
                    continue
                
                # Encode frame as JPEG
                _, buffer = cv2.imencode('.jpg', frame, [cv2.IMWRITE_JPEG_QUALITY, 85])
                self.latest_frames[camera_index] = buffer.tobytes()
                
                # Small delay to limit CPU usage, but check for stop signal
                if stop_event.wait(1/30):  # ~30 FPS but responsive to stop
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
        
        logger.info("Attempting to start %d selected cameras: %s", len(selected_cameras), [c.index for c in selected_cameras])
        
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
                    logger.warning("Camera %d thread did not finish cleanly", camera_index)
        
        # Clear all data structures
        self.cameras.clear()
        self.active_captures.clear()
        self.streaming_threads.clear()
        self.stop_streaming.clear()
        self.latest_frames.clear()
        
        logger.info("Camera cleanup completed")