//! Integration tests for the shell-sorter server with camera detection

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

    for attempt in 0..10 {
        // Try random ports in the range 49152-65535 (IANA dynamic/private port range)
        let test_port =
            49152 + (std::process::id() as u16 + attempt as u16 * 1000) % (65535 - 49152);
        match tokio::net::TcpListener::bind(format!("127.0.0.1:{test_port}")).await {
            Ok(l) => {
                port = test_port;
                listener = Some(l);
                break;
            }
            Err(_) if attempt < 9 => continue, // Try another port
            Err(e) => {
                return Err(format!("Failed to bind to any port after 10 attempts: {e}").into());
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

        let state = Arc::new(AppState {
            settings,
            controller: controller_handle,
            camera_manager: camera_handle,
            usb_camera_manager: usb_camera_handle,
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
                id.starts_with("usb:"),
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
