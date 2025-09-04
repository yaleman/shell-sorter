use std::num::NonZeroU16;
use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use shell_sorter::camera_manager::EspCameraManager;
use shell_sorter::config::Settings;
use shell_sorter::controller_monitor::ControllerMonitor;
use shell_sorter::server;
use shell_sorter::usb_camera_controller::start_usb_camera_manager;
use shell_sorter::{OurError, OurResult};
use tokio::sync::RwLock;
use tracing::{debug, info};
use tracing_subscriber::{filter::EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(name = "shell-sorter")]
#[command(about = "Ammunition shell case sorting machine controller")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable debug output
    #[arg(short, long, global = true)]
    debug: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Machine control operations
    Machine {
        #[command(subcommand)]
        action: MachineAction,
    },
    /// Camera operations
    Camera {
        #[command(subcommand)]
        action: CameraAction,
    },
    /// Data management operations
    Data {
        #[command(subcommand)]
        action: DataAction,
    },
    /// Machine learning operations
    Ml {
        #[command(subcommand)]
        action: MlAction,
    },
    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Start the web server
    Serve {
        /// Host to bind to
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// Port to bind to
        #[arg(long, default_value = "8000")]
        port: NonZeroU16,
    },
}

#[derive(Subcommand)]
enum MachineAction {
    /// Trigger next case sequence
    NextCase,
    /// Show machine status
    Status,
    /// Show sensor readings
    Sensors,
    /// Flash control
    Flash {
        /// Flash state (on/off)
        state: String,
        /// Flash brightness (0-100)
        #[arg(long)]
        brightness: Option<u8>,
    },
}

#[derive(Subcommand)]
enum CameraAction {
    /// Detect available cameras
    Detect,
    /// List configured cameras
    List,
    /// Capture images
    Capture {
        /// Session ID for capture
        #[arg(long)]
        session_id: Option<String>,
    },
    /// Start camera stream
    Stream {
        /// Camera index
        #[arg(long)]
        index: Option<usize>,
    },
    /// USB camera operations
    Usb {
        #[command(subcommand)]
        action: UsbCameraAction,
    },
}

#[derive(Subcommand)]
enum UsbCameraAction {
    /// Detect USB cameras with hardware identification
    Detect,
    /// List detected USB cameras
    List,
    /// Capture image from USB camera
    Capture {
        /// Hardware ID of camera to capture from
        hardware_id: String,
    },
    /// Test USB camera functionality
    Test {
        /// Hardware ID of camera to test
        hardware_id: String,
    },
}

#[derive(Subcommand)]
enum DataAction {
    /// List shell case data
    ListShells,
    /// Tag captured images
    Tag {
        /// Session ID to tag
        #[arg(long)]
        session_id: Option<String>,
    },
    /// Export data
    Export {
        /// Export format (json/csv)
        #[arg(long, default_value = "json")]
        format: String,
    },
    /// Import data
    Import {
        /// File to import
        #[arg(long)]
        file: String,
    },
}

#[derive(Subcommand)]
enum MlAction {
    /// List case types
    ListTypes,
    /// Add new case type
    AddType {
        /// Case type name
        #[arg(long)]
        name: String,
        /// Case designation
        #[arg(long)]
        designation: Option<String>,
    },
    /// Generate composite images
    GenerateComposites,
    /// Train model
    Train {
        /// Specific case types to train
        #[arg(long)]
        types: Option<Vec<String>>,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show configuration
    Show,
    /// Set configuration value
    Set {
        /// Configuration key
        key: String,
        /// Configuration value
        value: String,
    },
    /// Reset configuration to defaults
    Reset,
}

#[tokio::main]
async fn main() -> OurResult<()> {
    let cli = Cli::parse();

    // Initialize configuration
    let settings = match Settings::new() {
        Ok(settings) => settings,
        Err(e) => {
            eprintln!("Failed to load configuration: {e}");
            std::process::exit(1);
        }
    };

    // Initialize tracing
    let base_level = if cli.debug { "debug" } else { "info" };

    let default_directive = match format!("shell_sorter={base_level}").parse() {
        Ok(directive) => directive,
        Err(e) => {
            eprintln!("Failed to parse default log directive: {e}");
            std::process::exit(1);
        }
    };

    let hyper_directive = match "hyper_util::client=warn".parse() {
        Ok(directive) => directive,
        Err(e) => {
            eprintln!("Failed to parse hyper log directive: {e}");
            std::process::exit(1);
        }
    };

    let filter = EnvFilter::builder()
        .with_default_directive(default_directive)
        .from_env_lossy()
        .add_directive(hyper_directive);

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();

    if cli.debug {
        debug!("Debug mode enabled");
    }
    info!("Shell Sorter starting up");

    match cli.command {
        Commands::Machine { action } => handle_machine_command(action, &settings).await,
        Commands::Camera { action } => handle_camera_command(action, &settings).await,
        Commands::Data { action } => handle_data_command(action, &settings).await,
        Commands::Ml { action } => handle_ml_command(action, &settings).await,
        Commands::Config { action } => handle_config_command(action, &settings).await,
        Commands::Serve { host, port } => {
            let mut settings = settings.clone();
            settings.host = host;
            settings.port = port;
            start_web_server(settings, Settings::get_config_path()).await
        }
    }
}

async fn handle_machine_command(action: MachineAction, _settings: &Settings) -> OurResult<()> {
    match action {
        MachineAction::NextCase => {
            info!("Triggering next case sequence...");
            // TODO: Implement machine control
            Ok(())
        }
        MachineAction::Status => {
            info!("Machine status: Ready");
            // TODO: Implement status check
            Ok(())
        }
        MachineAction::Sensors => {
            info!("Sensor readings:");
            info!("  Case ready: false");
            info!("  Case in view: false");
            // TODO: Implement sensor reading
            Ok(())
        }
        MachineAction::Flash { state, brightness } => {
            info!("Setting flash to: {} (brightness: {:?})", state, brightness);
            // TODO: Implement flash control
            Ok(())
        }
    }
}

async fn handle_camera_command(action: CameraAction, settings: &Settings) -> OurResult<()> {
    match action {
        CameraAction::Detect => {
            info!("Detecting cameras...");
            debug!("Camera detection process starting");

            // Create HTTP client to communicate with running server
            let client = reqwest::Client::new();
            let base_url = settings.base_url();
            let detect_url = format!("{base_url}/api/cameras/detect");

            match client.get(&detect_url).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        match response.json::<serde_json::Value>().await {
                            Ok(json) => {
                                if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                                    println!("Camera detection completed!");
                                    if data.is_empty() {
                                        println!("No cameras detected");
                                    } else {
                                        println!("Detected {} camera(s):", data.len());
                                        for camera in data {
                                            if let (
                                                Some(name),
                                                Some(id),
                                                Some(hostname),
                                                Some(online),
                                            ) = (
                                                camera.get("name").and_then(|n| n.as_str()),
                                                camera.get("id").and_then(|i| i.as_str()),
                                                camera.get("hostname").and_then(|h| h.as_str()),
                                                camera.get("online").and_then(|o| o.as_bool()),
                                            ) {
                                                println!("  • {name} ({id})");
                                                println!("    Hostname: {hostname}");
                                                println!(
                                                    "    Status: {}",
                                                    if online { "Online" } else { "Offline" }
                                                );
                                                println!();
                                            }
                                        }
                                    }
                                } else {
                                    println!("No camera data found in response");
                                }
                            }
                            Err(e) => {
                                return Err(OurError::App(format!(
                                    "Failed to parse response: {e}"
                                )));
                            }
                        }
                    } else {
                        return Err(OurError::App(format!(
                            "Server returned error: {}",
                            response.status()
                        )));
                    }
                }
                Err(e) => {
                    return Err(OurError::App(format!(
                        "Failed to connect to server at {base_url}: {e}\nMake sure the server is running with: shell-sorter serve"
                    )));
                }
            }

            Ok(())
        }
        CameraAction::List => {
            info!("Listing cameras...");

            // Create HTTP client to communicate with running server
            let client = reqwest::Client::new();
            let base_url = settings.base_url();
            let cameras_url = format!("{base_url}/api/cameras");

            match client.get(&cameras_url).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        match response.json::<serde_json::Value>().await {
                            Ok(json) => {
                                if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                                    if data.is_empty() {
                                        println!("No cameras found");
                                    } else {
                                        println!("Found {} camera(s):", data.len());
                                        for camera in data {
                                            if let (
                                                Some(name),
                                                Some(id),
                                                Some(hostname),
                                                Some(online),
                                            ) = (
                                                camera.get("name").and_then(|n| n.as_str()),
                                                camera.get("id").and_then(|i| i.as_str()),
                                                camera.get("hostname").and_then(|h| h.as_str()),
                                                camera.get("online").and_then(|o| o.as_bool()),
                                            ) {
                                                println!("  • {name} ({id})");
                                                println!("    Hostname: {hostname}");
                                                println!(
                                                    "    Status: {}",
                                                    if online { "Online" } else { "Offline" }
                                                );
                                                println!();
                                            }
                                        }
                                    }
                                } else {
                                    println!("No camera data found in response");
                                }
                            }
                            Err(e) => {
                                return Err(OurError::App(format!(
                                    "Failed to parse response: {e}"
                                )));
                            }
                        }
                    } else {
                        return Err(OurError::App(format!(
                            "Server returned error: {}",
                            response.status()
                        )));
                    }
                }
                Err(e) => {
                    return Err(OurError::App(format!(
                        "Failed to connect to server at {base_url}: {e}\nMake sure the server is running with: shell-sorter serve"
                    )));
                }
            }

            Ok(())
        }
        CameraAction::Capture { session_id } => {
            info!("Capturing images...");
            debug!("Session ID: {:?}", session_id);
            // TODO: Implement image capture
            Ok(())
        }
        CameraAction::Stream { index } => {
            info!("Starting camera stream...");
            debug!("Camera index: {:?}", index);
            // TODO: Implement camera streaming
            Ok(())
        }
        CameraAction::Usb { action } => handle_usb_camera_command(action).await,
    }
}

async fn handle_usb_camera_command(action: UsbCameraAction) -> OurResult<()> {
    match action {
        UsbCameraAction::Detect => {
            info!("Detecting USB cameras with hardware identification...");

            let usb_camera_manager = start_usb_camera_manager().await?;
            let cameras = usb_camera_manager.detect_cameras().await?;

            if cameras.is_empty() {
                println!("No USB cameras detected");
            } else {
                println!("Detected {} USB camera(s):", cameras.len());
                for camera in cameras {
                    println!("  • {} ({})", camera.name, camera.hardware_id);
                    println!("    Index: {}", camera.index);
                    if let Some(vendor_id) = &camera.vendor_id {
                        println!("    Vendor ID: {vendor_id}");
                    }
                    if let Some(product_id) = &camera.product_id {
                        println!("    Product ID: {product_id}");
                    }
                    if let Some(serial) = &camera.serial_number {
                        println!("    Serial: {serial}");
                    }
                    println!(
                        "    Status: {}",
                        if camera.connected {
                            "Connected"
                        } else {
                            "Disconnected"
                        }
                    );
                    println!("    Supported formats: {}", camera.supported_formats.len());
                    for format in camera.supported_formats.iter().take(3) {
                        println!(
                            "      - {}x{}@{}fps ({})",
                            format.width, format.height, format.fps, format.format
                        );
                    }
                    if camera.supported_formats.len() > 3 {
                        println!("      ... and {} more", camera.supported_formats.len() - 3);
                    }
                    println!();
                }
            }

            Ok(())
        }
        UsbCameraAction::List => {
            info!("Listing detected USB cameras...");

            let usb_camera_manager = start_usb_camera_manager().await?;
            let cameras = usb_camera_manager.list_cameras().await?;

            if cameras.is_empty() {
                println!("No USB cameras in cache. Run 'shell-sorter camera usb detect' first.");
            } else {
                println!("Cached USB cameras:");
                for camera in cameras {
                    println!("  • {} ({})", camera.name, camera.hardware_id);
                    println!(
                        "    Status: {}",
                        if camera.connected {
                            "Connected"
                        } else {
                            "Disconnected"
                        }
                    );
                }
            }

            Ok(())
        }
        UsbCameraAction::Capture { hardware_id } => {
            info!("Capturing image from USB camera: {hardware_id}");

            let usb_camera_manager = start_usb_camera_manager().await?;

            // First detect cameras to ensure the hardware_id exists
            let cameras = usb_camera_manager.detect_cameras().await?;
            if !cameras.iter().any(|c| c.hardware_id == hardware_id) {
                return Err(OurError::App(format!("Camera not found: {hardware_id}")));
            }

            // Select and start streaming for this camera
            usb_camera_manager
                .select_cameras(vec![hardware_id.clone()])
                .await?;
            usb_camera_manager.start_streaming().await?;

            // Capture image
            match usb_camera_manager.capture_image(hardware_id.clone()).await {
                Ok(image_data) => {
                    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
                    let filename = format!(
                        "usb_capture_{}_{}.jpg",
                        hardware_id.replace(':', "_"),
                        timestamp
                    );

                    std::fs::write(&filename, image_data)
                        .map_err(|e| OurError::App(format!("Failed to save image: {e}")))?;

                    println!("Image captured and saved to: {filename}");
                }
                Err(e) => {
                    return Err(OurError::App(format!("Failed to capture image: {e}")));
                }
            }

            // Stop streaming
            usb_camera_manager.stop_streaming().await?;

            Ok(())
        }
        UsbCameraAction::Test { hardware_id } => {
            info!("Testing USB camera: {hardware_id}");

            let usb_camera_manager = start_usb_camera_manager().await?;

            // Detect cameras
            println!("1. Detecting cameras...");
            let cameras = usb_camera_manager.detect_cameras().await?;

            let camera = cameras
                .iter()
                .find(|c| c.hardware_id == hardware_id)
                .ok_or_else(|| OurError::App(format!("Camera not found: {hardware_id}")))?;

            println!("   ✓ Camera found: {}", camera.name);

            // Test camera selection
            println!("2. Selecting camera...");
            usb_camera_manager
                .select_cameras(vec![hardware_id.clone()])
                .await?;
            println!("   ✓ Camera selected");

            // Test streaming
            println!("3. Starting streaming...");
            usb_camera_manager.start_streaming().await?;
            println!("   ✓ Streaming started");

            // Test capture
            println!("4. Capturing test image...");
            match usb_camera_manager.capture_image(hardware_id.clone()).await {
                Ok(image_data) => {
                    println!("   ✓ Image captured ({} bytes)", image_data.len());
                }
                Err(e) => {
                    println!("   ✗ Capture failed: {e}");
                }
            }

            // Stop streaming
            println!("5. Stopping streaming...");
            usb_camera_manager.stop_streaming().await?;
            println!("   ✓ Streaming stopped");

            println!("\nUSB camera test completed successfully!");

            Ok(())
        }
    }
}

async fn handle_data_command(action: DataAction, _settings: &Settings) -> OurResult<()> {
    match action {
        DataAction::ListShells => {
            info!("Shell case data:");
            // TODO: Implement shell listing
            Ok(())
        }
        DataAction::Tag { session_id } => {
            info!("Tagging images...");
            debug!("Session ID: {:?}", session_id);
            // TODO: Implement image tagging
            Ok(())
        }
        DataAction::Export { format } => {
            info!("Exporting data in {} format...", format);
            // TODO: Implement data export
            Ok(())
        }
        DataAction::Import { file } => {
            info!("Importing data from file: {}", file);
            // TODO: Implement data import
            Ok(())
        }
    }
}

async fn handle_ml_command(action: MlAction, _settings: &Settings) -> OurResult<()> {
    match action {
        MlAction::ListTypes => {
            info!("Case types:");
            // TODO: Implement case type listing
            Ok(())
        }
        MlAction::AddType { name, designation } => {
            info!(
                "Adding case type: {} (designation: {:?})",
                name, designation
            );
            // TODO: Implement case type addition
            Ok(())
        }
        MlAction::GenerateComposites => {
            info!("Generating composite images...");
            debug!("Starting composite generation process");
            // TODO: Implement composite generation
            Ok(())
        }
        MlAction::Train { types } => {
            info!("Training model...");
            debug!("Training for types: {:?}", types);
            // TODO: Implement model training
            Ok(())
        }
    }
}

async fn handle_config_command(action: ConfigAction, settings: &Settings) -> OurResult<()> {
    match action {
        ConfigAction::Show => {
            println!("Configuration:");
            println!("  Host: {}", settings.host);
            println!("  Port: {}", settings.port);
            println!("  Debug: {}", settings.debug);
            println!("  Machine name: {}", settings.machine_name);
            println!("  Camera count: {}", settings.camera_count);
            println!("  ML enabled: {}", settings.ml_enabled);
            println!("  Confidence threshold: {}", settings.confidence_threshold);
            Ok(())
        }
        ConfigAction::Set { key: _, value: _ } => {
            println!("Setting configuration...");
            // TODO: Implement config setting
            Ok(())
        }
        ConfigAction::Reset => {
            println!("Resetting configuration...");
            // TODO: Implement config reset
            Ok(())
        }
    }
}

async fn start_web_server(settings: Settings, settings_filename: PathBuf) -> OurResult<()> {
    let (global_tx, _global_rx_base) = tokio::sync::broadcast::channel(1024);

    // Create the controller monitor and get a handle for communication
    let controller_monitor = ControllerMonitor::new(settings.clone(), global_tx.clone())
        .map_err(|e| OurError::App(format!("Failed to create controller monitor: {e}")))?;

    // Create the camera manager and get a handle for communication
    let camera_manager = EspCameraManager::new(
        global_tx.clone(),
        global_tx.subscribe(),
        settings.network_camera_hostnames.clone(),
    )
    .map_err(|e| OurError::App(format!("Failed to create camera manager: {e}")))?;

    // Create the USB camera manager and get a handle for communication
    let _usb_camera_handle = start_usb_camera_manager()
        .await
        .map_err(|e| OurError::App(format!("Failed to create USB camera manager: {e}")))?;

    // Spawn the controller monitor in a separate task
    tokio::spawn(async move {
        if let Err(e) = controller_monitor.run().await {
            tracing::error!("Controller monitor error: {e}");
        }
    });

    // Spawn the camera manager in a separate task
    tokio::spawn(async move {
        if let Err(e) = camera_manager.run().await {
            tracing::error!("Camera manager error: {e}");
        }
    });

    // Start the web server with all handles
    server::start_server(
        Arc::new(RwLock::new(settings)),
        settings_filename,
        global_tx.clone(),
        global_tx.subscribe(),
    )
    .await
}
