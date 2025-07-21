//! Integration tests for the shell-sorter server with camera detection

use crate::constants::USB_DEVICE_PREFIX;
use serde_json::Value;
use shell_sorter::camera_manager::CameraManager;
use shell_sorter::config::Settings;
use shell_sorter::controller_monitor::ControllerMonitor;
use shell_sorter::usb_camera_controller::start_usb_camera_manager;
use std::time::Duration;
use tokio::time::timeout;

/// Test configuration for integration tests
fn create_test_settings() -> Settings {
    Settings {
        machine_name: "Test Machine".to_string(),
        host: "127.0.0.1".to_string(),
        port: 0, // Let the OS choose the port
        esphome_hostname: "test-esp.local".to_string(),
        network_camera_hostnames: vec!["test-cam1.local".to_string()],
        auto_detect_cameras: false,
        auto_start_esp32_cameras: false,
        data_directory: std::env::temp_dir().join("shell-sorter-test"),
        image_directory: std::env::temp_dir()
            .join("shell-sorter-test")
            .join("images"),
        models_directory: std::env::temp_dir()
            .join("shell-sorter-test")
            .join("models"),
        references_directory: std::env::temp_dir()
            .join("shell-sorter-test")
            .join("references"),
        cameras: vec![],
        camera_count: 0,
        camera_resolution: "640x480".to_string(),
        ml_enabled: false,
        confidence_threshold: 0.8,
        model_name: None,
        supported_case_types: vec![],
        debug: true,
    }
}

/// Start a test server with the given settings and return the base URL
async fn start_test_server()
-> Result<(String, tokio::task::JoinHandle<()>), Box<dyn std::error::Error>> {
    let settings = create_test_settings();

    // Try to find an available port in the high port range
    let mut listener = None;
    let mut port = 0;
    let max_attempts = 20;
    for attempt in 0..max_attempts {
        // Try random ports in the range 49152-65535 (IANA dynamic/private port range)
        let test_port = rand::random_range(49152u16..65530u16);
        match tokio::net::TcpListener::bind(format!("127.0.0.1:{test_port}")).await {
            Ok(l) => {
                port = test_port;
                listener = Some(l);
                break;
            }
            Err(err) if attempt < 9 => {
                // If binding fails, try another port
                eprintln!(
                    "Port {test_port} is in use, trying another... {err:?}, attempt {} of {max_attempts}",
                    attempt + 1,
                );
                continue;
            } // Try another port
            Err(e) => {
                return Err(format!(
                    "Failed to bind to any port after {max_attempts} attempts: {e}"
                )
                .into());
            }
        }
    }

    let listener = listener.ok_or("Failed to find available port")?;

    // Create the controller monitor
    let (controller_monitor, controller_handle) = ControllerMonitor::new(settings.clone())
        .map_err(|e| format!("Failed to create controller monitor: {e}"))?;

    // Create the camera manager
    let (camera_manager, camera_handle) =
        CameraManager::new(settings.network_camera_hostnames.clone())
            .map_err(|e| format!("Failed to create camera manager: {e}"))?;

    // Create the USB camera manager
    let usb_camera_handle = start_usb_camera_manager()
        .await
        .map_err(|e| format!("Failed to create USB camera manager: {e}"))?;

    // Spawn background tasks
    tokio::spawn(async move {
        if let Err(e) = controller_monitor.run().await {
            eprintln!("Controller monitor error: {e}");
        }
    });

    tokio::spawn(async move {
        if let Err(e) = camera_manager.run().await {
            eprintln!("Camera manager error: {e}");
        }
    });

    let base_url = format!("http://127.0.0.1:{port}");

    // Start the server in a background task with the pre-bound listener
    let server_handle = tokio::spawn(async move {
        use shell_sorter::server::{AppState, create_test_router};
        use std::sync::Arc;

        // Initialize ML trainer and shell data manager for tests
        let mut ml_trainer = shell_sorter::ml_training::MLTrainer::new(settings.clone());
        ml_trainer
            .initialize()
            .expect("Failed to initialize ML trainer in test");
        let shell_data_manager =
            shell_sorter::shell_data::ShellDataManager::new(settings.data_directory.clone());

        let state = Arc::new(AppState {
            settings,
            controller: controller_handle,
            camera_manager: camera_handle,
            usb_camera_manager: usb_camera_handle,
            ml_trainer: Arc::new(std::sync::Mutex::new(ml_trainer)),
            shell_data_manager: Arc::new(shell_data_manager),
        });

        let app = create_test_router(state);

        if let Err(e) = axum::serve(listener, app).await {
            eprintln!("Server error: {e}");
        }
    });

    // Give the server a moment to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    Ok((base_url, server_handle))
}

#[tokio::test]
async fn test_camera_detection_endpoint() {
    let (base_url, _server_handle) = start_test_server()
        .await
        .expect("Failed to start test server");

    let client = reqwest::Client::new();

    // Test camera detection endpoint
    let response = timeout(
        Duration::from_secs(30),
        client.get(format!("{base_url}/api/cameras/detect")).send(),
    )
    .await
    .expect("Request timed out")
    .expect("Failed to send request");

    assert!(
        response.status().is_success(),
        "Camera detection endpoint failed: {}",
        response.status()
    );

    let json: Value = response
        .json()
        .await
        .expect("Failed to parse JSON response");

    // Check response structure
    assert!(
        json.get("success").is_some(),
        "Response missing 'success' field"
    );
    assert!(json.get("data").is_some(), "Response missing 'data' field");

    let success = json["success"]
        .as_bool()
        .expect("'success' field is not a boolean");
    assert!(success, "Camera detection was not successful");

    let cameras = json["data"]
        .as_array()
        .expect("'data' field is not an array");

    // Print detected cameras for debugging
    println!("Detected {} cameras:", cameras.len());
    for camera in cameras {
        if let (Some(name), Some(camera_type), Some(id)) = (
            camera.get("name").and_then(|v| v.as_str()),
            camera.get("camera_type").and_then(|v| v.as_str()),
            camera.get("id").and_then(|v| v.as_str()),
        ) {
            println!("  • {name} ({camera_type}) - {id}");

            // Verify camera structure
            assert!(
                ["esphome", "usb"].contains(&camera_type),
                "Unknown camera type: {camera_type}"
            );
            assert!(!id.is_empty(), "Camera ID is empty");
            assert!(!name.is_empty(), "Camera name is empty");

            // Check type-specific fields
            match camera_type {
                "usb" => {
                    assert!(
                        camera.get("index").is_some(),
                        "USB camera missing 'index' field"
                    );
                    assert!(
                        camera.get("hostname").is_none() || camera["hostname"].is_null(),
                        "USB camera should not have hostname"
                    );
                }
                "esphome" => {
                    assert!(
                        camera.get("hostname").is_some(),
                        "ESPHome camera missing 'hostname' field"
                    );
                    assert!(
                        camera.get("index").is_none() || camera["index"].is_null(),
                        "ESPHome camera should not have index"
                    );
                }
                _ => unreachable!(),
            }
        } else {
            panic!("Camera missing required fields: {camera:?}");
        }
    }
}

#[tokio::test]
async fn test_camera_list_endpoint() {
    let (base_url, _server_handle) = start_test_server()
        .await
        .expect("Failed to start test server");

    let client = reqwest::Client::new();

    // First, trigger detection to populate the camera list
    let detect_response = timeout(
        Duration::from_secs(30),
        client.get(format!("{base_url}/api/cameras/detect")).send(),
    )
    .await
    .expect("Detection request timed out")
    .expect("Failed to send detection request");

    assert!(
        detect_response.status().is_success(),
        "Camera detection failed"
    );

    // Now test the list endpoint
    let response = timeout(
        Duration::from_secs(10),
        client.get(format!("{base_url}/api/cameras")).send(),
    )
    .await
    .expect("List request timed out")
    .expect("Failed to send list request");

    assert!(
        response.status().is_success(),
        "Camera list endpoint failed: {}",
        response.status()
    );

    let json: Value = response
        .json()
        .await
        .expect("Failed to parse JSON response");

    // Check response structure
    assert!(
        json.get("success").is_some(),
        "Response missing 'success' field"
    );
    assert!(json.get("data").is_some(), "Response missing 'data' field");

    let success = json["success"]
        .as_bool()
        .expect("'success' field is not a boolean");
    assert!(success, "Camera list was not successful");

    let cameras = json["data"]
        .as_array()
        .expect("'data' field is not an array");

    println!("Listed {} cameras:", cameras.len());
    for camera in cameras {
        if let (Some(name), Some(camera_type)) = (
            camera.get("name").and_then(|v| v.as_str()),
            camera.get("camera_type").and_then(|v| v.as_str()),
        ) {
            println!("  • {name} ({camera_type})");
        }
    }
}

#[tokio::test]
async fn test_usb_camera_detection() {
    let (base_url, _server_handle) = start_test_server()
        .await
        .expect("Failed to start test server");

    let client = reqwest::Client::new();

    // Test camera detection endpoint specifically for USB cameras
    let response = timeout(
        Duration::from_secs(30),
        client.get(format!("{base_url}/api/cameras/detect")).send(),
    )
    .await
    .expect("Request timed out")
    .expect("Failed to send request");

    assert!(
        response.status().is_success(),
        "Camera detection endpoint failed"
    );

    let json: Value = response
        .json()
        .await
        .expect("Failed to parse JSON response");
    let cameras = json["data"]
        .as_array()
        .expect("'data' field is not an array");

    // Look for USB cameras specifically
    let usb_cameras: Vec<_> = cameras
        .iter()
        .filter(|camera| {
            camera
                .get("camera_type")
                .and_then(|v| v.as_str())
                .map(|t| t == "usb")
                .unwrap_or(false)
        })
        .collect();

    println!("Found {} USB cameras", usb_cameras.len());

    for camera in &usb_cameras {
        println!("USB Camera: {camera:?}");

        // Verify USB-specific fields
        assert!(camera.get("index").is_some(), "USB camera missing index");
        assert!(camera.get("name").is_some(), "USB camera missing name");
        assert!(camera.get("id").is_some(), "USB camera missing id");

        // Hardware ID should be USB-prefixed
        if let Some(id) = camera.get("id").and_then(|v| v.as_str()) {
            assert!(
                id.starts_with(USB_DEVICE_PREFIX_WITH_COLON),
                "USB camera ID should start with 'usb:': {id}",
            );
        }
    }

    // On most development machines, there should be at least one USB camera (built-in webcam)
    // But we won't fail the test if there are none, as it depends on the hardware
    if usb_cameras.is_empty() {
        println!("No USB cameras detected - this may be expected depending on hardware");
    }
}

#[tokio::test]
async fn test_api_response_structure() {
    let (base_url, _server_handle) = start_test_server()
        .await
        .expect("Failed to start test server");

    let client = reqwest::Client::new();

    // Test various endpoints to ensure they all follow the ApiResponse structure
    let endpoints = vec![
        "/api/cameras",
        "/api/cameras/detect",
        "/api/machine/hardware-status",
        "/api/case-types",
    ];

    for endpoint in endpoints {
        println!("Testing endpoint: {endpoint}");

        let response = timeout(
            Duration::from_secs(30),
            client.get(format!("{base_url}{endpoint}")).send(),
        )
        .await
        .expect("Request timed out")
        .expect("Failed to send request");

        assert!(
            response.status().is_success(),
            "Endpoint {endpoint} failed: {}",
            response.status()
        );

        let json: Value = response
            .json()
            .await
            .expect("Failed to parse JSON response");

        // All API responses should follow the ApiResponse<T> structure
        assert!(
            json.get("success").is_some(),
            "Endpoint {endpoint} missing 'success' field"
        );
        assert!(
            json.get("message").is_some(),
            "Endpoint {endpoint} missing 'message' field",
        );

        // If successful, should have data field
        if json["success"].as_bool().unwrap_or(false) {
            assert!(
                json.get("data").is_some(),
                "Successful response from {endpoint} missing 'data' field",
            );
        }
    }
}

#[tokio::test]
async fn test_dashboard_page() {
    let (base_url, _server_handle) = start_test_server()
        .await
        .expect("Failed to start test server");

    let client = reqwest::Client::new();

    // Test that the dashboard page loads
    let response = timeout(Duration::from_secs(10), client.get(&base_url).send())
        .await
        .expect("Request timed out")
        .expect("Failed to send request");

    assert!(
        response.status().is_success(),
        "Dashboard page failed to load: {}",
        response.status()
    );

    let html = response.text().await.expect("Failed to get response text");

    // Check for expected HTML content
    assert!(
        html.contains("Shell Sorter Control Panel"),
        "Dashboard missing title"
    );
    assert!(
        html.contains("Detect Cameras"),
        "Dashboard missing detect cameras button"
    );
    assert!(
        html.contains("camera-list"),
        "Dashboard missing camera list element"
    );
}

#[tokio::test]
async fn test_status_endpoint() {
    let (base_url, _server_handle) = start_test_server()
        .await
        .expect("Failed to start test server");

    let client = reqwest::Client::new();

    let response = timeout(
        Duration::from_secs(10),
        client.get(format!("{base_url}/api/status")).send(),
    )
    .await
    .expect("Status request timed out")
    .expect("Failed to send status request");

    assert!(
        response.status().is_success(),
        "Status endpoint failed: {}",
        response.status()
    );

    let status: serde_json::Value = response
        .json()
        .await
        .expect("Failed to parse status response");

    // Check that status has the expected fields
    assert!(
        status.get("status").is_some(),
        "Status response missing 'status' field"
    );
    assert!(
        status.get("total_sorted").is_some(),
        "Status response missing 'total_sorted' field"
    );

    // Verify field types
    assert!(
        status["status"].is_string(),
        "Status 'status' field should be a string"
    );
    assert!(
        status["total_sorted"].is_number(),
        "Status 'total_sorted' field should be a number"
    );
}

#[tokio::test]
async fn test_shell_edit_page() {
    let (base_url, _server_handle) = start_test_server()
        .await
        .expect("Failed to start test server");

    let client = reqwest::Client::new();

    // First, create a test shell using the API
    let shell_data = serde_json::json!({
        "session_id": "test-session-123",
        "brand": "Winchester",
        "shell_type": "9mm",
        "include": true,
        "image_filenames": ["test1.jpg", "test2.jpg"]
    });

    let save_response = timeout(
        Duration::from_secs(10),
        client
            .post(format!("{base_url}/api/shells/save"))
            .json(&shell_data)
            .send(),
    )
    .await
    .expect("Save request timed out")
    .expect("Failed to send save request");

    assert!(
        save_response.status().is_success(),
        "Failed to save test shell data: {}",
        save_response.status()
    );

    // Now test the shell-edit page
    let response = timeout(
        Duration::from_secs(10),
        client
            .get(format!("{base_url}/shell-edit/test-session-123"))
            .send(),
    )
    .await
    .expect("Shell-edit request timed out")
    .expect("Failed to send shell-edit request");

    assert!(
        response.status().is_success(),
        "Shell-edit page failed to load: {}",
        response.status()
    );

    let html = response.text().await.expect("Failed to get response text");

    // Check for expected HTML content specific to shell editing
    assert!(
        html.contains("Edit Shell: Winchester 9mm"),
        "Shell-edit page missing proper title"
    );
    assert!(
        html.contains("test-session-123"),
        "Shell-edit page missing session ID"
    );
    assert!(
        html.contains("value=\"Winchester\""),
        "Shell-edit page missing brand value"
    );
    assert!(
        html.contains("checked"),
        "Shell-edit page should show include checkbox as checked"
    );
    assert!(
        html.contains("save-shell-changes"),
        "Shell-edit page missing save button"
    );
    assert!(
        html.contains("delete-shell-btn"),
        "Shell-edit page missing delete button"
    );
    assert!(
        html.contains("shell_edit.js"),
        "Shell-edit page missing JavaScript file"
    );
}

#[tokio::test]
async fn test_shell_edit_page_not_found() {
    let (base_url, _server_handle) = start_test_server()
        .await
        .expect("Failed to start test server");

    let client = reqwest::Client::new();

    // Test shell-edit page with non-existent session ID
    let response = timeout(
        Duration::from_secs(10),
        client
            .get(format!("{base_url}/shell-edit/non-existent-session"))
            .send(),
    )
    .await
    .expect("Shell-edit request timed out")
    .expect("Failed to send shell-edit request");

    assert_eq!(
        response.status(),
        reqwest::StatusCode::NOT_FOUND,
        "Shell-edit page should return 404 for non-existent session"
    );
}

#[tokio::test]
async fn test_tagging_page() {
    let (base_url, _server_handle) = start_test_server()
        .await
        .expect("Failed to start test server");

    let client = reqwest::Client::new();

    // Test the tagging page
    let response = timeout(
        Duration::from_secs(10),
        client
            .get(format!("{base_url}/tagging/new-session-456"))
            .send(),
    )
    .await
    .expect("Tagging request timed out")
    .expect("Failed to send tagging request");

    assert!(
        response.status().is_success(),
        "Tagging page failed to load: {}",
        response.status()
    );

    let html = response.text().await.expect("Failed to get response text");

    // Check for expected HTML content specific to shell tagging
    assert!(
        html.contains("Shell Image Tagging"),
        "Tagging page missing proper title"
    );
    assert!(
        html.contains("new-session-456"),
        "Tagging page missing session ID"
    );
    assert!(
        html.contains("Session: new-session-456"),
        "Tagging page missing session display"
    );
    assert!(html.contains("brand"), "Tagging page missing brand input");
    assert!(
        html.contains("shell_type"),
        "Tagging page missing shell type select"
    );
    assert!(
        html.contains("save-btn"),
        "Tagging page missing save button"
    );
    assert!(
        html.contains("cancel-btn"),
        "Tagging page missing cancel button"
    );
    assert!(
        html.contains("image_filenames"),
        "Tagging page missing image filenames input"
    );
}

#[tokio::test]
async fn test_shell_data_api_endpoints() {
    let (base_url, _server_handle) = start_test_server()
        .await
        .expect("Failed to start test server");

    let client = reqwest::Client::new();

    // Test saving shell data
    let shell_data = serde_json::json!({
        "session_id": "api-test-789",
        "brand": "Federal",
        "shell_type": "308win",
        "include": false,
        "image_filenames": ["federal1.jpg", "federal2.jpg", "federal3.jpg"]
    });

    let save_response = timeout(
        Duration::from_secs(10),
        client
            .post(format!("{base_url}/api/shells/save"))
            .json(&shell_data)
            .send(),
    )
    .await
    .expect("Save request timed out")
    .expect("Failed to send save request");

    assert!(
        save_response.status().is_success(),
        "Failed to save shell data: {}",
        save_response.status()
    );

    let save_json: Value = save_response
        .json()
        .await
        .expect("Failed to parse save response");

    assert!(
        save_json["success"].as_bool().unwrap_or(false),
        "Shell save was not successful"
    );
    assert!(
        save_json["data"]["session_id"] == "api-test-789",
        "Save response missing correct session ID"
    );

    // Test listing shells
    let list_response = timeout(
        Duration::from_secs(10),
        client.get(format!("{base_url}/api/shells")).send(),
    )
    .await
    .expect("List request timed out")
    .expect("Failed to send list request");

    assert!(
        list_response.status().is_success(),
        "Failed to list shells: {}",
        list_response.status()
    );

    let list_json: Value = list_response
        .json()
        .await
        .expect("Failed to parse list response");

    assert!(
        list_json["success"].as_bool().unwrap_or(false),
        "Shell list was not successful"
    );

    let shells = list_json["data"]
        .as_array()
        .expect("Shell list data is not an array");

    // Should find our saved shell
    let found_shell = shells.iter().find(|shell| {
        shell["session_id"]
            .as_str()
            .map(|id| id == "api-test-789")
            .unwrap_or(false)
    });

    assert!(found_shell.is_some(), "Saved shell not found in list");

    let shell = found_shell.unwrap();
    assert_eq!(
        shell["brand"].as_str().unwrap(),
        "Federal",
        "Shell brand mismatch"
    );
    assert_eq!(
        shell["shell_type"].as_str().unwrap(),
        "308win",
        "Shell type mismatch"
    );
    assert_eq!(
        shell["include"].as_bool().unwrap(),
        false,
        "Shell include flag mismatch"
    );

    // Test toggling training flag
    let toggle_response = timeout(
        Duration::from_secs(10),
        client
            .post(format!("{base_url}/api/shells/api-test-789/toggle"))
            .send(),
    )
    .await
    .expect("Toggle request timed out")
    .expect("Failed to send toggle request");

    assert!(
        toggle_response.status().is_success(),
        "Failed to toggle shell training: {}",
        toggle_response.status()
    );

    let toggle_json: Value = toggle_response
        .json()
        .await
        .expect("Failed to parse toggle response");

    assert!(
        toggle_json["success"].as_bool().unwrap_or(false),
        "Shell toggle was not successful"
    );
    assert_eq!(
        toggle_json["data"]["include"].as_bool().unwrap(),
        true,
        "Training flag should be toggled to true"
    );
}

#[tokio::test]
async fn test_ml_training_api_endpoints() {
    let (base_url, _server_handle) = start_test_server()
        .await
        .expect("Failed to start test server");

    let client = reqwest::Client::new();

    // Test ML shells endpoint
    let ml_response = timeout(
        Duration::from_secs(10),
        client.get(format!("{base_url}/api/ml/shells")).send(),
    )
    .await
    .expect("ML shells request timed out")
    .expect("Failed to send ML shells request");

    assert!(
        ml_response.status().is_success(),
        "Failed to get ML shells: {}",
        ml_response.status()
    );

    let ml_json: Value = ml_response
        .json()
        .await
        .expect("Failed to parse ML shells response");

    assert!(
        ml_json["success"].as_bool().unwrap_or(false),
        "ML shells request was not successful"
    );

    // Data should be an array (may be empty initially)
    let shells = ml_json["data"]
        .as_array()
        .expect("ML shells data is not an array");

    println!("ML training shells found: {}", shells.len());

    // Test case types endpoint
    let case_types_response = timeout(
        Duration::from_secs(10),
        client.get(format!("{base_url}/api/case-types")).send(),
    )
    .await
    .expect("Case types request timed out")
    .expect("Failed to send case types request");

    assert!(
        case_types_response.status().is_success(),
        "Failed to get case types: {}",
        case_types_response.status()
    );

    let case_types_json: Value = case_types_response
        .json()
        .await
        .expect("Failed to parse case types response");

    assert!(
        case_types_json["success"].as_bool().unwrap_or(false),
        "Case types request was not successful"
    );

    // Data should be an array (may be empty initially)
    let case_types = case_types_json["data"]
        .as_array()
        .expect("Case types data is not an array");

    println!("Case types found: {}", case_types.len());

    // Verify each case type has required fields
    for case_type in case_types {
        assert!(
            case_type.get("name").is_some(),
            "Case type missing name field"
        );
        assert!(
            case_type.get("designation").is_some(),
            "Case type missing designation field"
        );
        assert!(
            case_type.get("reference_count").is_some(),
            "Case type missing reference_count field"
        );
        assert!(
            case_type.get("training_count").is_some(),
            "Case type missing training_count field"
        );
        assert!(
            case_type.get("shell_count").is_some(),
            "Case type missing shell_count field"
        );
        assert!(
            case_type.get("ready_for_training").is_some(),
            "Case type missing ready_for_training field"
        );
    }
}

#[tokio::test]
async fn test_config_page() {
    let (base_url, _server_handle) = start_test_server()
        .await
        .expect("Failed to start test server");

    let client = reqwest::Client::new();

    // Test config page loads
    let response = timeout(
        Duration::from_secs(10),
        client.get(format!("{base_url}/config")).send(),
    )
    .await
    .expect("Config request timed out")
    .expect("Failed to send config request");

    assert!(
        response.status().is_success(),
        "Config page failed to load: {}",
        response.status()
    );

    let html = response.text().await.expect("Failed to get response text");

    // Check for expected HTML content
    assert!(
        html.contains("Configuration"),
        "Config page missing configuration title"
    );
    assert!(
        html.contains("config.js"),
        "Config page missing JavaScript file"
    );
}
