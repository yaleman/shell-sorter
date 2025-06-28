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

document.addEventListener('DOMContentLoaded', function() {
    // Camera management elements
    const detectCamerasBtn = document.getElementById('detect-cameras-btn');
    const startSelectedBtn = document.getElementById('start-selected-btn');
    const stopAllBtn = document.getElementById('stop-all-btn');
    const captureImagesBtn = document.getElementById('capture-images-btn');
    const nextCaseBtn = document.getElementById('next-case-btn');
    const mlTrainingBtn = document.getElementById('ml-training-btn');

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

    // Auto-refresh status every 5 seconds
    setInterval(async function() {
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

    // Camera management functions
    if (detectCamerasBtn) {
        detectCamerasBtn.addEventListener('click', async function() {
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

    // Handle camera selection using event delegation
    document.addEventListener('change', async function(e) {
        if (e.target.classList.contains('camera-checkbox')) {
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
                            feedDiv.innerHTML = `<img src="/api/cameras/${camera.index}/stream" 
                                                     alt="Camera ${camera.index} feed"
                                                     class="camera-stream">`;
                            cameraItem.appendChild(feedDiv);
                        } else if (!camera.is_active && existingFeed) {
                            existingFeed.remove();
                        }
                    }
                });
                return cameras;
            }
        } catch (error) {
            console.error('Error updating camera feeds:', error);
        }
        return null;
    }
    
    function pollForCameraUpdates() {
        console.log('Starting camera status polling...');
        let pollCount = 0;
        const maxPolls = 12; // Poll for up to 60 seconds (12 * 5s)
        
        const pollInterval = setInterval(async () => {
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
                    clearInterval(pollInterval);
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

    // Initialize camera selection display
    updateCameraSelection();
    
    // Auto-detect cameras if none are currently detected
    autoDetectCameras();
    
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
                        }
                    } else {
                        console.log('Auto-detection failed');
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