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

document.addEventListener('DOMContentLoaded', function() {
    // Camera management elements
    const detectCamerasBtn = document.getElementById('detect-cameras-btn');
    const startSelectedBtn = document.getElementById('start-selected-btn');
    const stopAllBtn = document.getElementById('stop-all-btn');
    const captureImagesBtn = document.getElementById('capture-images-btn');
    const nextCaseBtn = document.getElementById('next-case-btn');
    const mlTrainingBtn = document.getElementById('ml-training-btn');
    const configBtn = document.getElementById('config-btn');

    if (captureImagesBtn) {
        captureImagesBtn.addEventListener('click', async function() {
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
        nextCaseBtn.addEventListener('click', async function() {
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
        mlTrainingBtn.addEventListener('click', function() {
            window.location.href = '/ml-training';
        });
    }

    if (configBtn) {
        configBtn.addEventListener('click', function() {
            window.location.href = '/config';
        });
    }

    // Auto-refresh status every 25 seconds
    let statusInterval = setInterval(async function() {
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
    
    // ESPHome status monitoring
    async function updateESPHomeStatus() {
        try {
            const response = await fetch('/api/machine/esphome-status');
            if (response.ok) {
                const status = await response.json();
                const statusElement = document.getElementById('esphome-status');
                const statusText = document.getElementById('esphome-status-text');
                
                if (statusElement && statusText) {
                    // Remove all status classes
                    statusElement.className = 'status-indicator';
                    
                    if (status.online) {
                        statusElement.classList.add('esphome-status-online');
                        statusText.textContent = 'Online';
                    } else {
                        statusElement.classList.add('esphome-status-offline');
                        statusText.textContent = 'Offline';
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
        }
    }
    
    // Initial ESPHome status check
    updateESPHomeStatus();
    
    // Auto-refresh ESPHome status every 30 seconds
    const esphomeStatusInterval = setInterval(updateESPHomeStatus, 30000);
    
    // Clean up intervals when page unloads
    window.addEventListener('beforeunload', function() {
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
        detectCamerasBtn.addEventListener('click', async function() {
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
                    const cameras = await response.json();
                    showToast(`Detected ${cameras.length} cameras`, 'success');
                    location.reload();
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
        startSelectedBtn.addEventListener('click', async function() {
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
        stopAllBtn.addEventListener('click', async function() {
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
    document.addEventListener('change', async function(e) {
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
                .map(cb => parseInt(cb.dataset.cameraIndex));

            console.log('Selected cameras:', selectedCameras); // Debug log

            try {
                const controller = new AbortController();
                const timeoutId = setTimeout(() => controller.abort(), 2500);

                const response = await fetch('/api/cameras/select', {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json',
                    },
                    body: JSON.stringify(selectedCameras),
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
    document.addEventListener('click', async function(e) {
        if (e.target.classList.contains('region-select-btn')) {
            const cameraIndex = parseInt(e.target.dataset.cameraIndex);
            window.location.href = `/region-selection/${cameraIndex}`;
        }
        else if (e.target.classList.contains('region-clear-btn')) {
            const cameraIndex = parseInt(e.target.dataset.cameraIndex);
            
            try {
                const response = await fetch(`/api/cameras/${cameraIndex}/region`, {
                    method: 'DELETE'
                });

                if (response.ok) {
                    const result = await response.json();
                    showToast(result.message, 'success');
                    setTimeout(() => location.reload(), 500);
                } else {
                    const error = await response.text();
                    showToast('Error clearing region: ' + error, 'error');
                }
            } catch (error) {
                console.error('Error:', error);
                showToast('Error clearing region: ' + error.message, 'error');
            }
        }
    });

    async function updateCameraFeeds() {
        try {
            const controller = new AbortController();
            const timeoutId = setTimeout(() => controller.abort(), 2500);

            const response = await fetch('/api/cameras', {
                signal: controller.signal
            });

            clearTimeout(timeoutId);

            if (response.ok) {
                const cameras = await response.json();
                console.debug('Updating camera feeds:', cameras); // Debug log
                cameras.forEach(camera => {
                    const cameraItem = document.querySelector(`[data-camera-index="${camera.index}"]`);
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
                            feedDiv.dataset.cameraIndex = camera.index.toString();
                            
                            // Add region data if available
                            if (camera.region_x !== null && camera.region_x !== undefined) {
                                feedDiv.dataset.region = JSON.stringify({
                                    x: camera.region_x,
                                    y: camera.region_y,
                                    width: camera.region_width,
                                    height: camera.region_height
                                });
                            }
                            
                            feedDiv.innerHTML = `<img src="/api/cameras/${camera.index}/stream" 
                                                     alt="Camera ${camera.index} feed"
                                                     class="camera-stream">`;
                            cameraItem.appendChild(feedDiv);
                        } else if (!camera.is_active && existingFeed) {
                            existingFeed.remove();
                        }
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
                const cameras = await response.json();
                
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
            // Check if we already have cameras detected
            const response = await fetch('/api/cameras', {
                signal: AbortSignal.timeout(2500)
            });
            
            if (response.ok) {
                const cameras = await response.json();
                
                // If no cameras are detected, automatically trigger detection
                if (cameras.length === 0) {
                    console.log('No cameras detected, auto-detecting...');
                    showToast('No cameras found, automatically detecting...', 'info');
                    
                    const detectResponse = await fetch('/api/cameras/detect', {
                        signal: AbortSignal.timeout(10000)
                    });
                    
                    if (detectResponse.ok) {
                        const detectedCameras = await detectResponse.json();
                        console.log(`Auto-detected ${detectedCameras.length} cameras`);
                        
                        if (detectedCameras.length > 0) {
                            showToast(`Auto-detected ${detectedCameras.length} cameras`, 'success');
                            // Reload to show the detected cameras
                            setTimeout(() => location.reload(), 1000);
                        } else {
                            console.log('No cameras found during auto-detection');
                            showToast('No cameras found on this system', 'warning');
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
});