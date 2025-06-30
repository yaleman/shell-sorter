"""Pytest configuration and fixtures."""

import pytest
import tempfile
from pathlib import Path
from unittest.mock import MagicMock, patch
from typing import Dict, Any, Generator

from shell_sorter.config import Settings
from shell_sorter.camera_manager import CameraManager


@pytest.fixture
def temp_data_dir() -> Generator[Path, None, None]:
    """Create a temporary directory for test data."""
    with tempfile.TemporaryDirectory() as temp_dir:
        yield Path(temp_dir)


@pytest.fixture
def mock_settings(temp_data_dir: Path) -> Settings:
    """Create mock settings with temporary directories."""
    settings = Settings(
        data_directory=temp_data_dir / "data",
        image_directory=temp_data_dir / "images",
        debug=True,
        esphome_hostname="test-controller.local",
    )
    
    # Ensure directories exist
    settings.data_directory.mkdir(exist_ok=True)
    settings.image_directory.mkdir(exist_ok=True)
    
    return settings


@pytest.fixture
def mock_camera_manager(mock_settings: Settings) -> CameraManager:
    """Create a camera manager with mocked hardware calls."""
    with patch('shell_sorter.camera_manager.cv2') as mock_cv2, \
         patch('shell_sorter.camera_manager.subprocess') as mock_subprocess:
        
        # Mock cv2.VideoCapture to avoid accessing real cameras
        mock_capture = MagicMock()
        mock_capture.isOpened.return_value = False  # No cameras by default
        mock_cv2.VideoCapture.return_value = mock_capture
        
        # Mock subprocess calls to avoid system calls
        mock_subprocess.run.return_value = MagicMock(returncode=1, stdout="", stderr="")
        
        camera_manager = CameraManager(mock_settings)
        return camera_manager


@pytest.fixture
def sample_camera_info() -> Dict[str, Any]:
    """Sample camera information for testing."""
    return {
        "index": 0,
        "name": "Test USB Camera",
        "resolution": (640, 480),
        "is_active": False,
        "is_selected": True,
        "view_type": "side",
        "region_x": 100,
        "region_y": 100,
        "region_width": 200,
        "region_height": 200,
        "is_network_camera": False,
        "device_path": "/dev/video0",
        "vendor_id": "1234",
        "product_id": "5678",
        "serial_number": "ABC123",
        "hardware_id": "1234:5678:ABC123",
    }


@pytest.fixture
def sample_network_camera_info() -> Dict[str, Any]:
    """Sample network camera information for testing."""
    return {
        "index": 1000,
        "name": "ESPHome Camera (esp32cam1.local)",
        "resolution": (800, 600),
        "is_active": False,
        "is_selected": True,
        "view_type": "tail",
        "region_x": 200,
        "region_y": 150,
        "region_width": 300,
        "region_height": 250,
        "is_network_camera": True,
        "stream_url": "http://esp32cam1.local/camera",
        "hostname": "esp32cam1.local",
        "hardware_id": "network:esp32cam1.local",
    }


@pytest.fixture(autouse=True)
def mock_platform_calls():
    """Mock platform-specific calls to avoid system dependencies."""
    with patch('shell_sorter.camera_manager.platform.system') as mock_platform, \
         patch('shell_sorter.camera_manager.subprocess.run') as mock_subprocess:
        
        mock_platform.return_value = "Linux"  # Default to Linux for testing
        mock_subprocess.return_value = MagicMock(returncode=1, stdout="", stderr="")
        
        yield