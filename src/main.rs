// src/main.rs - MiVi Medical Frame Viewer Entry Point

use std::process;
use clap::Parser;
use tracing::{info, error, warn};
use tracing_subscriber::{fmt, EnvFilter};

use mivi_frame_viewer::{
    backend::BackendConfig,
    frontend::MedicalFrameApp,
    cli::Args,
    error::MiViError,
};

/// Main entry point for MiVi Medical Frame Viewer
#[tokio::main]
async fn main() {
    // Parse command line arguments
    let args = Args::parse();

    // Initialize logging
    if let Err(e) = setup_logging(&args) {
        eprintln!("❌ Failed to setup logging: {}", e);
        process::exit(1);
    }

    // Print startup banner
    print_startup_banner();

    // Validate arguments
    if let Err(e) = validate_args(&args) {
        error!("❌ Invalid arguments: {}", e);
        process::exit(1);
    }

    // Create backend configuration
    let backend_config = create_backend_config(&args);

    // Initialize and run the application
    match run_application(backend_config).await {
        Ok(()) => {
            info!("✅ MiVi Medical Frame Viewer exited normally");
        }
        Err(e) => {
            error!("❌ Application error: {}", e);
            process::exit(1);
        }
    }
}

/// Setup logging configuration
fn setup_logging(args: &Args) -> Result<(), MiViError> {
    let log_level = if args.verbose {
        "debug"
    } else {
        "info"
    };

    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(format!("mivi_frame_viewer={}", log_level)))
        .map_err(|e| MiViError::Configuration(format!("Invalid log filter: {}", e)))?;

    // Use try_init to avoid panicking if logging is already initialized
    let _result = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .with_level(true)
        .with_ansi(true)
        .try_init();

    Ok(())
}

/// Print startup banner
fn print_startup_banner() {
    println!();
    println!("╔═══════════════════════════════════════════════════════════╗");
    println!("║                                                           ║");
    println!("║     🏥 MiVi - Medical Imaging Virtual Intelligence        ║");
    println!("║                                                           ║");
    println!("║     Professional Real-time DICOM Frame Viewer            ║");
    println!("║     Version 0.2.0 - Built with Rust & Slint             ║");
    println!("║                                                           ║");
    println!("╚═══════════════════════════════════════════════════════════╝");
    println!();

    info!("🚀 Starting MiVi Medical Frame Viewer v0.2.0");
    info!("🔧 Built with Rust and Slint UI Framework");
    info!("🏗️ Build profile: {}", if cfg!(debug_assertions) { "debug" } else { "release" });
}

/// Validate command line arguments
fn validate_args(args: &Args) -> Result<(), MiViError> {
    // Validate shared memory name
    if args.shm_name.is_empty() {
        return Err(MiViError::Configuration("Shared memory name cannot be empty".to_string()));
    }

    if args.shm_name.len() > 255 {
        return Err(MiViError::Configuration("Shared memory name too long (max 255 characters)".to_string()));
    }

    // Validate format
    let valid_formats = ["yuv", "bgr", "rgb", "rgba", "grayscale"];
    if !valid_formats.contains(&args.format.to_string().to_lowercase().as_str()) {
        return Err(MiViError::Configuration(format!(
            "Invalid format '{}'. Valid formats: {}",
            args.format,
            valid_formats.join(", ")
        )));
    }

    // Validate dimensions
    if args.width == 0 || args.height == 0 {
        return Err(MiViError::Configuration("Width and height must be greater than 0".to_string()));
    }

    if args.width > 7680 || args.height > 4320 {
        warn!("⚠️ Large frame dimensions detected: {}x{} (consider performance impact)", args.width, args.height);
    }

    // Validate reconnect delay
    if args.reconnect_delay == 0 {
        return Err(MiViError::Configuration("Reconnect delay must be greater than 0".to_string()));
    }

    if args.reconnect_delay > 60000 {
        warn!("⚠️ Very long reconnect delay: {}ms", args.reconnect_delay);
    }

    info!("✅ Command line arguments validated");
    Ok(())
}

/// Create backend configuration from command line arguments
fn create_backend_config(args: &Args) -> BackendConfig {
    info!("⚙️ Creating backend configuration");
    info!("   📂 Shared memory: {}", args.shm_name);
    info!("   🎨 Format: {}", args.format);
    info!("   📐 Dimensions: {}x{}", args.width, args.height);
    info!("   ⚡ Catch-up mode: {}", args.catch_up);
    info!("   🔄 Reconnect delay: {}ms", args.reconnect_delay);
    info!("   📝 Verbose logging: {}", args.verbose);

    BackendConfig {
        shm_name: args.shm_name.clone(),
        format: args.format.to_string(),
        width: args.width,
        height: args.height,
        catch_up: args.catch_up,
        verbose: args.verbose,
        reconnect_delay: std::time::Duration::from_millis(args.reconnect_delay),
    }
}

/// Run the main application
async fn run_application(backend_config: BackendConfig) -> Result<(), MiViError> {
    info!("🎬 Initializing MiVi Medical Frame Application");

    // Create the application
    let mut app = MedicalFrameApp::new(backend_config).await
        .map_err(|e| MiViError::Application(format!("Failed to create application: {}", e)))?;

    // Setup signal handlers for graceful shutdown
    setup_signal_handlers().await?;

    // Run the application
    info!("🏃 Running application main loop");
    app.run().await
        .map_err(|e| MiViError::Application(format!("Application runtime error: {}", e)))?;

    info!("🛑 Application shutdown complete");
    Ok(())
}

/// Setup signal handlers for graceful shutdown
async fn setup_signal_handlers() -> Result<(), MiViError> {
    #[cfg(unix)]
    {
        use tokio::signal;

        tokio::spawn(async {
            let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("Failed to setup SIGTERM handler");
            let mut sigint = signal::unix::signal(signal::unix::SignalKind::interrupt())
                .expect("Failed to setup SIGINT handler");

            tokio::select! {
                _ = sigterm.recv() => {
                    info!("📡 Received SIGTERM, initiating graceful shutdown");
                }
                _ = sigint.recv() => {
                    info!("📡 Received SIGINT (Ctrl+C), initiating graceful shutdown");
                }
            }
        });
    }

    #[cfg(windows)]
    {
        use tokio::signal;

        tokio::spawn(async {
            match signal::ctrl_c().await {
                Ok(_) => {
                    info!("📡 Received Ctrl+C, initiating graceful shutdown");
                }
                Err(e) => {
                    error!("Failed to setup Ctrl+C handler: {}", e);
                }
            }
        });
    }

    Ok(())
}