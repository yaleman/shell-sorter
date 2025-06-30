// ML Training Interface JavaScript

class MLTrainingInterface {
    constructor() {
        this.shells = [];
        this.filteredShells = [];
        this.initializeEventListeners();
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
                        <div class="images-section">
                            <h4>Images</h4>
                            <div class="edit-images">
                                ${(shell.captured_images || shell.image_filenames.map(f => ({filename: f, view_type: 'unknown'}))).map(img => `
                                    <div class="edit-image-item">
                                        <img src="/images/${img.filename}" alt="Shell image" class="edit-image">
                                        <div class="edit-image-controls">
                                            <label>View Type:</label>
                                            <select class="edit-view-type" data-filename="${img.filename}">
                                                <option value="unknown" ${img.view_type === 'unknown' ? 'selected' : ''}>Unknown</option>
                                                <option value="side" ${img.view_type === 'side' ? 'selected' : ''}>Side View</option>
                                                <option value="tail" ${img.view_type === 'tail' ? 'selected' : ''}>Tail View</option>
                                            </select>
                                            <button class="btn btn-sm btn-danger delete-image-btn" data-filename="${img.filename}">Delete Image</button>
                                        </div>
                                    </div>
                                `).join('')}
                            </div>
                        </div>
                    </div>
                    <div class="modal-footer">
                        <button class="btn btn-primary" id="save-shell-changes">Save Changes</button>
                        <button class="btn btn-secondary" id="cancel-edit-modal">Cancel</button>
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
        
        // Handle image deletion
        document.querySelectorAll('.delete-image-btn').forEach(btn => {
            btn.addEventListener('click', (e) => {
                const filename = e.target.dataset.filename;
                this.deleteImageFromShell(sessionId, filename);
            });
        });
    }

    closeEditModal() {
        const modal = document.getElementById('edit-modal-overlay');
        if (modal) {
            modal.remove();
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