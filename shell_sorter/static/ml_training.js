// ML Training Interface JavaScript

class MLTrainingInterface {
    constructor() {
        this.shells = [];
        this.filteredShells = [];
        this.initializeEventListeners();
        
        // Auto-load shell data when the interface initializes
        this.loadShells();
    }

    initializeEventListeners() {
        // Button event listeners
        document.getElementById('load-shells-btn').addEventListener('click', () => this.loadShells());
        document.getElementById('generate-composites-btn').addEventListener('click', () => this.generateComposites());
        document.getElementById('start-training-btn').addEventListener('click', () => this.startTraining());
        
        // Filter event listeners
        document.getElementById('filter-brand').addEventListener('change', () => this.applyFilters());
        document.getElementById('filter-type').addEventListener('change', () => this.applyFilters());
        document.getElementById('filter-include').addEventListener('change', () => this.applyFilters());
        
        // Edit and delete button event delegation
        document.addEventListener('click', (event) => {
            if (event.target.classList.contains('edit-shell-btn')) {
                const sessionId = event.target.dataset.sessionId;
                this.openShellEditModal(sessionId);
            } else if (event.target.classList.contains('delete-shell-btn')) {
                const sessionId = event.target.dataset.sessionId;
                this.deleteShell(sessionId);
            }
        });
        
        // ML Training button in main interface
        const mlTrainingBtn = document.getElementById('ml-training-btn');
        if (mlTrainingBtn) {
            mlTrainingBtn.addEventListener('click', () => {
                window.location.href = '/ml-training';
            });
        }
    }

    async loadShells() {
        try {
            this.showToast('Loading shell data...', 'info');
            
            const response = await fetch('/api/ml/shells');
            if (!response.ok) {
                throw new Error(`HTTP error! status: ${response.status}`);
            }
            
            const data = await response.json();
            this.shells = data.shells;
            this.filteredShells = [...this.shells];
            
            this.updateStatistics(data.summary);
            this.populateFilters();
            this.renderShells();
            this.updateButtonStates();
            
            // Check for existing composite images
            this.checkExistingComposites();
            
            this.showToast(`Loaded ${data.summary.total} shells (${data.summary.included} included)`, 'success');
            
        } catch (error) {
            console.error('Error loading shells:', error);
            this.showToast('Error loading shell data: ' + error.message, 'error');
        }
    }

    updateStatistics(summary) {
        document.getElementById('total-shells').textContent = summary.total;
        document.getElementById('included-shells').textContent = summary.included;
        document.getElementById('unique-types').textContent = summary.unique_types;
        
        // Update shell type coverage statistics
        this.updateShellTypeStats();
    }

    updateShellTypeStats() {
        const shellTypeStats = document.getElementById('shell-type-stats');
        const typeStatsList = document.getElementById('type-stats-list');
        
        if (!this.shells || this.shells.length === 0) {
            shellTypeStats.style.display = 'none';
            return;
        }

        // Group shells by type and calculate statistics
        const typeGroups = {};
        this.shells.forEach(shell => {
            const typeKey = `${shell.brand} ${shell.shell_type}`;
            if (!typeGroups[typeKey]) {
                typeGroups[typeKey] = {
                    total: 0,
                    included: 0
                };
            }
            typeGroups[typeKey].total++;
            if (shell.include !== false) {
                typeGroups[typeKey].included++;
            }
        });

        // Filter types that have over 50% selection or significant data
        const significantTypes = Object.entries(typeGroups)
            .map(([typeName, stats]) => ({
                name: typeName,
                ...stats,
                percentage: (stats.included / stats.total) * 100
            }))
            .filter(type => type.percentage >= 50 || type.total >= 3)
            .sort((a, b) => b.percentage - a.percentage);

        if (significantTypes.length === 0) {
            shellTypeStats.style.display = 'none';
            return;
        }

        // Generate HTML for type statistics
        typeStatsList.innerHTML = significantTypes.map(type => {
            const cssClass = type.percentage >= 75 ? 'good' : 'warning';
            return `
                <div class="type-stat-item ${cssClass}">
                    <span class="type-stat-name">${type.name}</span>
                    <span class="type-stat-ratio">${type.included}/${type.total}</span>
                </div>
            `;
        }).join('');

        shellTypeStats.style.display = 'block';
    }

    populateFilters() {
        const brands = [...new Set(this.shells.map(s => s.brand))].sort();
        const types = [...new Set(this.shells.map(s => s.shell_type))].sort();
        
        const brandSelect = document.getElementById('filter-brand');
        const typeSelect = document.getElementById('filter-type');
        
        // Clear existing options (except "All" option)
        brandSelect.innerHTML = '<option value="">All Brands</option>';
        typeSelect.innerHTML = '<option value="">All Types</option>';
        
        brands.forEach(brand => {
            const option = document.createElement('option');
            option.value = brand;
            option.textContent = brand;
            brandSelect.appendChild(option);
        });
        
        types.forEach(type => {
            const option = document.createElement('option');
            option.value = type;
            option.textContent = type;
            typeSelect.appendChild(option);
        });
    }

    applyFilters() {
        const brandFilter = document.getElementById('filter-brand').value;
        const typeFilter = document.getElementById('filter-type').value;
        const includeFilter = document.getElementById('filter-include').value;
        
        this.filteredShells = this.shells.filter(shell => {
            if (brandFilter && shell.brand !== brandFilter) return false;
            if (typeFilter && shell.shell_type !== typeFilter) return false;
            if (includeFilter !== '') {
                const isIncluded = shell.include !== false; // Default to true if not specified
                if (includeFilter === 'true' && !isIncluded) return false;
                if (includeFilter === 'false' && isIncluded) return false;
            }
            return true;
        });
        
        this.renderShells();
    }

    renderShells() {
        const shellList = document.getElementById('shell-list');
        
        if (this.filteredShells.length === 0) {
            shellList.innerHTML = '<div class="loading-message">No shells match the current filters</div>';
            return;
        }
        
        shellList.innerHTML = this.filteredShells.map(shell => this.createShellElement(shell)).join('');
        
        // Add event listeners for toggle buttons
        this.filteredShells.forEach(shell => {
            const toggle = document.getElementById(`toggle-${shell.session_id}`);
            if (toggle) {
                toggle.addEventListener('change', () => this.toggleShellInclude(shell.session_id, toggle.checked));
            }
        });
    }

    createShellElement(shell) {
        const isIncluded = shell.include !== false; // Default to true if not specified
        const formattedDate = new Date(shell.date_captured).toLocaleDateString();
        
        // Use captured_images if available, otherwise fall back to image_filenames
        const images = shell.captured_images && shell.captured_images.length > 0 
            ? shell.captured_images 
            : shell.image_filenames.map(filename => ({ filename, view_type: null }));
        
        // Sort images: side views first, then tail views, then unknown/unspecified
        const sortedImages = [...images].sort((a, b) => {
            const viewOrder = { 'side': 0, 'tail': 1, 'unknown': 2, null: 3, undefined: 3 };
            return viewOrder[a.view_type] - viewOrder[b.view_type];
        });
        
        return `
            <div class="shell-item ${isIncluded ? 'included' : 'excluded'}" data-session-id="${shell.session_id}">
                <div class="shell-header">
                    <div class="shell-info">
                        <div class="shell-title">${shell.brand} ${shell.shell_type}</div>
                        <div class="shell-details">
                            Captured: ${formattedDate} | Images: ${images.length} | Session: ${shell.session_id}
                        </div>
                    </div>
                    <div class="shell-actions">
                        <div class="shell-toggle">
                            <label for="toggle-${shell.session_id}">Include in Training:</label>
                            <input type="checkbox" id="toggle-${shell.session_id}" class="include-toggle" ${isIncluded ? 'checked' : ''}>
                        </div>
                        <button class="btn btn-sm btn-secondary edit-shell-btn" data-session-id="${shell.session_id}">Edit Shell</button>
                        <button class="btn btn-sm btn-danger delete-shell-btn" data-session-id="${shell.session_id}">Delete</button>
                    </div>
                </div>
                <div class="shell-images">
                    ${sortedImages.map((image, index) => `
                        <div class="shell-image-container" data-image-index="${index}">
                            <img src="/images/${image.filename}" alt="Shell image" class="shell-image" onerror="this.style.display='none'">
                            <div class="image-view-badge">
                                <span class="view-type-badge view-type-${image.view_type || 'unknown'}">${this.formatViewType(image.view_type)}</span>
                            </div>
                        </div>
                    `).join('')}
                </div>
                <div class="composite-preview" id="composite-${shell.session_id}" style="display: none;">
                    <img src="/api/composites/${shell.session_id}" alt="Composite image" class="composite-image" 
                         onload="this.parentElement.style.display='block'" 
                         onerror="this.parentElement.style.display='none'">
                </div>
            </div>
        `;
    }

    formatViewType(viewType) {
        switch(viewType) {
            case 'side': return 'Side View';
            case 'tail': return 'Tail View';
            case 'unknown': return 'Unknown';
            default: return 'Unknown';
        }
    }

    openShellEditModal(sessionId) {
        const shell = this.shells.find(s => s.session_id === sessionId);
        if (!shell) {
            this.showToast('Shell not found', 'error');
            return;
        }

        // Create modal HTML
        const modalHTML = `
            <div class="modal-overlay" id="edit-modal-overlay">
                <div class="edit-modal">
                    <div class="modal-header">
                        <h3>Edit Shell: ${shell.brand} ${shell.shell_type}</h3>
                        <button class="modal-close" id="close-edit-modal">&times;</button>
                    </div>
                    <div class="modal-content">
                        <div class="form-group">
                            <label>Brand:</label>
                            <input type="text" id="edit-brand" value="${shell.brand}">
                        </div>
                        <div class="form-group">
                            <label>Shell Type:</label>
                            <input type="text" id="edit-shell-type" value="${shell.shell_type}">
                        </div>
                        <div class="form-group">
                            <label>
                                <input type="checkbox" id="edit-include" ${shell.include !== false ? 'checked' : ''}>
                                Include in Training
                            </label>
                        </div>
                        <div class="composite-section">
                            <h4>Composite Image</h4>
                            <div class="composite-preview-modal">
                                <img src="/api/composites/${shell.session_id}" alt="Composite image" class="composite-image-modal" 
                                     onerror="this.parentElement.innerHTML='<div class=\\'no-composite\\'>No composite image available. Generate composites first.</div>'">
                            </div>
                        </div>
                        
                        <div class="images-section">
                            <h4>Images & Regions</h4>
                            <div class="edit-images">
                                ${(shell.captured_images || shell.image_filenames.map(f => ({filename: f, view_type: 'unknown'}))).map((img, index) => `
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
                                `).join('')}
                            </div>
                        </div>
                    </div>
                    <div class="modal-footer">
                        <button class="btn btn-danger" id="delete-shell-from-modal">Delete Shell</button>
                        <div class="modal-footer-right">
                            <button class="btn btn-primary" id="save-shell-changes">Save Changes</button>
                            <button class="btn btn-secondary" id="cancel-edit-modal">Cancel</button>
                        </div>
                    </div>
                </div>
            </div>
        `;

        // Add modal to page
        document.body.insertAdjacentHTML('beforeend', modalHTML);

        // Add event listeners
        document.getElementById('close-edit-modal').addEventListener('click', () => this.closeEditModal());
        document.getElementById('cancel-edit-modal').addEventListener('click', () => this.closeEditModal());
        document.getElementById('save-shell-changes').addEventListener('click', () => this.saveShellChanges(sessionId));
        document.getElementById('delete-shell-from-modal').addEventListener('click', () => this.deleteShellFromModal(sessionId));
        
        // Handle image deletion
        document.querySelectorAll('.delete-image-btn').forEach(btn => {
            btn.addEventListener('click', (e) => {
                const filename = e.target.dataset.filename;
                this.deleteImageFromShell(sessionId, filename);
            });
        });
        
        // Handle region editing
        document.querySelectorAll('.edit-region-btn').forEach(btn => {
            btn.addEventListener('click', (e) => {
                const imageIndex = parseInt(e.target.dataset.imageIndex);
                const filename = e.target.dataset.filename;
                this.startRegionEdit(sessionId, imageIndex, filename);
            });
        });
        
        // Handle region clearing
        document.querySelectorAll('.clear-region-btn').forEach(btn => {
            btn.addEventListener('click', (e) => {
                const imageIndex = parseInt(e.target.dataset.imageIndex);
                const filename = e.target.dataset.filename;
                this.clearRegionFromImage(sessionId, imageIndex, filename);
            });
        });
        
        // Initialize region overlays
        setTimeout(() => this.initializeRegionOverlays(), 100);
    }

    closeEditModal() {
        const modal = document.getElementById('edit-modal-overlay');
        if (modal) {
            modal.remove();
        }
    }

    async deleteShellFromModal(sessionId) {
        const shell = this.shells.find(s => s.session_id === sessionId);
        if (!shell) {
            this.showToast('Shell not found', 'error');
            return;
        }

        if (!confirm(`Are you sure you want to delete the shell "${shell.brand} ${shell.shell_type}" and all its images? This action cannot be undone.`)) {
            return;
        }

        try {
            const response = await fetch(`/api/ml/shells/${sessionId}`, {
                method: 'DELETE'
            });

            if (response.ok) {
                this.showToast('Shell deleted successfully', 'success');
                this.closeEditModal();
                this.loadShells(); // Reload to show changes
            } else {
                throw new Error(`Failed to delete shell: ${response.statusText}`);
            }
        } catch (error) {
            console.error('Error deleting shell:', error);
            this.showToast('Error deleting shell: ' + error.message, 'error');
        }
    }

    async saveShellChanges(sessionId) {
        try {
            const brand = document.getElementById('edit-brand').value.trim();
            const shellType = document.getElementById('edit-shell-type').value.trim();
            const include = document.getElementById('edit-include').checked;

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
            const response = await fetch(`/api/ml/shells/${sessionId}/update`, {
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
                this.closeEditModal();
                this.loadShells(); // Reload to show changes
            } else {
                throw new Error(`Failed to update shell: ${response.statusText}`);
            }
        } catch (error) {
            console.error('Error saving shell changes:', error);
            this.showToast('Error saving changes: ' + error.message, 'error');
        }
    }

    async deleteShell(sessionId) {
        const shell = this.shells.find(s => s.session_id === sessionId);
        if (!shell) {
            this.showToast('Shell not found', 'error');
            return;
        }

        if (!confirm(`Are you sure you want to delete the shell "${shell.brand} ${shell.shell_type}" and all its images? This action cannot be undone.`)) {
            return;
        }

        try {
            const response = await fetch(`/api/ml/shells/${sessionId}`, {
                method: 'DELETE'
            });

            if (response.ok) {
                this.showToast('Shell deleted successfully', 'success');
                this.loadShells(); // Reload to show changes
            } else {
                throw new Error(`Failed to delete shell: ${response.statusText}`);
            }
        } catch (error) {
            console.error('Error deleting shell:', error);
            this.showToast('Error deleting shell: ' + error.message, 'error');
        }
    }

    async deleteImageFromShell(sessionId, filename) {
        if (!confirm(`Are you sure you want to delete the image "${filename}"? This action cannot be undone.`)) {
            return;
        }

        try {
            const response = await fetch(`/api/ml/shells/${sessionId}/images/${filename}`, {
                method: 'DELETE'
            });

            if (response.ok) {
                this.showToast('Image deleted successfully', 'success');
                // Reload the modal to show changes
                this.closeEditModal();
                this.openShellEditModal(sessionId);
            } else {
                throw new Error(`Failed to delete image: ${response.statusText}`);
            }
        } catch (error) {
            console.error('Error deleting image:', error);
            this.showToast('Error deleting image: ' + error.message, 'error');
        }
    }

    initializeRegionOverlays() {
        // Initialize region overlays for images that have regions
        document.querySelectorAll('.region-overlay-edit').forEach(overlay => {
            const regionX = parseInt(overlay.dataset.regionX);
            const regionY = parseInt(overlay.dataset.regionY);
            const regionWidth = parseInt(overlay.dataset.regionWidth);
            const regionHeight = parseInt(overlay.dataset.regionHeight);
            
            // Find the corresponding image
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

    startRegionEdit(sessionId, imageIndex, filename) {
        const image = document.getElementById(`edit-image-${imageIndex}`);
        const container = image.closest('.edit-image-container');
        
        if (!image || !container) {
            this.showToast('Image not found', 'error');
            return;
        }

        // Create or get overlay
        let overlay = document.getElementById(`region-overlay-${imageIndex}`);
        if (!overlay) {
            overlay = document.createElement('div');
            overlay.id = `region-overlay-${imageIndex}`;
            overlay.className = 'region-overlay-edit';
            container.appendChild(overlay);
        }

        // Initialize region selection on this image
        this.initializeRegionSelection(sessionId, imageIndex, filename, image, overlay);
    }

    initializeRegionSelection(sessionId, imageIndex, filename, image, overlay) {
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

        // Mouse events for region selection
        const handleMouseDown = (event) => {
            if (event.button !== 0) return; // Only left mouse button
            
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
            
            // Show save/cancel buttons
            this.showRegionSaveDialog(sessionId, imageIndex, filename, currentSelection);
            
            event.preventDefault();
        };

        // Add event listeners
        image.addEventListener('mousedown', handleMouseDown);
        image.addEventListener('mousemove', handleMouseMove);
        image.addEventListener('mouseup', handleMouseUp);
        image.addEventListener('contextmenu', (e) => e.preventDefault());
        
        // Store cleanup function
        image._regionCleanup = () => {
            image.removeEventListener('mousedown', handleMouseDown);
            image.removeEventListener('mousemove', handleMouseMove);
            image.removeEventListener('mouseup', handleMouseUp);
            image.style.cursor = 'default';
        };

        this.showToast('Click and drag to select a region on the image', 'info');
    }

    showRegionSaveDialog(sessionId, imageIndex, filename, selection) {
        if (!selection || selection.width < 10 || selection.height < 10) {
            this.showToast('Region is too small. Please select a larger area.', 'warning');
            return;
        }

        const confirmed = confirm(`Save region: ${selection.x},${selection.y} (${selection.width}x${selection.height})?`);
        
        if (confirmed) {
            this.saveRegionToImage(sessionId, imageIndex, filename, selection);
        } else {
            // Clear the overlay
            const overlay = document.getElementById(`region-overlay-${imageIndex}`);
            if (overlay) {
                overlay.style.display = 'none';
            }
        }

        // Clean up event listeners
        const image = document.getElementById(`edit-image-${imageIndex}`);
        if (image && image._regionCleanup) {
            image._regionCleanup();
            delete image._regionCleanup;
        }
    }

    async saveRegionToImage(sessionId, imageIndex, filename, selection) {
        try {
            const response = await fetch(`/api/ml/shells/${sessionId}/images/${filename}/region`, {
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
                // Reload the modal to show changes
                this.closeEditModal();
                this.openShellEditModal(sessionId);
            } else {
                throw new Error(`Failed to save region: ${response.statusText}`);
            }
        } catch (error) {
            console.error('Error saving region:', error);
            this.showToast('Error saving region: ' + error.message, 'error');
        }
    }

    async clearRegionFromImage(sessionId, imageIndex, filename) {
        if (!confirm('Are you sure you want to clear the region for this image?')) {
            return;
        }

        try {
            const response = await fetch(`/api/ml/shells/${sessionId}/images/${filename}/region`, {
                method: 'DELETE'
            });

            if (response.ok) {
                this.showToast('Region cleared successfully', 'success');
                // Reload the modal to show changes
                this.closeEditModal();
                this.openShellEditModal(sessionId);
            } else {
                throw new Error(`Failed to clear region: ${response.statusText}`);
            }
        } catch (error) {
            console.error('Error clearing region:', error);
            this.showToast('Error clearing region: ' + error.message, 'error');
        }
    }

    async toggleShellInclude(sessionId, include) {
        try {
            const response = await fetch(`/api/ml/shells/${sessionId}/toggle`, {
                method: 'POST'
            });
            
            if (!response.ok) {
                throw new Error(`HTTP error! status: ${response.status}`);
            }
            
            const data = await response.json();
            
            // Update shell in local data
            const shell = this.shells.find(s => s.session_id === sessionId);
            if (shell) {
                shell.include = data.include;
            }
            
            // Update UI
            const shellElement = document.querySelector(`[data-session-id="${sessionId}"]`);
            if (shellElement) {
                shellElement.className = `shell-item ${data.include ? 'included' : 'excluded'}`;
            }
            
            // Update statistics
            const includedCount = this.shells.filter(s => s.include !== false).length;
            document.getElementById('included-shells').textContent = includedCount;
            
            // Update shell type statistics
            this.updateShellTypeStats();
            
            this.showToast(data.message, 'success');
            
        } catch (error) {
            console.error('Error toggling shell include:', error);
            this.showToast('Error updating shell: ' + error.message, 'error');
            
            // Reset checkbox on error
            const toggle = document.getElementById(`toggle-${sessionId}`);
            if (toggle) {
                toggle.checked = !toggle.checked;
            }
        }
    }

    async generateComposites() {
        try {
            this.showProgress(true, 'Generating composite images...');
            
            const response = await fetch('/api/ml/generate-composites', {
                method: 'POST'
            });
            
            if (!response.ok) {
                throw new Error(`HTTP error! status: ${response.status}`);
            }
            
            const data = await response.json();
            
            this.showProgress(false);
            this.showToast(data.message, data.errors > 0 ? 'warning' : 'success');
            
            // Show composite images
            this.shells.forEach(shell => {
                if (shell.include !== false) {
                    const compositePreview = document.getElementById(`composite-${shell.session_id}`);
                    if (compositePreview) {
                        compositePreview.style.display = 'block';
                    }
                }
            });
            
            this.updateButtonStates();
            
        } catch (error) {
            console.error('Error generating composites:', error);
            this.showProgress(false);
            this.showToast('Error generating composite images: ' + error.message, 'error');
        }
    }

    async startTraining() {
        try {
            this.showProgress(true, 'Training ML model...');
            
            // Get included shells grouped by type
            const includedShells = this.shells.filter(s => s.include !== false);
            const shellsByType = {};
            
            includedShells.forEach(shell => {
                const key = `${shell.brand}_${shell.shell_type}`;
                if (!shellsByType[key]) {
                    shellsByType[key] = [];
                }
                shellsByType[key].push(shell);
            });
            
            const caseTypes = Object.keys(shellsByType);
            
            if (caseTypes.length === 0) {
                throw new Error('No shells included for training');
            }
            
            // Check minimum requirements
            const insufficientTypes = caseTypes.filter(type => shellsByType[type].length < 5);
            if (insufficientTypes.length > 0) {
                this.showToast(`Warning: Some types have fewer than 5 shells: ${insufficientTypes.join(', ')}`, 'warning');
            }
            
            const response = await fetch('/api/train-model', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({
                    case_types: caseTypes
                })
            });
            
            if (!response.ok) {
                throw new Error(`HTTP error! status: ${response.status}`);
            }
            
            const data = await response.json();
            
            this.showProgress(false);
            this.showToast(`Training completed! ${data.message}`, 'success');
            
        } catch (error) {
            console.error('Error training model:', error);
            this.showProgress(false);
            this.showToast('Error training model: ' + error.message, 'error');
        }
    }

    updateButtonStates() {
        const hasShells = this.shells.length > 0;
        const hasIncludedShells = this.shells.some(s => s.include !== false);
        
        document.getElementById('generate-composites-btn').disabled = !hasIncludedShells;
        document.getElementById('start-training-btn').disabled = !hasIncludedShells;
    }

    showProgress(show, text = 'Processing...') {
        const progressDiv = document.getElementById('training-progress');
        const progressText = document.getElementById('progress-text');
        
        if (show) {
            progressText.textContent = text;
            progressDiv.style.display = 'block';
        } else {
            progressDiv.style.display = 'none';
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
        
        // Add event listener for close button
        toast.querySelector('.toast-close').addEventListener('click', () => {
            toast.remove();
        });
        
        toastContainer.appendChild(toast);
        
        // Show toast
        setTimeout(() => toast.classList.add('show'), 100);
        
        // Auto-remove after 5 seconds
        setTimeout(() => {
            toast.classList.remove('show');
            setTimeout(() => toast.remove(), 300);
        }, 5000);
    }

    async checkExistingComposites() {
        // Check for existing composite images for included shells
        const includedShells = this.shells.filter(s => s.include !== false);
        
        for (const shell of includedShells) {
            try {
                // Try to load the composite image to see if it exists
                const response = await fetch(`/api/composites/${shell.session_id}`, {
                    method: 'HEAD' // Just check if it exists without downloading
                });
                
                if (response.ok) {
                    // Composite exists, show it
                    const compositePreview = document.getElementById(`composite-${shell.session_id}`);
                    if (compositePreview) {
                        compositePreview.style.display = 'block';
                    }
                }
            } catch (error) {
                // Composite doesn't exist or error occurred, leave it hidden
                console.debug(`No composite found for ${shell.session_id}`);
            }
        }
    }
}

// Initialize when DOM is loaded
document.addEventListener('DOMContentLoaded', () => {
    new MLTrainingInterface();
});