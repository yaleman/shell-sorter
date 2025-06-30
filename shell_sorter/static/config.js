// Configuration page JavaScript
document.addEventListener('DOMContentLoaded', function() {
    // UI Elements
    const backToDashboardBtn = document.getElementById('back-to-dashboard-btn');
    const autoStartCamerasCheckbox = document.getElementById('auto-start-cameras');
    const autoDetectCamerasCheckbox = document.getElementById('auto-detect-cameras');
    const esphomeHostnameInput = document.getElementById('esphome-hostname');
    const networkCamerasList = document.getElementById('network-cameras-list');
    const addNetworkCameraBtn = document.getElementById('add-network-camera-btn');
    const refreshCamerasBtn = document.getElementById('refresh-cameras-btn');
    const clearAllCamerasBtn = document.getElementById('clear-all-cameras-btn');
    const saveConfigBtn = document.getElementById('save-config-btn');
    const resetConfigBtn = document.getElementById('reset-config-btn');
    const camerasConfigList = document.getElementById('cameras-config-list');

    // Configuration state
    let configData = {
        auto_start_cameras: false,
        auto_detect_cameras: false,
        esphome_hostname: 'shell-sorter-controller.local',
        network_camera_hostnames: ['esp32cam1.local'],
        cameras: []
    };

    // Navigation
    if (backToDashboardBtn) {
        backToDashboardBtn.addEventListener('click', function() {
            window.location.href = '/';
        });
    }

    // Load configuration on page load
    loadConfiguration();

    // Auto-start cameras checkbox
    if (autoStartCamerasCheckbox) {
        autoStartCamerasCheckbox.addEventListener('change', function() {
            configData.auto_start_cameras = this.checked;
            console.log('Auto-start cameras:', configData.auto_start_cameras);
        });
    }

    // Auto-detect cameras checkbox
    if (autoDetectCamerasCheckbox) {
        autoDetectCamerasCheckbox.addEventListener('change', function() {
            configData.auto_detect_cameras = this.checked;
            console.log('Auto-detect cameras:', configData.auto_detect_cameras);
        });
    }

    // ESPHome hostname input
    if (esphomeHostnameInput) {
        esphomeHostnameInput.addEventListener('change', function() {
            configData.esphome_hostname = this.value.trim();
            console.log('ESPHome hostname:', configData.esphome_hostname);
        });
    }

    // Add network camera button
    if (addNetworkCameraBtn) {
        addNetworkCameraBtn.addEventListener('click', function() {
            addNetworkCameraItem('');
        });
    }

    // Refresh cameras button
    if (refreshCamerasBtn) {
        refreshCamerasBtn.addEventListener('click', async function() {
            await refreshCameraList();
        });
    }

    // Clear all cameras button
    if (clearAllCamerasBtn) {
        clearAllCamerasBtn.addEventListener('click', async function() {
            if (confirm('Are you sure you want to remove all cameras from the configuration? This will clear all camera settings including regions and view types.')) {
                await clearAllCameras();
            }
        });
    }

    // Save configuration button
    if (saveConfigBtn) {
        saveConfigBtn.addEventListener('click', async function() {
            await saveConfiguration();
        });
    }

    // Reset configuration button
    if (resetConfigBtn) {
        resetConfigBtn.addEventListener('click', async function() {
            if (confirm('Are you sure you want to reset all configuration to defaults? This will clear all settings.')) {
                await resetConfiguration();
            }
        });
    }

    // Handle camera deletion and network camera actions using event delegation
    document.addEventListener('click', async function(e) {
        if (e.target.classList.contains('delete-camera-btn')) {
            const cameraIndex = parseInt(e.target.dataset.cameraIndex);
            const cameraItem = e.target.closest('.camera-config-item');
            const cameraName = cameraItem.querySelector('h3').textContent;
            
            if (confirm(`Are you sure you want to delete camera "${cameraName}"? This will remove all settings for this camera including regions and view type.`)) {
                await deleteCamera(cameraIndex);
            }
        } else if (e.target.classList.contains('remove-network-camera-btn')) {
            const networkCameraItem = e.target.closest('.network-camera-item');
            networkCameraItem.remove();
            updateNetworkCamerasFromUI();
        }
    });

    // Handle network camera hostname changes using event delegation
    document.addEventListener('input', function(e) {
        if (e.target.classList.contains('network-camera-hostname')) {
            updateNetworkCamerasFromUI();
        }
    });

    async function loadConfiguration() {
        try {
            const response = await fetch('/api/config');
            if (response.ok) {
                configData = await response.json();
                updateUI();
                console.log('Loaded configuration:', configData);
            } else {
                showToast('Failed to load configuration', 'error');
            }
        } catch (error) {
            console.error('Error loading configuration:', error);
            showToast('Error loading configuration: ' + error.message, 'error');
        }
    }

    async function saveConfiguration() {
        try {
            const response = await fetch('/api/config', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify(configData)
            });

            if (response.ok) {
                showToast('Configuration saved successfully', 'success');
                console.log('Saved configuration:', configData);
            } else {
                const error = await response.text();
                showToast('Failed to save configuration: ' + error, 'error');
            }
        } catch (error) {
            console.error('Error saving configuration:', error);
            showToast('Error saving configuration: ' + error.message, 'error');
        }
    }

    async function refreshCameraList() {
        try {
            showToast('Refreshing camera list...', 'info');
            
            const response = await fetch('/api/cameras');
            if (response.ok) {
                const cameras = await response.json();
                configData.cameras = cameras;
                updateCamerasList();
                showToast(`Refreshed camera list - found ${cameras.length} cameras`, 'success');
            } else {
                showToast('Failed to refresh camera list', 'error');
            }
        } catch (error) {
            console.error('Error refreshing cameras:', error);
            showToast('Error refreshing cameras: ' + error.message, 'error');
        }
    }

    async function deleteCamera(cameraIndex) {
        try {
            const response = await fetch(`/api/config/cameras/${cameraIndex}`, {
                method: 'DELETE'
            });

            if (response.ok) {
                // Remove from local config
                configData.cameras = configData.cameras.filter(camera => camera.index !== cameraIndex);
                updateCamerasList();
                showToast('Camera deleted successfully', 'success');
            } else {
                const error = await response.text();
                showToast('Failed to delete camera: ' + error, 'error');
            }
        } catch (error) {
            console.error('Error deleting camera:', error);
            showToast('Error deleting camera: ' + error.message, 'error');
        }
    }

    async function clearAllCameras() {
        try {
            const response = await fetch('/api/config/cameras', {
                method: 'DELETE'
            });

            if (response.ok) {
                configData.cameras = [];
                updateCamerasList();
                showToast('All cameras cleared successfully', 'success');
            } else {
                const error = await response.text();
                showToast('Failed to clear cameras: ' + error, 'error');
            }
        } catch (error) {
            console.error('Error clearing cameras:', error);
            showToast('Error clearing cameras: ' + error.message, 'error');
        }
    }

    async function resetConfiguration() {
        try {
            const response = await fetch('/api/config/reset', {
                method: 'POST'
            });

            if (response.ok) {
                await loadConfiguration(); // Reload from server
                showToast('Configuration reset to defaults', 'success');
            } else {
                const error = await response.text();
                showToast('Failed to reset configuration: ' + error, 'error');
            }
        } catch (error) {
            console.error('Error resetting configuration:', error);
            showToast('Error resetting configuration: ' + error.message, 'error');
        }
    }

    function updateUI() {
        // Update auto-start checkbox
        if (autoStartCamerasCheckbox) {
            autoStartCamerasCheckbox.checked = configData.auto_start_cameras || false;
        }
        
        // Update auto-detect checkbox
        if (autoDetectCamerasCheckbox) {
            autoDetectCamerasCheckbox.checked = configData.auto_detect_cameras || false;
        }
        
        // Update ESPHome hostname input
        if (esphomeHostnameInput) {
            esphomeHostnameInput.value = configData.esphome_hostname || 'shell-sorter-controller.local';
        }

        // Update network cameras list
        updateNetworkCamerasList();

        // Update cameras list
        updateCamerasList();
    }

    function addNetworkCameraItem(hostname) {
        if (!networkCamerasList) return;

        const networkCameraItem = document.createElement('div');
        networkCameraItem.className = 'network-camera-item';
        networkCameraItem.innerHTML = `
            <input type="text" class="network-camera-hostname" value="${hostname}" placeholder="esp32cam1.local">
            <button type="button" class="btn btn-sm btn-danger remove-network-camera-btn">Remove</button>
        `;
        
        networkCamerasList.appendChild(networkCameraItem);
        
        // Update configuration immediately
        updateNetworkCamerasFromUI();
    }

    function updateNetworkCamerasList() {
        if (!networkCamerasList) return;

        // Clear existing items
        networkCamerasList.innerHTML = '';

        // Add items from configuration
        const hostnames = configData.network_camera_hostnames || ['esp32cam1.local'];
        hostnames.forEach(hostname => {
            addNetworkCameraItem(hostname);
        });
    }

    function updateNetworkCamerasFromUI() {
        if (!networkCamerasList) return;

        const hostnameInputs = networkCamerasList.querySelectorAll('.network-camera-hostname');
        const hostnames = Array.from(hostnameInputs)
            .map(input => input.value.trim())
            .filter(hostname => hostname.length > 0);
        
        configData.network_camera_hostnames = hostnames;
        console.log('Network camera hostnames:', configData.network_camera_hostnames);
    }

    function updateCamerasList() {
        if (!camerasConfigList) return;

        if (!configData.cameras || configData.cameras.length === 0) {
            camerasConfigList.innerHTML = `
                <div class="no-cameras-config">
                    <p>No cameras configured. Use "Detect Cameras" from the dashboard to add cameras.</p>
                </div>
            `;
            return;
        }

        const camerasHTML = configData.cameras.map(camera => `
            <div class="camera-config-item" data-camera-index="${camera.index}">
                <div class="camera-config-info">
                    <div class="camera-config-header">
                        <h3>${camera.name}</h3>
                        <span class="camera-status status-${camera.is_active ? 'active' : 'inactive'}">
                            ${camera.is_active ? 'Active' : 'Inactive'}
                        </span>
                    </div>
                    <div class="camera-config-details">
                        <span class="camera-detail">Index: ${camera.index}</span>
                        <span class="camera-detail">Resolution: ${camera.resolution}</span>
                        ${camera.view_type ? `<span class="camera-detail">View: ${camera.view_type}</span>` : ''}
                        ${camera.region_x !== null && camera.region_x !== undefined ? 
                            `<span class="camera-detail">Region: ${camera.region_x},${camera.region_y} (${camera.region_width}x${camera.region_height})</span>` : 
                            ''
                        }
                    </div>
                </div>
                <div class="camera-config-actions">
                    <button class="btn btn-sm btn-danger delete-camera-btn" data-camera-index="${camera.index}">
                        Delete
                    </button>
                </div>
            </div>
        `).join('');

        camerasConfigList.innerHTML = camerasHTML;
    }
});