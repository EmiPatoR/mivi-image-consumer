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
    
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .with_level(true)
        .with_ansi(true)
        .init();
    
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
    info!("🔧 Built with Rust {} and Slint UI Framework", env!("RUSTC_VERSION", "unknown"));
    info!("📅 Build date: {}", env!("BUILD_DATE", "unknown"));
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
    if !valid_formats.contains(&args.format.to_lowercase().as_str()) {
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
        format: args.format.clone(),
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
            
            // Note: In a more complex application, you might want to send a shutdown
            // signal to the main application loop here
        });
    }
    
    #[cfg(windows)]
    {
        use tokio::signal;
        
        tokio::spawn(async {
            let mut ctrl_c = signal::ctrl_c().await.expect("Failed to setup Ctrl+C handler");
            
            info!("📡 Received Ctrl+C, initiating graceful shutdown");
        });
    }
    
    Ok(())
}

/// Print system information for debugging
#[allow(dead_code)]
fn print_system_info() {
    info!("💻 System Information:");
    info!("   OS: {}", std::env::consts::OS);
    info!("   Architecture: {}", std::env::consts::ARCH);
    info!("   CPU cores: {}", num_cpus::get());
    
    // Print memory information if available
    #[cfg(target_os = "linux")]
    {
        if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
            if let Some(line) = meminfo.lines().find(|line| line.starts_with("MemTotal:")) {
                info!("   {}", line.trim());
            }
        }
    }
    
    // Print Rust version
    info!("   Rust version: {}", env!("RUSTC_VERSION", "unknown"));
    
    // Print build information
    info!("   Build target: {}", env!("TARGET", "unknown"));
    info!("   Build profile: {}", if cfg!(debug_assertions) { "debug" } else { "release" });
    
    if cfg!(feature = "simd") {
        info!("   SIMD acceleration: enabled");
    }
    
    if cfg!(feature = "gpu") {
        info!("   GPU acceleration: enabled");
    }
}

/// Cleanup resources on exit
fn cleanup_on_exit() {
    info!("🧹 Performing cleanup on exit");
    
    // Cleanup shared memory resources if needed
    // (This would be more relevant in a C++ application with manual memory management)
    
    // Clear any temporary files
    if let Ok(temp_dir) = std::env::temp_dir().read_dir() {
        for entry in temp_dir.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.starts_with("mivi_") && name.ends_with(".tmp") {
                    if let Err(e) = std::fs::remove_file(entry.path()) {
                        warn!("Failed to remove temporary file {:?}: {}", entry.path(), e);
                    }
                }
            }
        }
    }
    
    info!("✅ Cleanup complete");
}

// Register cleanup function to run on exit
#[cfg(not(test))]
#[used]
static CLEANUP_HANDLER: fn() = cleanup_on_exit;

// For testing purposes
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_validate_args() {
        let valid_args = Args {
            shm_name: "test_shm".to_string(),
            format: "yuv".to_string(),
            width: 1920,
            height: 1080,
            catch_up: false,
            verbose: false,
            reconnect_delay: 1000,
        };
        
        assert!(validate_args(&valid_args).is_ok());
        
        // Test invalid format
        let mut invalid_args = valid_args.clone();
        invalid_args.format = "invalid".to_string();
        assert!(validate_args(&invalid_args).is_err());
        
        // Test zero dimensions
        let mut invalid_args = valid_args.clone();
        invalid_args.width = 0;
        assert!(validate_args(&invalid_args).is_err());
        
        // Test empty shm name
        let mut invalid_args = valid_args;
        invalid_args.shm_name = "".to_string();
        assert!(validate_args(&invalid_args).is_err());
    }
    
    #[test]
    fn test_create_backend_config() {
        let args = Args {
            shm_name: "test_shm".to_string(),
            format: "yuv".to_string(),
            width: 1920,
            height: 1080,
            catch_up: true,
            verbose: false,
            reconnect_delay: 2000,
        };
        
        let config = create_backend_config(&args);
        
        assert_eq!(config.shm_name, "test_shm");
        assert_eq!(config.format, "yuv");
        assert_eq!(config.width, 1920);
        assert_eq!(config.height, 1080);
        assert_eq!(config.catch_up, true);
        assert_eq!(config.reconnect_delay.as_millis(), 2000);
    }
}
