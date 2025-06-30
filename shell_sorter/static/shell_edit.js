// Shell Edit Interface JavaScript

class ShellEditInterface {
    constructor() {
        this.shell = window.shellData;
        this.sessionId = window.sessionId;
        this.allShells = []; // Will be loaded for dropdown population
        this.initializeEventListeners();
        this.loadAllShells();
        this.renderShellData();
    }

    initializeEventListeners() {
        // Save button
        document.getElementById('save-shell-changes').addEventListener('click', () => this.saveShellChanges());
        
        // Delete button
        document.getElementById('delete-shell-btn').addEventListener('click', () => this.deleteShell());
        
        // Regenerate composite button
        document.querySelectorAll('.regenerate-composite-btn').forEach(btn => {
            btn.addEventListener('click', (e) => {
                const sessionId = e.target.dataset.sessionId;
                this.regenerateComposite(sessionId);
            });
        });

        // Handle shell type dropdown
        const shellTypeSelect = document.getElementById('edit-shell-type');
        const shellTypeCustom = document.getElementById('edit-shell-type-custom');
        
        if (shellTypeSelect && shellTypeCustom) {
            shellTypeSelect.addEventListener('change', (e) => {
                if (e.target.value === '__custom__') {
                    shellTypeCustom.style.display = 'block';
                    shellTypeCustom.focus();
                } else {
                    shellTypeCustom.style.display = 'none';
                    shellTypeCustom.value = '';
                }
            });
        }
    }

    async loadAllShells() {
        try {
            const response = await fetch('/api/ml/shells');
            if (response.ok) {
                const data = await response.json();
                this.allShells = data.shells;
                this.populateShellTypeDropdown();
            }
        } catch (error) {
            console.error('Error loading shells for dropdown:', error);
        }
    }

    populateShellTypeDropdown() {
        const shellTypes = [...new Set(this.allShells.map(s => s.shell_type))].sort();
        const shellTypeSelect = document.getElementById('edit-shell-type');
        
        // Clear existing options except first and last
        const selectOptions = Array.from(shellTypeSelect.options);
        selectOptions.slice(1, -1).forEach(option => option.remove());
        
        // Add shell type options
        shellTypes.forEach(type => {
            const option = document.createElement('option');
            option.value = type;
            option.textContent = type;
            option.selected = type === this.shell.shell_type;
            shellTypeSelect.insertBefore(option, shellTypeSelect.lastElementChild);
        });
    }

    renderShellData() {
        // Use captured_images if available, otherwise fall back to image_filenames
        const images = this.shell.captured_images && this.shell.captured_images.length > 0 
            ? this.shell.captured_images 
            : this.shell.image_filenames.map(filename => ({ filename, view_type: null }));

        // Sort images: side views first, then tail views, then unknown/unspecified
        const sortedImages = [...images].sort((a, b) => {
            const viewOrder = { 'side': 0, 'tail': 1, 'unknown': 2, null: 3, undefined: 3 };
            return viewOrder[a.view_type] - viewOrder[b.view_type];
        });

        const editImagesContainer = document.getElementById('edit-images');
        editImagesContainer.innerHTML = sortedImages.map((img, index) => `
            <div class="edit-image-item" data-image-index="${index}">
                <div class="edit-image-container">
                    <img src="/images/${img.filename}" alt="Shell image" class="edit-image" id="edit-image-${index}">
                    <div class="region-overlay-container" id="region-overlay-container-${index}">
                        ${img.region_x !== null && img.region_x !== undefined ? 
                            `<div class="region-overlay-edit" id="region-overlay-${index}" 
                                  data-region-x="${img.region_x}" 
                                  data-region-y="${img.region_y}" 
                                  data-region-width="${img.region_width}" 
                                  data-region-height="${img.region_height}">
                             </div>` : ''
                        }
                    </div>
                </div>
                <div class="edit-image-controls">
                    <div class="control-row">
                        <label>View Type:</label>
                        <select class="edit-view-type" data-filename="${img.filename}">
                            <option value="unknown" ${(img.view_type || 'unknown') === 'unknown' ? 'selected' : ''}>Unknown</option>
                            <option value="side" ${img.view_type === 'side' ? 'selected' : ''}>Side View</option>
                            <option value="tail" ${img.view_type === 'tail' ? 'selected' : ''}>Tail View</option>
                        </select>
                    </div>
                    <div class="control-row">
                        <button class="btn btn-sm btn-primary edit-region-btn" data-image-index="${index}" data-filename="${img.filename}">
                            ${img.region_x !== null && img.region_x !== undefined ? 'Edit Region' : 'Select Region'}
                        </button>
                        ${img.region_x !== null && img.region_x !== undefined ? 
                            `<button class="btn btn-sm btn-warning clear-region-btn" data-image-index="${index}" data-filename="${img.filename}">Clear Region</button>` 
                            : ''
                        }
                        <button class="btn btn-sm btn-danger delete-image-btn" data-filename="${img.filename}">Delete Image</button>
                    </div>
                    ${img.region_x !== null && img.region_x !== undefined ? 
                        `<div class="region-info">
                            Region: ${img.region_x},${img.region_y} (${img.region_width}x${img.region_height})
                         </div>` : ''
                    }
                </div>
            </div>
        `).join('');

        // Add event listeners for the dynamically created elements
        this.addImageEventListeners();
        
        // Initialize region overlays
        setTimeout(() => this.initializeRegionOverlays(), 100);
    }

    addImageEventListeners() {
        // Handle region editing
        document.querySelectorAll('.edit-region-btn').forEach(btn => {
            btn.addEventListener('click', (e) => {
                const imageIndex = parseInt(e.target.dataset.imageIndex);
                const filename = e.target.dataset.filename;
                this.startRegionEdit(imageIndex, filename);
            });
        });
        
        // Handle region clearing
        document.querySelectorAll('.clear-region-btn').forEach(btn => {
            btn.addEventListener('click', (e) => {
                const imageIndex = parseInt(e.target.dataset.imageIndex);
                const filename = e.target.dataset.filename;
                this.clearRegionFromImage(imageIndex, filename);
            });
        });
        
        // Handle image deletion
        document.querySelectorAll('.delete-image-btn').forEach(btn => {
            btn.addEventListener('click', (e) => {
                const filename = e.target.dataset.filename;
                this.deleteImageFromShell(filename);
            });
        });
    }

    async saveShellChanges() {
        try {
            const brand = document.getElementById('edit-brand').value.trim();
            const shellTypeSelect = document.getElementById('edit-shell-type').value;
            const shellTypeCustom = document.getElementById('edit-shell-type-custom').value.trim();
            const include = document.getElementById('edit-include').checked;

            // Determine the actual shell type
            let shellType = '';
            if (shellTypeSelect === '__custom__') {
                shellType = shellTypeCustom;
            } else if (shellTypeSelect) {
                shellType = shellTypeSelect;
            }

            if (!brand || !shellType) {
                this.showToast('Brand and shell type are required', 'error');
                return;
            }

            // Collect view type changes
            const viewTypeUpdates = [];
            document.querySelectorAll('.edit-view-type').forEach(select => {
                viewTypeUpdates.push({
                    filename: select.dataset.filename,
                    view_type: select.value
                });
            });

            // Save basic shell data
            const response = await fetch(`/api/ml/shells/${this.sessionId}/update`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify({
                    brand: brand,
                    shell_type: shellType,
                    include: include,
                    view_type_updates: viewTypeUpdates
                })
            });

            if (response.ok) {
                this.showToast('Shell updated successfully', 'success');
                // Update the page title and header
                document.title = `Edit Shell: ${brand} ${shellType} - Shell Sorter`;
                document.querySelector('header h1').textContent = `✏️ Edit Shell: ${brand} ${shellType}`;
                // Update local shell data
                this.shell.brand = brand;
                this.shell.shell_type = shellType;
                this.shell.include = include;
            } else {
                throw new Error(`Failed to update shell: ${response.statusText}`);
            }
        } catch (error) {
            console.error('Error saving shell changes:', error);
            this.showToast('Error saving changes: ' + error.message, 'error');
        }
    }

    async deleteShell() {
        if (!confirm(`Are you sure you want to delete the shell "${this.shell.brand} ${this.shell.shell_type}" and all its images? This action cannot be undone.`)) {
            return;
        }

        try {
            const response = await fetch(`/api/ml/shells/${this.sessionId}`, {
                method: 'DELETE'
            });

            if (response.ok) {
                this.showToast('Shell deleted successfully', 'success');
                // Redirect back to ML training page after a short delay
                setTimeout(() => {
                    window.location.href = '/ml-training';
                }, 1500);
            } else {
                throw new Error(`Failed to delete shell: ${response.statusText}`);
            }
        } catch (error) {
            console.error('Error deleting shell:', error);
            this.showToast('Error deleting shell: ' + error.message, 'error');
        }
    }

    async deleteImageFromShell(filename) {
        if (!confirm(`Are you sure you want to delete the image "${filename}"? This action cannot be undone.`)) {
            return;
        }

        try {
            const response = await fetch(`/api/ml/shells/${this.sessionId}/images/${filename}`, {
                method: 'DELETE'
            });

            if (response.ok) {
                this.showToast('Image deleted successfully', 'success');
                // Re-render the images
                await this.reloadShellData();
            } else {
                throw new Error(`Failed to delete image: ${response.statusText}`);
            }
        } catch (error) {
            console.error('Error deleting image:', error);
            this.showToast('Error deleting image: ' + error.message, 'error');
        }
    }

    async reloadShellData() {
        try {
            const response = await fetch(`/api/ml/shells`);
            if (response.ok) {
                const data = await response.json();
                this.shell = data.shells.find(s => s.session_id === this.sessionId);
                if (this.shell) {
                    this.renderShellData();
                }
            }
        } catch (error) {
            console.error('Error reloading shell data:', error);
        }
    }

    async regenerateComposite(sessionId) {
        try {
            this.showToast('Regenerating composite image...', 'info');
            
            const response = await fetch(`/api/ml/shells/${sessionId}/composite`, {
                method: 'POST'
            });

            if (response.ok) {
                this.showToast('Composite regenerated successfully', 'success');
                
                // Update the composite image by adding a cache-busting parameter
                const compositeImg = document.getElementById(`composite-image-${sessionId}`);
                if (compositeImg) {
                    const timestamp = new Date().getTime();
                    compositeImg.src = `/api/composites/${sessionId}?t=${timestamp}`;
                    compositeImg.style.display = 'block';
                    compositeImg.nextElementSibling.style.display = 'none';
                }
            } else {
                throw new Error(`Failed to regenerate composite: ${response.statusText}`);
            }
        } catch (error) {
            console.error('Error regenerating composite:', error);
            this.showToast('Error regenerating composite: ' + error.message, 'error');
        }
    }

    // Region editing methods (copied from ml_training.js)
    initializeRegionOverlays() {
        document.querySelectorAll('.region-overlay-edit').forEach(overlay => {
            const regionX = parseInt(overlay.dataset.regionX);
            const regionY = parseInt(overlay.dataset.regionY);
            const regionWidth = parseInt(overlay.dataset.regionWidth);
            const regionHeight = parseInt(overlay.dataset.regionHeight);
            
            const container = overlay.closest('.edit-image-container');
            const image = container.querySelector('.edit-image');
            
            if (image && image.complete) {
                this.updateRegionOverlay(overlay, image, regionX, regionY, regionWidth, regionHeight);
            } else if (image) {
                image.addEventListener('load', () => {
                    this.updateRegionOverlay(overlay, image, regionX, regionY, regionWidth, regionHeight);
                });
            }
        });
    }

    updateRegionOverlay(overlay, image, regionX, regionY, regionWidth, regionHeight) {
        const scaleX = image.clientWidth / image.naturalWidth;
        const scaleY = image.clientHeight / image.naturalHeight;
        
        const left = regionX * scaleX;
        const top = regionY * scaleY;
        const width = regionWidth * scaleX;
        const height = regionHeight * scaleY;
        
        overlay.style.left = left + 'px';
        overlay.style.top = top + 'px';
        overlay.style.width = width + 'px';
        overlay.style.height = height + 'px';
        overlay.style.display = 'block';
    }

    startRegionEdit(imageIndex, filename) {
        const image = document.getElementById(`edit-image-${imageIndex}`);
        const container = image.closest('.edit-image-container');
        
        if (!image || !container) {
            this.showToast('Image not found', 'error');
            return;
        }

        let overlay = document.getElementById(`region-overlay-${imageIndex}`);
        if (!overlay) {
            overlay = document.createElement('div');
            overlay.id = `region-overlay-${imageIndex}`;
            overlay.className = 'region-overlay-edit';
            container.appendChild(overlay);
        }

        this.initializeRegionSelection(imageIndex, filename, image, overlay);
    }

    initializeRegionSelection(imageIndex, filename, image, overlay) {
        let isSelecting = false;
        let startX = 0;
        let startY = 0;
        let currentSelection = null;

        const getImageCoordinates = (event) => {
            const rect = image.getBoundingClientRect();
            const scaleX = image.naturalWidth / image.clientWidth;
            const scaleY = image.naturalHeight / image.clientHeight;
            
            const x = Math.round((event.clientX - rect.left) * scaleX);
            const y = Math.round((event.clientY - rect.top) * scaleY);
            
            return { x, y };
        };

        const updateOverlay = (x1, y1, x2, y2) => {
            const rect = image.getBoundingClientRect();
            const scaleX = image.clientWidth / image.naturalWidth;
            const scaleY = image.clientHeight / image.naturalHeight;
            
            const left = Math.min(x1, x2) * scaleX;
            const top = Math.min(y1, y2) * scaleY;
            const width = Math.abs(x2 - x1) * scaleX;
            const height = Math.abs(y2 - y1) * scaleY;
            
            overlay.style.left = left + 'px';
            overlay.style.top = top + 'px';
            overlay.style.width = width + 'px';
            overlay.style.height = height + 'px';
            overlay.style.display = 'block';
        };

        const updateSelection = (x1, y1, x2, y2) => {
            const minX = Math.min(x1, x2);
            const minY = Math.min(y1, y2);
            const maxX = Math.max(x1, x2);
            const maxY = Math.max(y1, y2);
            const width = maxX - minX;
            const height = maxY - minY;
            
            currentSelection = { x: minX, y: minY, width, height };
        };

        const handleMouseDown = (event) => {
            if (event.button !== 0) return;
            
            isSelecting = true;
            const coords = getImageCoordinates(event);
            startX = coords.x;
            startY = coords.y;
            
            image.style.cursor = 'crosshair';
            event.preventDefault();
        };

        const handleMouseMove = (event) => {
            if (!isSelecting) return;
            
            const coords = getImageCoordinates(event);
            updateOverlay(startX, startY, coords.x, coords.y);
            updateSelection(startX, startY, coords.x, coords.y);
            
            event.preventDefault();
        };

        const handleMouseUp = (event) => {
            if (!isSelecting) return;
            
            isSelecting = false;
            image.style.cursor = 'default';
            
            const coords = getImageCoordinates(event);
            updateSelection(startX, startY, coords.x, coords.y);
            
            this.showRegionSaveDialog(imageIndex, filename, currentSelection);
            
            event.preventDefault();
        };

        image.addEventListener('mousedown', handleMouseDown);
        image.addEventListener('mousemove', handleMouseMove);
        image.addEventListener('mouseup', handleMouseUp);
        image.addEventListener('contextmenu', (e) => e.preventDefault());
        
        image._regionCleanup = () => {
            image.removeEventListener('mousedown', handleMouseDown);
            image.removeEventListener('mousemove', handleMouseMove);
            image.removeEventListener('mouseup', handleMouseUp);
            image.style.cursor = 'default';
        };

        this.showToast('Click and drag to select a region on the image', 'info');
    }

    showRegionSaveDialog(imageIndex, filename, selection) {
        if (!selection || selection.width < 10 || selection.height < 10) {
            this.showToast('Region is too small. Please select a larger area.', 'warning');
            return;
        }

        const confirmed = confirm(`Save region: ${selection.x},${selection.y} (${selection.width}x${selection.height})?`);
        
        if (confirmed) {
            this.saveRegionToImage(imageIndex, filename, selection);
        } else {
            const overlay = document.getElementById(`region-overlay-${imageIndex}`);
            if (overlay) {
                overlay.style.display = 'none';
            }
        }

        const image = document.getElementById(`edit-image-${imageIndex}`);
        if (image && image._regionCleanup) {
            image._regionCleanup();
            delete image._regionCleanup;
        }
    }

    async saveRegionToImage(imageIndex, filename, selection) {
        try {
            const response = await fetch(`/api/ml/shells/${this.sessionId}/images/${filename}/region`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify({
                    region_x: selection.x,
                    region_y: selection.y,
                    region_width: selection.width,
                    region_height: selection.height
                })
            });

            if (response.ok) {
                this.showToast('Region saved successfully', 'success');
                await this.reloadShellData();
            } else {
                throw new Error(`Failed to save region: ${response.statusText}`);
            }
        } catch (error) {
            console.error('Error saving region:', error);
            this.showToast('Error saving region: ' + error.message, 'error');
        }
    }

    async clearRegionFromImage(imageIndex, filename) {
        if (!confirm('Are you sure you want to clear the region for this image?')) {
            return;
        }

        try {
            const response = await fetch(`/api/ml/shells/${this.sessionId}/images/${filename}/region`, {
                method: 'DELETE'
            });

            if (response.ok) {
                this.showToast('Region cleared successfully', 'success');
                await this.reloadShellData();
            } else {
                throw new Error(`Failed to clear region: ${response.statusText}`);
            }
        } catch (error) {
            console.error('Error clearing region:', error);
            this.showToast('Error clearing region: ' + error.message, 'error');
        }
    }


    showToast(message, type = 'info') {
        const toastContainer = document.getElementById('toast-container');
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
            <span class="toast-content">${message}</span>
            <button class="toast-close">×</button>
        `;
        
        toast.querySelector('.toast-close').addEventListener('click', () => {
            toast.remove();
        });
        
        toastContainer.appendChild(toast);
        
        setTimeout(() => toast.classList.add('show'), 100);
        
        setTimeout(() => {
            toast.classList.remove('show');
            setTimeout(() => toast.remove(), 300);
        }, 5000);
    }
}

// Initialize when DOM is loaded
document.addEventListener('DOMContentLoaded', () => {
    new ShellEditInterface();
});