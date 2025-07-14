// Toast notification system
function showToast(message, type = 'info', duration = 5000) {
    const container = document.getElementById('toast-container');
    if (!container) return;

    const toast = document.createElement('div');
    toast.className = `toast ${type}`;

    const icons = {
        success: '✓',
        error: '✗',
        warning: '⚠',
        info: 'ℹ'
    };

    toast.innerHTML = `
        <span class="toast-icon">${icons[type] || icons.info}</span>
        <div class="toast-content">${message}</div>
        <button class="toast-close">&times;</button>
    `;

    container.appendChild(toast);

    // Show toast with animation
    setTimeout(() => toast.classList.add('show'), 100);

    // Auto-remove after duration
    const autoRemove = setTimeout(() => removeToast(toast), duration);

    // Manual close button
    const closeBtn = toast.querySelector('.toast-close');
    closeBtn.addEventListener('click', () => {
        clearTimeout(autoRemove);
        removeToast(toast);
    });
}

function removeToast(toast) {
    toast.classList.remove('show');
    setTimeout(() => {
        if (toast.parentNode) {
            toast.parentNode.removeChild(toast);
        }
    }, 300);
}

function updateCameraSelection() {
    const checkboxes = document.querySelectorAll('.camera-checkbox');
    checkboxes.forEach(checkbox => {
        const cameraItem = checkbox.closest('.camera-item');
        if (checkbox.checked) {
            cameraItem.classList.add('selected');
        } else {
            cameraItem.classList.remove('selected');
        }
    });

    // Always show overlays when cameras are active
    showRegionOverlays();
}

// Region data management with localStorage
const RegionStorage = {
    STORAGE_KEY: 'shell-sorter-regions',

    // Load region data from localStorage
    load() {
        try {
            const data = localStorage.getItem(this.STORAGE_KEY);
            return data ? JSON.parse(data) : {};
        } catch (error) {
            console.warn('Failed to load region data from localStorage:', error);
            return {};
        }
    },

    // Save region data to localStorage
    save(regions) {
        try {
            localStorage.setItem(this.STORAGE_KEY, JSON.stringify(regions));
            console.log('Saved region data to localStorage:', regions);
        } catch (error) {
            console.error('Failed to save region data to localStorage:', error);
        }
    },

    // Get region for specific camera
    getRegion(cameraIndex) {
        const regions = this.load();
        return regions[cameraIndex] || null;
    },

    // Set region for specific camera
    setRegion(cameraIndex, regionData) {
        const regions = this.load();
        regions[cameraIndex] = regionData;
        this.save(regions);
    },

    // Remove region for specific camera
    removeRegion(cameraIndex) {
        const regions = this.load();
        delete regions[cameraIndex];
        this.save(regions);
    },

    // Sync with server data (called on page load)
    syncWithServer(cameraData) {
        const regions = this.load();
        let updated = false;

        // Ensure cameraData is an array
        if (!Array.isArray(cameraData)) {
            console.warn('syncWithServer: cameraData is not an array:', cameraData);
            return;
        }

        cameraData.forEach(camera => {
            if (camera.region_x !== null && camera.region_x !== undefined) {
                const serverRegion = {
                    x: camera.region_x,
                    y: camera.region_y,
                    width: camera.region_width,
                    height: camera.region_height
                };

                // Update localStorage if server has newer data
                if (!regions[camera.index] || JSON.stringify(regions[camera.index]) !== JSON.stringify(serverRegion)) {
                    regions[camera.index] = serverRegion;
                    updated = true;
                }
            }
        });

        if (updated) {
            this.save(regions);
            console.log('Synced region data with server');
        }
    }
};

// Function to load and display cameras in the camera list
async function loadCameras() {
    try {
        const response = await fetch('/api/cameras');
        if (response.ok) {
            const apiResponse = await response.json();
            if (apiResponse.success && apiResponse.data) {
                const cameras = apiResponse.data;
                displayCameras(cameras);
            } else {
                console.warn('Failed to load cameras:', apiResponse.message);
                displayNoCameras();
            }
        } else {
            console.warn('Failed to fetch cameras');
            displayNoCameras();
        }
    } catch (error) {
        console.error('Error loading cameras:', error);
        displayNoCameras();
    }
}

// Function to display cameras in the camera list
function displayCameras(cameras) {
    const cameraList = document.getElementById('camera-list');
    if (!cameraList) return;

    if (cameras.length === 0) {
        displayNoCameras();
        return;
    }

    // Clear existing content
    cameraList.innerHTML = '';

    cameras.forEach(camera => {
        const cameraItem = document.createElement('div');
        cameraItem.className = 'camera-item';
        cameraItem.dataset.cameraId = camera.id;
        // Also set camera-index for backward compatibility with existing region code
        if (camera.index !== undefined) {
            cameraItem.dataset.cameraIndex = camera.index;
        }

        cameraItem.innerHTML = `
            <div class="camera-header">
                <label class="camera-checkbox-label">
                    <input type="checkbox" class="camera-checkbox" data-camera-id="${camera.id}" ${camera.is_selected ? 'checked' : ''}>
                    <span class="camera-name">${camera.name}</span>
                    <span class="camera-type">(${camera.camera_type})</span>
                    <span class="camera-details">
                        <span class="camera-info-icon" title="Camera Details">ℹ️</span>
                        <div class="camera-info-tooltip">
                            <div><strong>ID:</strong> ${camera.id}</div>
                            ${camera.hostname ? `<div><strong>Host:</strong> ${camera.hostname}</div>` : ''}
                            ${camera.index !== undefined ? `<div><strong>Index:</strong> ${camera.index}</div>` : ''}
                            ${camera.vendor_id ? `<div><strong>Vendor:</strong> ${camera.vendor_id}</div>` : ''}
                            ${camera.product_id ? `<div><strong>Product:</strong> ${camera.product_id}</div>` : ''}
                            ${camera.serial_number ? `<div><strong>Serial:</strong> ${camera.serial_number}</div>` : ''}
                        </div>
                    </span>
                </label>
                <span class="camera-status ${camera.is_active ? 'status-active' : 'status-inactive'}">${camera.is_active ? 'Active' : 'Inactive'}</span>
            </div>
            <div class="camera-controls">
                <button class="btn btn-sm btn-secondary camera-view-type-btn" data-camera-id="${camera.id}" ${camera.index !== undefined ? `data-camera-index="${camera.index}"` : ''}>
                    Set View Type
                </button>
                <button class="btn btn-sm btn-secondary camera-region-btn" data-camera-id="${camera.id}" ${camera.index !== undefined ? `data-camera-index="${camera.index}"` : ''}>
                    Set Region
                </button>
                <button class="btn btn-sm btn-secondary camera-autofocus-btn" data-camera-id="${camera.id}" ${camera.index !== undefined ? `data-camera-index="${camera.index}"` : ''}>
                    Autofocus
                </button>
                ${camera.camera_type === 'usb' ? `
                    <div class="brightness-control">
                        <label for="brightness-${camera.id}" class="brightness-label">Brightness:</label>
                        <input type="range" id="brightness-${camera.id}" class="brightness-slider" 
                               min="0" max="255" step="1" value="128" 
                               data-camera-id="${camera.id}">
                        <span class="brightness-value" id="brightness-value-${camera.id}">128</span>
                    </div>
                ` : ''}
            </div>
        `;

        cameraList.appendChild(cameraItem);
    });

    console.log(`Displayed ${cameras.length} cameras`);

    // Update camera selection state immediately after displaying
    updateCameraSelection();
}

// Function to display "no cameras" message
function displayNoCameras() {
    const cameraList = document.getElementById('camera-list');
    if (!cameraList) return;

    cameraList.innerHTML = '<p class="no-cameras">No cameras detected. Click "Detect Cameras" to search for available cameras.</p>';
}

async function updateCameraFeeds() {
    try {
        const controller = new AbortController();
        const timeoutId = setTimeout(() => controller.abort(), 2500);

        const response = await fetch('/api/cameras', {
            signal: controller.signal
        });

        clearTimeout(timeoutId);

        if (response.ok) {
            const result = await response.json();
            const cameras = result.success ? result.data || [] : [];
            console.debug('Updating camera feeds:', cameras); // Debug log
            cameras.forEach(camera => {
                const cameraItem = document.querySelector(`[data-camera-id="${camera.id}"]`);
                if (cameraItem) {
                    const statusSpan = cameraItem.querySelector('.camera-status');
                    const existingFeed = cameraItem.querySelector('.camera-feed');

                    // Update status
                    if (statusSpan) {
                        statusSpan.className = `camera-status status-${camera.is_active ? 'active' : 'inactive'}`;
                        statusSpan.textContent = camera.is_active ? 'Active' : 'Inactive';
                    }

                    // Add or remove camera feed
                    if (camera.is_active && !existingFeed) {
                        const feedDiv = document.createElement('div');
                        feedDiv.className = 'camera-feed';
                        feedDiv.dataset.cameraId = camera.id;
                        // Also set camera-index for backward compatibility with region code
                        if (camera.index !== undefined) {
                            feedDiv.dataset.cameraIndex = camera.index.toString();
                        }

                        // Add region data if available
                        if (camera.region_x !== null && camera.region_x !== undefined) {
                            feedDiv.dataset.region = JSON.stringify({
                                x: camera.region_x,
                                y: camera.region_y,
                                width: camera.region_width,
                                height: camera.region_height
                            });
                        }

                        feedDiv.innerHTML = `<img src="/api/cameras/${camera.id}/stream" 
                                                     alt="Camera ${camera.name} feed"
                                                     class="camera-stream"
                                                     onerror="console.error('Failed to load camera stream for ${camera.id}')">`;
                        cameraItem.appendChild(feedDiv);
                        console.log(`Created camera feed for ${camera.id} with stream URL: /api/cameras/${camera.id}/stream`);
                    } else if (!camera.is_active && existingFeed) {
                        existingFeed.remove();
                        console.log(`Removed camera feed for ${camera.id}`);
                    } else if (camera.is_active && existingFeed) {
                        console.log(`Camera feed already exists for ${camera.id}`);
                    }
                } else {
                    console.warn(`Could not find camera item for ID: ${camera.id}`);
                }
            });

            // Show overlays when camera status changes
            showRegionOverlays();

            return cameras;
        }
    } catch (error) {
        console.error('Error updating camera feeds:', error);
    }
    return null;
}

let currentCameraPollInterval = null;

function pollForCameraUpdates() {
    // Clear any existing polling first
    if (currentCameraPollInterval) {
        clearInterval(currentCameraPollInterval);
    }

    console.log('Starting camera status polling...');
    let pollCount = 0;
    const maxPolls = 12; // Poll for up to 60 seconds (12 * 5s)

    currentCameraPollInterval = setInterval(async () => {
        pollCount++;
        console.log(`Camera status poll ${pollCount}/${maxPolls}`);

        const cameras = await updateCameraFeeds();

        // Check if any selected cameras are now active
        if (cameras) {
            const selectedActive = cameras.filter(cam => cam.is_selected && cam.is_active);
            const selectedTotal = cameras.filter(cam => cam.is_selected);

            console.log(`Active cameras: ${selectedActive.length}/${selectedTotal.length}`);

            // Stop polling if all selected cameras are active or we've reached max polls
            if (selectedActive.length === selectedTotal.length || pollCount >= maxPolls) {
                clearInterval(currentCameraPollInterval);
                currentCameraPollInterval = null;
                if (selectedActive.length === selectedTotal.length) {
                    console.log('All selected cameras are now active');
                } else {
                    console.log('Stopped polling - some cameras may have failed to start');
                }
            }
        }
    }, 5000); // Poll every 5 seconds
}




function updateStatusDisplay(status) {
    const statusIndicator = document.querySelector('.status-indicator');
    if (statusIndicator) {
        statusIndicator.className = `status-indicator status-${status.status}`;
        statusIndicator.textContent = `Status: ${status.status.charAt(0).toUpperCase() + status.status.slice(1)}`;
    }

    const totalSorted = document.querySelector('.status-item .value');
    if (totalSorted) {
        totalSorted.textContent = status.total_sorted;
    }
}

// Initialize overlay state on page load - always show overlays
initializeOverlays();

function initializeOverlays() {
    // Always show overlays when available
    showRegionOverlays();
}

function showRegionOverlays() {
    let overlays = document.querySelectorAll('.camera-region-overlay');

    // If no overlays exist, try to create them from region buttons
    if (overlays.length === 0) {
        const created = createMissingOverlays();
        overlays = document.querySelectorAll('.camera-region-overlay');
        if (created > 0) {
            console.log(`Successfully created ${created} overlays`);
        }
    }

    // Show all overlays
    overlays.forEach(overlay => {
        updateOverlayPosition(overlay);
        overlay.style.display = 'block';
    });

    console.log(`Showing ${overlays.length} region overlays`);
}

function createMissingOverlays() {
    let overlaysCreated = 0;

    // Look for camera feeds that have region info but no overlay
    const cameraFeeds = document.querySelectorAll('.camera-feed');
    console.log(`Checking ${cameraFeeds.length} camera feeds for missing overlays`);

    cameraFeeds.forEach((feed) => {
        const existing = feed.querySelector('.camera-region-overlay');
        if (existing) {
            return; // Already has overlay
        }

        const cameraIndexStr = feed.dataset.cameraIndex;
        const cameraIndex = parseInt(cameraIndexStr);

        if (isNaN(cameraIndex)) {
            console.warn(`Invalid camera index "${cameraIndexStr}" for feed:`, feed);
            return;
        }

        console.log(`Checking camera ${cameraIndex} for region data`);

        // Try multiple sources for region data (in order of preference)
        let region = null;

        // 1. localStorage (most reliable)
        region = RegionStorage.getRegion(cameraIndex);
        if (region) {
            console.log(`Found region data in localStorage for camera ${cameraIndex}:`, region);
        }

        // 2. Template JSON data attribute
        if (!region && feed.dataset.region) {
            try {
                region = JSON.parse(feed.dataset.region);
                console.log(`Found region data in template for camera ${cameraIndex}:`, region);
                // Store in localStorage for future use
                RegionStorage.setRegion(cameraIndex, region);
            } catch (error) {
                console.error(`Failed to parse template region JSON for camera ${cameraIndex}:`, error);
            }
        }

        if (region) {
            const { x, y, width, height } = region;

            // Create overlay element
            const overlay = document.createElement('div');
            overlay.className = 'camera-region-overlay';
            overlay.dataset.regionX = x.toString();
            overlay.dataset.regionY = y.toString();
            overlay.dataset.regionWidth = width.toString();
            overlay.dataset.regionHeight = height.toString();
            overlay.style.display = 'none';

            feed.appendChild(overlay);
            overlaysCreated++;
            console.log(`Created overlay for camera ${cameraIndex} with region ${x},${y} (${width}x${height})`);
        } else {
            console.log(`No region data found for camera ${cameraIndex}`);
        }
    });

    console.log(`Created ${overlaysCreated} missing overlays`);
    return overlaysCreated;
}

function updateOverlayPosition(overlay) {
    const cameraFeed = overlay.parentElement;
    const cameraImage = cameraFeed.querySelector('.camera-stream');

    if (!cameraImage) return;

    const regionX = parseInt(overlay.dataset.regionX);
    const regionY = parseInt(overlay.dataset.regionY);
    const regionWidth = parseInt(overlay.dataset.regionWidth);
    const regionHeight = parseInt(overlay.dataset.regionHeight);

    // Wait for image to load if not loaded yet
    if (cameraImage.naturalWidth === 0) {
        cameraImage.addEventListener('load', () => updateOverlayPosition(overlay), { once: true });
        return;
    }

    const scaleX = cameraImage.clientWidth / cameraImage.naturalWidth;
    const scaleY = cameraImage.clientHeight / cameraImage.naturalHeight;

    const overlayLeft = regionX * scaleX;
    const overlayTop = regionY * scaleY;
    const overlayWidth = regionWidth * scaleX;
    const overlayHeight = regionHeight * scaleY;

    overlay.style.left = overlayLeft + 'px';
    overlay.style.top = overlayTop + 'px';
    overlay.style.width = overlayWidth + 'px';
    overlay.style.height = overlayHeight + 'px';
}

// ESPHome status monitoring with adaptive polling support
let isControllerOnline = false;
let esphomeStatusInterval = null;

async function updateESPHomeStatus() {
    try {
        const response = await fetch('/api/machine/hardware-status');
        if (response.ok) {
            const status = await response.json();
            const statusElement = document.getElementById('esphome-status');
            const statusText = document.getElementById('esphome-status-text');

            if (statusElement && statusText) {
                // Remove all status classes
                statusElement.className = 'status-indicator';

                const wasOnline = isControllerOnline;
                isControllerOnline = status.data && status.data.controller === 'Connected';

                if (isControllerOnline) {
                    statusElement.classList.add('esphome-status-online');
                    statusText.textContent = 'Online';
                } else {
                    statusElement.classList.add('esphome-status-offline');
                    statusText.textContent = 'Offline';
                }

                // If status changed, update polling interval
                if (wasOnline !== isControllerOnline && esphomeStatusInterval) {
                    clearInterval(esphomeStatusInterval);
                    const pollInterval = isControllerOnline ? 5000 : 1000;
                    esphomeStatusInterval = setInterval(updateESPHomeStatusWithAdaptivePolling, pollInterval);
                    console.log(`Controller status changed, polling every ${pollInterval / 1000}s`);
                }
            }
        }
    } catch (error) {
        console.error('Error fetching ESPHome status:', error);
        const statusElement = document.getElementById('esphome-status');
        const statusText = document.getElementById('esphome-status-text');

        if (statusElement && statusText) {
            statusElement.className = 'status-indicator esphome-status-offline';
            statusText.textContent = 'Error';
        }

        // Set to offline state on error
        const wasOnline = isControllerOnline;
        isControllerOnline = false;

        // Update polling interval if needed
        if (wasOnline !== isControllerOnline && esphomeStatusInterval) {
            clearInterval(esphomeStatusInterval);
            esphomeStatusInterval = setInterval(updateESPHomeStatusWithAdaptivePolling, 1000);
            console.log('Controller error, polling every 1s');
        }
    }
}

async function updateESPHomeStatusWithAdaptivePolling() {
    await updateESPHomeStatus();

    // Clear existing interval
    if (esphomeStatusInterval) {
        clearInterval(esphomeStatusInterval);
    }

    // Set interval based on current status
    const pollInterval = isControllerOnline ? 5000 : 1000; // 5s when online, 1s when offline
    esphomeStatusInterval = setInterval(updateESPHomeStatusWithAdaptivePolling, pollInterval);
}

document.addEventListener('DOMContentLoaded', function () {
    // Camera management elements
    const detectCamerasBtn = document.getElementById('detect-cameras-btn');
    const startSelectedBtn = document.getElementById('start-selected-btn');
    const stopAllBtn = document.getElementById('stop-all-btn');
    const captureImagesBtn = document.getElementById('capture-images-btn');
    const nextCaseBtn = document.getElementById('next-case-btn');
    const mlTrainingBtn = document.getElementById('ml-training-btn');
    const configBtn = document.getElementById('config-btn');

    if (captureImagesBtn) {
        captureImagesBtn.addEventListener('click', async function () {
            try {
                const controller = new AbortController();
                const timeoutId = setTimeout(() => controller.abort(), 10000);

                const response = await fetch('/api/cameras/capture', {
                    method: 'POST',
                    signal: controller.signal
                });

                clearTimeout(timeoutId);

                if (response.ok) {
                    const result = await response.json();
                    showToast(result.message, 'success');
                    // Redirect to tagging interface
                    window.location.href = `/tagging/${result.session_id}`;
                } else {
                    const error = await response.text();
                    showToast('Error capturing images: ' + error, 'error');
                }
            } catch (error) {
                console.error('Error:', error);
                if (error.name === 'AbortError') {
                    showToast('Image capture timed out. Please try again.', 'warning');
                } else {
                    showToast('Error capturing images: ' + error.message, 'error');
                }
            }
        });
    }

    if (nextCaseBtn) {
        nextCaseBtn.addEventListener('click', async function () {
            try {
                const controller = new AbortController();
                const timeoutId = setTimeout(() => controller.abort(), 5000);

                const response = await fetch('/api/machine/next-case', {
                    method: 'POST',
                    signal: controller.signal
                });

                clearTimeout(timeoutId);

                if (response.ok) {
                    const result = await response.json();
                    showToast(result.message, 'success');
                    // Update controller status immediately after successful operation
                    updateESPHomeStatus();
                } else {
                    const error = await response.text();
                    showToast('Error triggering next case: ' + error, 'error');
                }
            } catch (error) {
                console.error('Error:', error);
                if (error.name === 'AbortError') {
                    showToast('Next case request timed out', 'warning');
                } else {
                    showToast('Error triggering next case: ' + error.message, 'error');
                }
            }
        });
    }

    if (mlTrainingBtn) {
        mlTrainingBtn.addEventListener('click', function () {
            window.location.href = '/ml-training';
        });
    }

    if (configBtn) {
        configBtn.addEventListener('click', function () {
            window.location.href = '/config';
        });
    }

    // Auto-refresh status every 25 seconds
    let statusInterval = setInterval(async function () {
        try {
            const controller = new AbortController();
            const timeoutId = setTimeout(() => controller.abort(), 2500);

            const response = await fetch('/api/status', {
                signal: controller.signal
            });

            clearTimeout(timeoutId);

            if (response.ok) {
                const status = await response.json();
                updateStatusDisplay(status);
            }
        } catch (error) {
            console.error('Error fetching status:', error);
        }
    }, 25000);



    // ESPHome status monitoring is now handled globally

    // Initial status check and start adaptive polling
    updateESPHomeStatusWithAdaptivePolling();

    // Clean up intervals when page unloads
    window.addEventListener('beforeunload', function () {
        if (statusInterval) {
            clearInterval(statusInterval);
        }
        if (esphomeStatusInterval) {
            clearInterval(esphomeStatusInterval);
        }
        if (currentCameraPollInterval) {
            clearInterval(currentCameraPollInterval);
        }
    });

    // Camera management functions
    if (detectCamerasBtn) {
        detectCamerasBtn.addEventListener('click', async function () {
            // Show immediate notification that detection is starting
            showToast('Detecting cameras...', 'info');

            try {
                const controller = new AbortController();
                const timeoutId = setTimeout(() => controller.abort(), 10000); // 10 seconds for camera detection

                const response = await fetch('/api/cameras/detect', {
                    signal: controller.signal
                });

                clearTimeout(timeoutId);

                if (response.ok) {
                    const apiResponse = await response.json();
                    if (apiResponse.success && apiResponse.data) {
                        const cameras = apiResponse.data;
                        showToast(`Detected ${cameras.length} cameras`, 'success');
                        displayCameras(cameras);
                        updateCameraSelection();
                    } else {
                        showToast(`Error detecting cameras: ${apiResponse.message || 'Unknown error'}`, 'error');
                    }
                } else {
                    showToast('Error detecting cameras: Server returned an error', 'error');
                }
            } catch (error) {
                console.error('Error:', error);
                if (error.name === 'AbortError') {
                    showToast('Camera detection timed out after 10 seconds. This may indicate no cameras are available or they are slow to respond.', 'warning');
                } else {
                    showToast('Error detecting cameras: ' + error.message, 'error');
                }
            }
        });
    }

    if (startSelectedBtn) {
        startSelectedBtn.addEventListener('click', async function () {
            console.debug('Starting selected cameras...'); // Debug log
            try {
                const controller = new AbortController();
                const timeoutId = setTimeout(() => controller.abort(), 5000); // 5 seconds for API response

                const response = await fetch('/api/cameras/start-selected', {
                    method: 'POST',
                    signal: controller.signal
                });

                clearTimeout(timeoutId);

                if (response.ok) {
                    const result = await response.json();
                    showToast(result.message, 'info');
                    // Cameras are starting in background, poll for updates
                    pollForCameraUpdates();
                } else {
                    showToast('Error starting selected cameras', 'error');
                }
            } catch (error) {
                console.error('Error:', error);
                showToast('Error starting selected cameras', 'error');
            }
        });
    }

    if (stopAllBtn) {
        stopAllBtn.addEventListener('click', async function () {
            try {
                const controller = new AbortController();
                const timeoutId = setTimeout(() => controller.abort(), 2500);

                const response = await fetch('/api/cameras/stop-all', {
                    method: 'POST',
                    signal: controller.signal
                });

                clearTimeout(timeoutId);

                if (response.ok) {
                    const result = await response.json();
                    showToast(result.message, 'success');
                    // Update camera feeds without reloading
                    await updateCameraFeeds();
                } else {
                    showToast('Error stopping cameras', 'error');
                }
            } catch (error) {
                console.error('Error:', error);
                showToast('Error stopping cameras', 'error');
            }
        });
    }

    // Handle view type selection
    document.addEventListener('change', async function (e) {
        if (e.target.classList.contains('view-type-select')) {
            const select = e.target;
            const cameraIndex = parseInt(select.dataset.cameraIndex);
            const viewType = select.value || null;

            try {
                const formData = new FormData();
                if (viewType) {
                    formData.append('view_type', viewType);
                }

                const response = await fetch(`/api/cameras/${cameraIndex}/view-type`, {
                    method: 'POST',
                    body: formData
                });

                if (response.ok) {
                    const result = await response.json();
                    showToast(result.message, 'success');
                    // Reload to update UI (region controls visibility)
                    setTimeout(() => location.reload(), 500);
                } else {
                    const error = await response.text();
                    showToast('Error setting view type: ' + error, 'error');
                    // Revert selection
                    select.value = select.dataset.previousValue || '';
                }
            } catch (error) {
                console.error('Error:', error);
                showToast('Error setting view type: ' + error.message, 'error');
                // Revert selection
                select.value = select.dataset.previousValue || '';
            }
        }
        // Handle camera selection using event delegation
        else if (e.target.classList.contains('camera-checkbox')) {
            const checkbox = e.target;
            const selectedCameras = Array.from(document.querySelectorAll('.camera-checkbox:checked'))
                .map(cb => cb.dataset.cameraId);

            console.log('Selected cameras:', selectedCameras); // Debug log

            try {
                const controller = new AbortController();
                const timeoutId = setTimeout(() => controller.abort(), 2500);

                const response = await fetch('/api/cameras/select', {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json',
                    },
                    body: JSON.stringify({ camera_ids: selectedCameras }),
                    signal: controller.signal
                });

                clearTimeout(timeoutId);

                if (response.ok) {
                    const result = await response.json();
                    console.log('Selection result:', result); // Debug log
                    // Update UI to show selection
                    updateCameraSelection();
                } else {
                    const error = await response.text();
                    console.error('Selection error:', error);
                    showToast('Error selecting cameras: ' + error, 'error');
                    checkbox.checked = !checkbox.checked; // Revert checkbox
                }
            } catch (error) {
                console.error('Error:', error);
                showToast('Error selecting cameras: ' + error.message, 'error');
                checkbox.checked = !checkbox.checked; // Revert checkbox
            }
        }
    });

    // Handle region selection buttons
    document.addEventListener('click', async function (e) {
        if (e.target.classList.contains('region-select-btn')) {
            const cameraIndex = parseInt(e.target.dataset.cameraIndex);
            window.location.href = `/region-selection/${cameraIndex}`;
        }
        else if (e.target.classList.contains('autofocus-btn')) {
            const cameraIndex = parseInt(e.target.dataset.cameraIndex);

            try {
                showToast('Triggering autofocus...', 'info');

                const response = await fetch(`/api/cameras/${cameraIndex}/autofocus`, {
                    method: 'POST'
                });

                if (response.ok) {
                    const result = await response.json();
                    if (result.focus_point) {
                        showToast(`Autofocus triggered at region center (${result.focus_point.x}, ${result.focus_point.y})`, 'success');
                    } else {
                        showToast('Autofocus triggered successfully', 'success');
                    }
                } else {
                    const errorData = await response.json();
                    showToast('Error triggering autofocus: ' + (errorData.detail || response.statusText), 'error');
                }
            } catch (error) {
                console.error('Error:', error);
                showToast('Error triggering autofocus: ' + error.message, 'error');
            }
        }
    });



    // Load and display cameras on page load
    loadCameras();

    // Initialize camera selection display
    updateCameraSelection();

    // Initialize overlays
    showRegionOverlays();

    // Sync region data with server and initialize overlays
    initializeRegionData();

    // Auto-detect cameras if none are currently detected
    autoDetectCameras();

    async function initializeRegionData() {
        try {
            // Fetch current camera data from server
            const response = await fetch('/api/cameras');
            if (response.ok) {
                const result = await response.json();
                const cameras = result.success ? result.data || [] : [];

                // Sync localStorage with server data
                RegionStorage.syncWithServer(cameras);

                // Create any missing overlays now that we have the data
                createMissingOverlays();
            }
        } catch (error) {
            console.warn('Failed to initialize region data:', error);
            // Still try to create overlays from template data
            createMissingOverlays();
        }
    }

    async function autoDetectCameras() {
        try {
            // First check if auto-detect is enabled in configuration
            const configResponse = await fetch('/api/config');
            if (!configResponse.ok) {
                console.log('Failed to get configuration, skipping auto-detection');
                return;
            }

            const config = await configResponse.json();
            if (!config.auto_detect_cameras) {
                console.log('Auto-detect cameras is disabled in configuration');
                return;
            }

            // Check if we already have cameras detected
            const response = await fetch('/api/cameras', {
                signal: AbortSignal.timeout(2500)
            });

            if (response.ok) {
                const result = await response.json();
                const cameras = result.success ? result.data || [] : [];

                // If no cameras are detected, automatically trigger detection
                if (cameras.length === 0) {
                    console.log('No cameras detected, auto-detecting...');
                    showToast('No cameras found, automatically detecting...', 'info');

                    const detectResponse = await fetch('/api/cameras/detect', {
                        signal: AbortSignal.timeout(10000)
                    });

                    if (detectResponse.ok) {
                        const detectApiResponse = await detectResponse.json();
                        if (detectApiResponse.success && detectApiResponse.data) {
                            const detectedCameras = detectApiResponse.data;
                            console.log(`Auto-detected ${detectedCameras.length} cameras`);

                            if (detectedCameras.length > 0) {
                                showToast(`Auto-detected ${detectedCameras.length} cameras`, 'success');
                                displayCameras(detectedCameras);
                                updateCameraSelection();
                            } else {
                                console.log('No cameras found during auto-detection');
                                showToast('No cameras found on this system', 'warning');
                                displayNoCameras();
                            }
                        } else {
                            console.log('Auto-detection failed:', detectApiResponse.message || 'Unknown error');
                            showToast(`Auto-detection failed: ${detectApiResponse.message || 'Unknown error'}`, 'error');
                            displayNoCameras();
                        }
                    } else {
                        console.log('Auto-detection failed');
                        showToast('Auto-detection failed', 'error');
                    }
                } else {
                    console.log(`${cameras.length} cameras already detected`);
                }
            }
        } catch (error) {
            console.log('Auto-detection check failed:', error.message);
            // Silently fail - don't show error toast for automatic detection
        }
    }

    // Debug Console functionality
    function initializeDebugConsole() {
        const showDebugBtn = document.getElementById('show-debug-btn');
        const debugConsole = document.getElementById('debug-console');
        const debugCloseBtn = document.getElementById('debug-close-btn');
        const debugClearBtn = document.getElementById('debug-clear-btn');
        const debugContent = document.getElementById('debug-content');
        const debugLog = document.getElementById('debug-log');
        const debugIndicator = document.getElementById('debug-indicator');
        const debugStatusText = document.getElementById('debug-status-text');

        let debugVisible = false;
        let debugHistory = [];
        let isDebugConnected = false;

        // Show debug console and connect WebSocket
        if (showDebugBtn) {
            showDebugBtn.addEventListener('click', function () {
                debugVisible = true;
                if (debugConsole) {
                    debugConsole.style.display = 'block';
                }
                showDebugBtn.style.display = 'none';

                // Connect WebSocket when console is shown
                if (!isDebugConnected) {
                    connectDebugWebSocket();
                    addDebugEntry('info', 'Debug console connected');
                }
            });
        }

        // Close debug console and disconnect WebSocket
        if (debugCloseBtn) {
            debugCloseBtn.addEventListener('click', function () {
                debugVisible = false;
                if (debugConsole) {
                    debugConsole.style.display = 'none';
                }
                if (showDebugBtn) {
                    showDebugBtn.style.display = 'inline-block';
                }

                // Disconnect WebSocket
                disconnectDebugWebSocket();
                addDebugEntry('info', 'Debug console disconnected');
                showToast('Debug console closed and disconnected', 'info');
            });
        }

        // Clear debug log
        if (debugClearBtn) {
            debugClearBtn.addEventListener('click', function () {
                if (debugLog) {
                    debugLog.innerHTML = '<div class="debug-entry debug-info">' +
                        '<span class="debug-timestamp">' + new Date().toISOString().slice(0, 19) + '</span>' +
                        '<span class="debug-type">INFO</span>' +
                        '<span class="debug-message">Debug console cleared</span>' +
                        '</div>';
                }
                debugHistory = [];
                showToast('Debug console cleared', 'info');
            });
        }

        // Add debug entry function
        window.addDebugEntry = function (type, message, data = null) {
            const timestamp = new Date().toISOString().slice(0, 19);
            const entry = {
                timestamp: timestamp,
                type: type.toUpperCase(),
                message: message,
                data: data
            };

            debugHistory.push(entry);

            // Keep only last 100 entries
            if (debugHistory.length > 100) {
                debugHistory = debugHistory.slice(-100);
            }

            if (debugLog) {
                const entryElement = document.createElement('div');
                entryElement.className = `debug-entry debug-${type.toLowerCase()}`;

                let entryHTML = `
                    <span class="debug-timestamp">${timestamp}</span>
                    <span class="debug-type">${type.toUpperCase()}</span>
                    <span class="debug-message">${message}</span>
                `;

                if (data) {
                    entryHTML += `<span class="debug-data">${JSON.stringify(data, null, 2)}</span>`;
                }

                entryElement.innerHTML = entryHTML;
                debugLog.appendChild(entryElement);

                // Auto-scroll to bottom
                debugLog.scrollTop = debugLog.scrollHeight;

                // Update status indicator
                if (debugIndicator) {
                    debugIndicator.className = `debug-indicator debug-${type.toLowerCase()}`;
                }
                if (debugStatusText) {
                    debugStatusText.textContent = `Last: ${type.toUpperCase()}`;
                }
            }
        };

        // WebSocket connection for real-time ESP command updates
        let debugWebSocket = null;
        let reconnectAttempts = 0;
        const maxReconnectAttempts = 5;
        let shouldReconnect = true;

        function connectDebugWebSocket() {
            if (isDebugConnected) {
                return; // Already connected
            }

            shouldReconnect = true;
            try {
                const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
                const wsUrl = `${protocol}//${window.location.host}/ws/debug/esp-commands`;

                debugWebSocket = new WebSocket(wsUrl);

                debugWebSocket.onopen = function (event) {
                    console.log('Debug WebSocket connected');
                    debugStatusText.textContent = 'Connected';
                    debugIndicator.className = 'debug-indicator debug-info';
                    reconnectAttempts = 0;
                    isDebugConnected = true;
                };

                debugWebSocket.onmessage = function (event) {
                    try {
                        const cmd = JSON.parse(event.data);

                        // Check for duplicates
                        if (!debugHistory.find(entry =>
                            entry.timestamp === cmd.timestamp &&
                            entry.message === `ESP: ${cmd.command}`
                        )) {
                            addDebugEntry('command', `ESP: ${cmd.command}`, {
                                url: cmd.url,
                                status: cmd.status,
                                response: cmd.response
                            });
                        }
                    } catch (error) {
                        console.error('Error parsing WebSocket message:', error);
                    }
                };

                debugWebSocket.onclose = function (event) {
                    console.log('Debug WebSocket disconnected');
                    debugStatusText.textContent = 'Disconnected';
                    debugIndicator.className = 'debug-indicator';
                    isDebugConnected = false;

                    // Only attempt to reconnect if we should (not manually disconnected)
                    if (shouldReconnect && reconnectAttempts < maxReconnectAttempts) {
                        reconnectAttempts++;
                        console.log(`Attempting to reconnect WebSocket (${reconnectAttempts}/${maxReconnectAttempts})`);
                        setTimeout(connectDebugWebSocket, 2000 * reconnectAttempts);
                    } else if (shouldReconnect) {
                        addDebugEntry('error', 'WebSocket connection lost, falling back to polling');
                        fallbackToPolling();
                    }
                };

                debugWebSocket.onerror = function (error) {
                    console.error('Debug WebSocket error:', error);
                    debugStatusText.textContent = 'Error';
                    debugIndicator.className = 'debug-indicator debug-error';
                };

            } catch (error) {
                console.error('Failed to create WebSocket connection:', error);
                fallbackToPolling();
            }
        }

        // Fallback to polling if WebSocket fails
        function fallbackToPolling() {
            async function fetchESPHistory() {
                try {
                    const response = await fetch('/api/debug/esp-commands');
                    if (response.ok) {
                        const commands = await response.json();

                        // Add any new commands to debug log
                        commands.forEach(cmd => {
                            if (!debugHistory.find(entry =>
                                entry.timestamp === cmd.timestamp &&
                                entry.message === `ESP: ${cmd.command}`
                            )) {
                                addDebugEntry('command', `ESP: ${cmd.command}`, {
                                    url: cmd.url,
                                    status: cmd.status,
                                    response: cmd.response
                                });
                            }
                        });
                    }
                } catch (error) {
                    console.debug('Failed to fetch ESP command history:', error);
                }
            }

            // Poll for ESP command updates every 2 seconds
            setInterval(fetchESPHistory, 2000);

            // Initial fetch
            fetchESPHistory();
        }

        // Disconnect WebSocket function
        function disconnectDebugWebSocket() {
            shouldReconnect = false;
            isDebugConnected = false;
            if (debugWebSocket) {
                debugWebSocket.close();
                debugWebSocket = null;
            }
            debugStatusText.textContent = 'Idle';
            debugIndicator.className = 'debug-indicator';
        }

        // Don't automatically connect - wait for user to show console
    }

    // Initialize debug console
    initializeDebugConsole();

    // Brightness control event handlers
    document.addEventListener('input', function(e) {
        if (e.target.classList.contains('brightness-slider')) {
            const cameraId = e.target.dataset.cameraId;
            const brightness = parseInt(e.target.value);
            const valueSpan = document.getElementById(`brightness-value-${cameraId}`);
            
            // Update display value immediately
            if (valueSpan) {
                valueSpan.textContent = brightness;
            }
            
            // Debounce the API calls
            clearTimeout(e.target.brightnessTimeout);
            e.target.brightnessTimeout = setTimeout(() => {
                setBrightness(cameraId, brightness);
            }, 500);
        }
    });

    // Load initial brightness values for USB cameras
    setTimeout(loadCameraBrightness, 1000);
});

// Brightness control functions
async function setBrightness(cameraId, brightness) {
    try {
        console.log(`Setting brightness for camera ${cameraId} to ${brightness}`);
        
        const response = await fetch(`/api/cameras/${cameraId}/brightness`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({ brightness: brightness })
        });

        if (response.ok) {
            const result = await response.json();
            if (result.success) {
                console.log(`Successfully set brightness for camera ${cameraId}`);
            } else {
                console.error(`Failed to set brightness: ${result.message}`);
                showToast(`Failed to set brightness: ${result.message}`, 'error');
            }
        } else {
            const errorText = await response.text();
            console.error(`HTTP error setting brightness: ${response.status} - ${errorText}`);
            showToast(`Failed to set brightness: ${response.status}`, 'error');
        }
    } catch (error) {
        console.error('Error setting brightness:', error);
        showToast(`Error setting brightness: ${error.message}`, 'error');
    }
}

async function getBrightness(cameraId) {
    try {
        const response = await fetch(`/api/cameras/${cameraId}/brightness`);

        if (response.ok) {
            const result = await response.json();
            if (result.success && result.data) {
                return result.data.brightness;
            } else {
                console.warn(`Failed to get brightness for camera ${cameraId}: ${result.message}`);
                return null;
            }
        } else {
            console.warn(`HTTP error getting brightness for camera ${cameraId}: ${response.status}`);
            return null;
        }
    } catch (error) {
        console.warn(`Error getting brightness for camera ${cameraId}:`, error);
        return null;
    }
}

async function loadCameraBrightness() {
    // Find all USB cameras and load their current brightness
    const brightnessSliders = document.querySelectorAll('.brightness-slider');
    
    for (const slider of brightnessSliders) {
        const cameraId = slider.dataset.cameraId;
        if (cameraId && cameraId.startsWith('usb:')) {
            const currentBrightness = await getBrightness(cameraId);
            if (currentBrightness !== null) {
                slider.value = currentBrightness;
                const valueSpan = document.getElementById(`brightness-value-${cameraId}`);
                if (valueSpan) {
                    valueSpan.textContent = currentBrightness;
                }
                console.log(`Loaded brightness for camera ${cameraId}: ${currentBrightness}`);
            }
        }
    }
}