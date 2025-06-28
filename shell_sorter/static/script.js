document.addEventListener('DOMContentLoaded', function() {
    const startForm = document.getElementById('start-form');
    const stopBtn = document.getElementById('stop-btn');
    
    // Camera management elements
    const detectCamerasBtn = document.getElementById('detect-cameras-btn');
    const startSelectedBtn = document.getElementById('start-selected-btn');
    const stopAllBtn = document.getElementById('stop-all-btn');
    const cameraCheckboxes = document.querySelectorAll('.camera-checkbox');
    
    if (startForm) {
        startForm.addEventListener('submit', async function(e) {
            e.preventDefault();
            
            const formData = new FormData(startForm);
            
            try {
                const response = await fetch('/api/start-sorting', {
                    method: 'POST',
                    body: formData
                });
                
                if (response.ok) {
                    const result = await response.json();
                    alert(result.message);
                    location.reload();
                } else {
                    alert('Error starting sorting job');
                }
            } catch (error) {
                console.error('Error:', error);
                alert('Error starting sorting job');
            }
        });
    }
    
    if (stopBtn) {
        stopBtn.addEventListener('click', async function() {
            if (confirm('Are you sure you want to stop the current sorting job?')) {
                try {
                    const response = await fetch('/api/stop-sorting', {
                        method: 'POST'
                    });
                    
                    if (response.ok) {
                        const result = await response.json();
                        alert(result.message);
                        location.reload();
                    } else {
                        alert('Error stopping sorting job');
                    }
                } catch (error) {
                    console.error('Error:', error);
                    alert('Error stopping sorting job');
                }
            }
        });
    }
    
    // Auto-refresh status every 5 seconds
    setInterval(async function() {
        try {
            const response = await fetch('/api/status');
            if (response.ok) {
                const status = await response.json();
                updateStatusDisplay(status);
            }
        } catch (error) {
            console.error('Error fetching status:', error);
        }
    }, 5000);
    
    // Camera management functions
    if (detectCamerasBtn) {
        detectCamerasBtn.addEventListener('click', async function() {
            try {
                const response = await fetch('/api/cameras/detect');
                if (response.ok) {
                    const cameras = await response.json();
                    alert(`Detected ${cameras.length} cameras`);
                    location.reload();
                } else {
                    alert('Error detecting cameras');
                }
            } catch (error) {
                console.error('Error:', error);
                alert('Error detecting cameras');
            }
        });
    }
    
    if (startSelectedBtn) {
        startSelectedBtn.addEventListener('click', async function() {
            try {
                const response = await fetch('/api/cameras/start-selected', {
                    method: 'POST'
                });
                if (response.ok) {
                    const result = await response.json();
                    alert(result.message);
                    // Update camera feeds without reloading
                    await updateCameraFeeds();
                } else {
                    alert('Error starting selected cameras');
                }
            } catch (error) {
                console.error('Error:', error);
                alert('Error starting selected cameras');
            }
        });
    }
    
    if (stopAllBtn) {
        stopAllBtn.addEventListener('click', async function() {
            if (confirm('Are you sure you want to stop all cameras?')) {
                try {
                    const response = await fetch('/api/cameras/stop-all', {
                        method: 'POST'
                    });
                    if (response.ok) {
                        const result = await response.json();
                        alert(result.message);
                        // Update camera feeds without reloading
                        await updateCameraFeeds();
                    } else {
                        alert('Error stopping cameras');
                    }
                } catch (error) {
                    console.error('Error:', error);
                    alert('Error stopping cameras');
                }
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
                const response = await fetch('/api/cameras/select', {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json',
                    },
                    body: JSON.stringify(selectedCameras)
                });
                
                if (response.ok) {
                    const result = await response.json();
                    console.log('Selection result:', result); // Debug log
                    // Update UI to show selection
                    updateCameraSelection();
                } else {
                    const error = await response.text();
                    console.error('Selection error:', error);
                    alert('Error selecting cameras: ' + error);
                    checkbox.checked = !checkbox.checked; // Revert checkbox
                }
            } catch (error) {
                console.error('Error:', error);
                alert('Error selecting cameras: ' + error.message);
                checkbox.checked = !checkbox.checked; // Revert checkbox
            }
        }
    });
    
    async function updateCameraFeeds() {
        try {
            const response = await fetch('/api/cameras');
            if (response.ok) {
                const cameras = await response.json();
                
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
            }
        } catch (error) {
            console.error('Error updating camera feeds:', error);
        }
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
});