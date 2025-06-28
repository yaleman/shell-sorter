document.addEventListener('DOMContentLoaded', function() {
    const startForm = document.getElementById('start-form');
    const stopBtn = document.getElementById('stop-btn');
    
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
});