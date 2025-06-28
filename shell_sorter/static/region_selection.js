// Region selection functionality
document.addEventListener('DOMContentLoaded', function() {
    const cameraImage = document.getElementById('camera-image');
    const regionOverlay = document.getElementById('region-overlay');
    const saveRegionBtn = document.getElementById('save-region-btn');
    const clearRegionBtn = document.getElementById('clear-region-btn');
    const resetSelectionBtn = document.getElementById('reset-selection-btn');
    const startCameraBtn = document.getElementById('start-camera-btn');
    
    const startCoordsSpan = document.getElementById('start-coords');
    const endCoordsSpan = document.getElementById('end-coords');
    const sizeCoordsSpan = document.getElementById('size-coords');
    
    let isSelecting = false;
    let startX = 0;
    let startY = 0;
    let currentSelection = null;
    let cameraIndex = null;
    
    // Extract camera index from URL
    const pathParts = window.location.pathname.split('/');
    if (pathParts.length >= 3 && pathParts[1] === 'region-selection') {
        cameraIndex = parseInt(pathParts[2]);
    }
    
    if (!cameraImage) {
        console.log('Camera image not found - camera may not be active');
        return;
    }
    
    function getImageCoordinates(event) {
        const rect = cameraImage.getBoundingClientRect();
        const scaleX = cameraImage.naturalWidth / cameraImage.clientWidth;
        const scaleY = cameraImage.naturalHeight / cameraImage.clientHeight;
        
        const x = Math.round((event.clientX - rect.left) * scaleX);
        const y = Math.round((event.clientY - rect.top) * scaleY);
        
        return { x, y };
    }
    
    function updateOverlay(x1, y1, x2, y2) {
        if (!regionOverlay) return;
        
        const rect = cameraImage.getBoundingClientRect();
        const scaleX = cameraImage.clientWidth / cameraImage.naturalWidth;
        const scaleY = cameraImage.clientHeight / cameraImage.naturalHeight;
        
        const left = Math.min(x1, x2) * scaleX;
        const top = Math.min(y1, y2) * scaleY;
        const width = Math.abs(x2 - x1) * scaleX;
        const height = Math.abs(y2 - y1) * scaleY;
        
        regionOverlay.style.left = left + 'px';
        regionOverlay.style.top = top + 'px';
        regionOverlay.style.width = width + 'px';
        regionOverlay.style.height = height + 'px';
        regionOverlay.style.display = 'block';
    }
    
    function updateCoordinatesDisplay(x1, y1, x2, y2) {
        const minX = Math.min(x1, x2);
        const minY = Math.min(y1, y2);
        const maxX = Math.max(x1, x2);
        const maxY = Math.max(y1, y2);
        const width = maxX - minX;
        const height = maxY - minY;
        
        startCoordsSpan.textContent = `(${minX}, ${minY})`;
        endCoordsSpan.textContent = `(${maxX}, ${maxY})`;
        sizeCoordsSpan.textContent = `${width} × ${height}`;
        
        currentSelection = { x: minX, y: minY, width, height };
        saveRegionBtn.disabled = false;
    }
    
    function clearSelection() {
        if (regionOverlay) {
            regionOverlay.style.display = 'none';
        }
        startCoordsSpan.textContent = 'Not selected';
        endCoordsSpan.textContent = 'Not selected';
        sizeCoordsSpan.textContent = 'Not selected';
        currentSelection = null;
        saveRegionBtn.disabled = true;
    }
    
    // Mouse events for region selection
    if (cameraImage) {
        cameraImage.addEventListener('mousedown', function(event) {
            if (event.button !== 0) return; // Only left mouse button
            
            isSelecting = true;
            const coords = getImageCoordinates(event);
            startX = coords.x;
            startY = coords.y;
            
            cameraImage.style.cursor = 'crosshair';
            event.preventDefault();
        });
        
        cameraImage.addEventListener('mousemove', function(event) {
            if (!isSelecting) return;
            
            const coords = getImageCoordinates(event);
            updateOverlay(startX, startY, coords.x, coords.y);
            updateCoordinatesDisplay(startX, startY, coords.x, coords.y);
            
            event.preventDefault();
        });
        
        cameraImage.addEventListener('mouseup', function(event) {
            if (!isSelecting) return;
            
            isSelecting = false;
            cameraImage.style.cursor = 'crosshair';
            
            const coords = getImageCoordinates(event);
            updateCoordinatesDisplay(startX, startY, coords.x, coords.y);
            
            event.preventDefault();
        });
        
        // Prevent context menu
        cameraImage.addEventListener('contextmenu', function(event) {
            event.preventDefault();
        });
        
        // Handle mouse leave to stop selection
        cameraImage.addEventListener('mouseleave', function() {
            if (isSelecting) {
                isSelecting = false;
                cameraImage.style.cursor = 'crosshair';
            }
        });
    }
    
    // Save region button
    if (saveRegionBtn) {
        saveRegionBtn.addEventListener('click', async function() {
            if (!currentSelection || cameraIndex === null || cameraIndex === undefined) {
                showToast('No region selected or camera index not found', 'error');
                return;
            }
            
            try {
                const formData = new FormData();
                formData.append('x', currentSelection.x.toString());
                formData.append('y', currentSelection.y.toString());
                formData.append('width', currentSelection.width.toString());
                formData.append('height', currentSelection.height.toString());
                
                const response = await fetch(`/api/cameras/${cameraIndex}/region`, {
                    method: 'POST',
                    body: formData
                });
                
                if (response.ok) {
                    const result = await response.json();
                    showToast(result.message, 'success');
                    
                    // Navigate back to dashboard after successful save
                    setTimeout(() => {
                        window.location.href = '/';
                    }, 1000);
                } else {
                    const error = await response.text();
                    showToast('Error saving region: ' + error, 'error');
                }
            } catch (error) {
                console.error('Error:', error);
                showToast('Error saving region: ' + error.message, 'error');
            }
        });
    }
    
    // Clear region button
    if (clearRegionBtn) {
        clearRegionBtn.addEventListener('click', async function() {
            if (cameraIndex === null || cameraIndex === undefined) {
                showToast('Camera index not found', 'error');
                return;
            }
            
            try {
                const response = await fetch(`/api/cameras/${cameraIndex}/region`, {
                    method: 'DELETE'
                });
                
                if (response.ok) {
                    const result = await response.json();
                    showToast(result.message, 'success');
                    
                    // Clear current selection and reload
                    clearSelection();
                    setTimeout(() => {
                        location.reload();
                    }, 1000);
                } else {
                    const error = await response.text();
                    showToast('Error clearing region: ' + error, 'error');
                }
            } catch (error) {
                console.error('Error:', error);
                showToast('Error clearing region: ' + error.message, 'error');
            }
        });
    }
    
    // Reset selection button
    if (resetSelectionBtn) {
        resetSelectionBtn.addEventListener('click', function() {
            clearSelection();
            showToast('Selection cleared', 'info');
        });
    }
    
    // Start camera button
    if (startCameraBtn) {
        startCameraBtn.addEventListener('click', async function() {
            if (cameraIndex === null || cameraIndex === undefined) {
                showToast('Camera index not found', 'error');
                return;
            }
            
            try {
                const response = await fetch(`/api/cameras/${cameraIndex}/start`, {
                    method: 'POST'
                });
                
                if (response.ok) {
                    const result = await response.json();
                    showToast(result.message, 'success');
                    
                    // Reload page after camera starts
                    setTimeout(() => {
                        location.reload();
                    }, 2000);
                } else {
                    const error = await response.text();
                    showToast('Error starting camera: ' + error, 'error');
                }
            } catch (error) {
                console.error('Error:', error);
                showToast('Error starting camera: ' + error.message, 'error');
            }
        });
    }
    
    // Handle image load to ensure proper dimensions
    if (cameraImage) {
        cameraImage.addEventListener('load', function() {
            console.log('Camera image loaded:', {
                naturalWidth: cameraImage.naturalWidth,
                naturalHeight: cameraImage.naturalHeight,
                clientWidth: cameraImage.clientWidth,
                clientHeight: cameraImage.clientHeight
            });
        });
        
        // If image is already loaded
        if (cameraImage.complete) {
            console.log('Camera image already loaded:', {
                naturalWidth: cameraImage.naturalWidth,
                naturalHeight: cameraImage.naturalHeight,
                clientWidth: cameraImage.clientWidth,
                clientHeight: cameraImage.clientHeight
            });
        }
    }
    
    console.log('Region selection initialized for camera', cameraIndex);
});