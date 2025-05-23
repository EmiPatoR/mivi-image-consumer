// src/lib.rs - MiVi Medical Frame Viewer Library

//! # MiVi - Medical Imaging Virtual Intelligence
//! 
//! A professional real-time DICOM frame viewer with zero-latency streaming capabilities.
//! Designed specifically for medical imaging devices with shared memory integration.
//! 
//! ## Features
//! 
//! - **Zero-Copy Frame Processing**: Optimized for minimal latency and maximum performance
//! - **Professional Medical UI**: Modern Slint-based interface matching web application design
//! - **Multi-Format Support**: YUV, BGR, RGB, Grayscale, and high-precision formats
//! - **Real-time Statistics**: FPS, latency, and connection monitoring
//! - **Automatic Reconnection**: Robust connection management for medical devices
//! - **Cross-Platform**: Windows, Linux, and macOS support
//! 
//! ## Architecture
//! 
//! The application is built with a clean separation between backend and frontend:
//! 
//! - **Backend**: Handles shared memory communication, frame processing, and device management
//! - **Frontend**: Manages the Slint UI, user interactions, and visual presentation
//! - **Zero-Copy Bridge**: Efficient data transfer between backend and frontend
//! 
//! ## Usage
//! 
//! ```rust
//! use mivi_frame_viewer::{
//!     backend::BackendConfig,
//!     frontend::MedicalFrameApp,
//! };
//! 
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = BackendConfig {
//!         shm_name: "ultrasound_frames".to_string(),
//!         format: "yuv".to_string(),
//!         width: 1024,
//!         height: 768,
//!         catch_up: false,
//!         verbose: false,
//!         reconnect_delay: std::time::Duration::from_secs(1),
//!     };
//!     
//!     let mut app = MedicalFrameApp::new(config).await?;
//!     app.run().await?;
//!     
//!     Ok(())
//! }
//! ```

#![doc(html_root_url = "https://docs.rs/mivi_frame_viewer/")]
#![warn(missing_docs)]
#![warn(rust_2018_idioms)]
#![warn(unreachable_pub)]

// Public modules
pub mod backend;
pub mod frontend;
pub mod cli;
pub mod error;

// Re-exports for convenience
pub use backend::{
    MedicalFrameBackend, BackendConfig, BackendCommand, BackendEvent, BackendState,
    types::{ProcessedFrame, RawFrame, FrameStatistics, ConnectionStatus},
};

pub use frontend::{
    MedicalFrameApp, SlintBridge, ImageConverter, UiState, FrontendError,
};

pub use cli::Args;
pub use error::MiViError;

/// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Build information
pub const BUILD_INFO: BuildInfo = BuildInfo {
    version: VERSION,
    git_hash: env!("GIT_HASH", "unknown"),
    build_date: env!("BUILD_DATE", "unknown"),
    rust_version: env!("RUSTC_VERSION", "unknown"),
    target: env!("TARGET", "unknown"),
    profile: if cfg!(debug_assertions) { "debug" } else { "release" },
};

/// Build information structure
#[derive(Debug, Clone)]
pub struct BuildInfo {
    /// Crate version
    pub version: &'static str,
    /// Git commit hash
    pub git_hash: &'static str,
    /// Build date
    pub build_date: &'static str,
    /// Rust compiler version
    pub rust_version: &'static str,
    /// Target triple
    pub target: &'static str,
    /// Build profile (debug/release)
    pub profile: &'static str,
}

impl BuildInfo {
    /// Get formatted build information string
    pub fn formatted(&self) -> String {
        format!(
            "MiVi v{} ({}) built on {} with Rust {} for {} ({})",
            self.version, self.git_hash, self.build_date, 
            self.rust_version, self.target, self.profile
        )
    }
}

/// Initialize the MiVi library with default settings
/// 
/// This function sets up logging and other global configurations.
/// It should be called once at the start of the application.
pub fn init() -> Result<(), MiViError> {
    // Initialize logging with default settings
    init_logging(LogLevel::Info)?;
    
    // Initialize other global state if needed
    Ok(())
}

/// Initialize logging with specified level
pub fn init_logging(level: LogLevel) -> Result<(), MiViError> {
    use tracing_subscriber::{fmt, EnvFilter};
    
    let log_level_str = match level {
        LogLevel::Trace => "trace",
        LogLevel::Debug => "debug", 
        LogLevel::Info => "info",
        LogLevel::Warn => "warn",
        LogLevel::Error => "error",
    };
    
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(format!("mivi_frame_viewer={}", log_level_str)))
        .map_err(|e| MiViError::Configuration(format!("Invalid log filter: {}", e)))?;
    
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .with_level(true)
        .with_ansi(true)
        .try_init()
        .map_err(|e| MiViError::Configuration(format!("Failed to initialize logging: {}", e)))?;
        
    Ok(())
}

/// Logging levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    /// Trace level (most verbose)
    Trace,
    /// Debug level
    Debug,
    /// Info level (default)
    Info,
    /// Warning level
    Warn,
    /// Error level (least verbose)
    Error,
}

/// Medical imaging format utilities
pub mod formats {
    use crate::backend::types::FrameFormat;
    
    /// Get all supported medical imaging formats
    pub fn supported_formats() -> Vec<FrameFormat> {
        vec![
            FrameFormat::YUV,
            FrameFormat::BGR,
            FrameFormat::BGRA,
            FrameFormat::RGB,
            FrameFormat::RGBA,
            FrameFormat::YUV10,
            FrameFormat::RGB10,
            FrameFormat::Grayscale,
        ]
    }
    
    /// Check if a format is supported
    pub fn is_supported(format: FrameFormat) -> bool {
        supported_formats().contains(&format)
    }
    
    /// Get format from string
    pub fn from_string(s: &str) -> Option<FrameFormat> {
        match s.to_lowercase().as_str() {
            "yuv" => Some(FrameFormat::YUV),
            "bgr" => Some(FrameFormat::BGR),
            "bgra" => Some(FrameFormat::BGRA),
            "rgb" => Some(FrameFormat::RGB),
            "rgba" => Some(FrameFormat::RGBA),
            "yuv10" => Some(FrameFormat::YUV10),
            "rgb10" => Some(FrameFormat::RGB10),
            "grayscale" | "gray" => Some(FrameFormat::Grayscale),
            _ => None,
        }
    }
    
    /// Get string representation of format
    pub fn to_string(format: FrameFormat) -> &'static str {
        match format {
            FrameFormat::YUV => "YUV",
            FrameFormat::BGR => "BGR",
            FrameFormat::BGRA => "BGRA",
            FrameFormat::RGB => "RGB",
            FrameFormat::RGBA => "RGBA",
            FrameFormat::YUV10 => "YUV10",
            FrameFormat::RGB10 => "RGB10",
            FrameFormat::Grayscale => "Grayscale",
            FrameFormat::Unknown => "Unknown",
        }
    }
}

/// Utility functions for medical imaging
pub mod utils {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    
    /// Convert nanoseconds timestamp to system time
    pub fn ns_to_system_time(ns: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_nanos(ns)
    }
    
    /// Get current timestamp in nanoseconds
    pub fn current_timestamp_ns() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
    }
    
    /// Calculate frame rate from frame intervals
    pub fn calculate_fps(frame_intervals: &[Duration]) -> f64 {
        if frame_intervals.is_empty() {
            return 0.0;
        }
        
        let total_time: Duration = frame_intervals.iter().sum();
        if total_time.is_zero() {
            return 0.0;
        }
        
        frame_intervals.len() as f64 / total_time.as_secs_f64()
    }
    
    /// Format bytes in human-readable form
    pub fn format_bytes(bytes: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = bytes as f64;
        let mut unit_index = 0;
        
        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }
        
        if unit_index == 0 {
            format!("{} {}", bytes, UNITS[unit_index])
        } else {
            format!("{:.1} {}", size, UNITS[unit_index])
        }
    }
    
    /// Format duration in human-readable form
    pub fn format_duration(duration: Duration) -> String {
        let total_seconds = duration.as_secs();
        
        if total_seconds < 60 {
            format!("{}s", total_seconds)
        } else if total_seconds < 3600 {
            format!("{}m {}s", total_seconds / 60, total_seconds % 60)
        } else {
            format!("{}h {}m", total_seconds / 3600, (total_seconds % 3600) / 60)
        }
    }
    
    /// Validate medical image dimensions
    pub fn validate_dimensions(width: u32, height: u32) -> Result<(), String> {
        if width == 0 || height == 0 {
            return Err("Dimensions must be greater than 0".to_string());
        }
        
        if width > 8192 || height > 8192 {
            return Err("Dimensions exceed maximum supported size (8192x8192)".to_string());
        }
        
        // Check for reasonable aspect ratios in medical imaging
        let aspect_ratio = width as f64 / height as f64;
        if aspect_ratio < 0.1 || aspect_ratio > 10.0 {
            return Err("Unusual aspect ratio detected, please verify dimensions".to_string());
        }
        
        Ok(())
    }
    
    /// Calculate expected frame size for given parameters
    pub fn calculate_frame_size(width: u32, height: u32, bytes_per_pixel: u32) -> usize {
        (width as usize) * (height as usize) * (bytes_per_pixel as usize)
    }
}

/// Performance monitoring utilities
pub mod perf {
    use std::time::{Duration, Instant};
    use std::collections::VecDeque;
    
    /// Performance monitor for tracking frame processing metrics
    #[derive(Debug)]
    pub struct PerformanceMonitor {
        frame_times: VecDeque<Instant>,
        processing_times: VecDeque<Duration>,
        max_samples: usize,
        start_time: Instant,
    }
    
    impl PerformanceMonitor {
        /// Create a new performance monitor
        pub fn new(max_samples: usize) -> Self {
            Self {
                frame_times: VecDeque::with_capacity(max_samples),
                processing_times: VecDeque::with_capacity(max_samples),
                max_samples,
                start_time: Instant::now(),
            }
        }
        
        /// Record a frame processing event
        pub fn record_frame(&mut self, processing_time: Duration) {
            let now = Instant::now();
            
            // Add new measurements
            self.frame_times.push_back(now);
            self.processing_times.push_back(processing_time);
            
            // Remove old measurements if we exceed max samples
            if self.frame_times.len() > self.max_samples {
                self.frame_times.pop_front();
                self.processing_times.pop_front();
            }
        }
        
        /// Calculate current FPS
        pub fn fps(&self) -> f64 {
            if self.frame_times.len() < 2 {
                return 0.0;
            }
            
            let time_span = self.frame_times.back().unwrap()
                .duration_since(*self.frame_times.front().unwrap());
            
            if time_span.is_zero() {
                return 0.0;
            }
            
            (self.frame_times.len() - 1) as f64 / time_span.as_secs_f64()
        }
        
        /// Calculate average processing time
        pub fn average_processing_time(&self) -> Duration {
            if self.processing_times.is_empty() {
                return Duration::ZERO;
            }
            
            let total: Duration = self.processing_times.iter().sum();
            total / self.processing_times.len() as u32
        }
        
        /// Get uptime since monitor creation
        pub fn uptime(&self) -> Duration {
            self.start_time.elapsed()
        }
        
        /// Reset all statistics
        pub fn reset(&mut self) {
            self.frame_times.clear();
            self.processing_times.clear();
            self.start_time = Instant::now();
        }
    }
}

// Tests
#[cfg(test)]
mod tests {
    use crate::backend::FrameFormat;
    use super::*;
    
    #[test]
    fn test_build_info() {
        let info = BUILD_INFO;
        assert!(!info.version.is_empty());
        assert!(!info.target.is_empty());
        assert!(info.profile == "debug" || info.profile == "release");
        
        let formatted = info.formatted();
        assert!(formatted.contains("MiVi"));
        assert!(formatted.contains(info.version));
    }
    
    #[test]
    fn test_formats() {
        use formats::*;
        
        assert!(is_supported(FrameFormat::YUV));
        assert!(is_supported(FrameFormat::RGB));
        assert!(!supported_formats().is_empty());
        
        assert_eq!(from_string("yuv"), Some(FrameFormat::YUV));
        assert_eq!(from_string("rgb"), Some(FrameFormat::RGB));
        assert_eq!(from_string("invalid"), None);
        
        assert_eq!(to_string(FrameFormat::YUV), "YUV");
        assert_eq!(to_string(FrameFormat::RGB), "RGB");
    }
    
    #[test]
    fn test_utils() {
        use utils::*;
        
        // Test validate_dimensions
        assert!(validate_dimensions(1920, 1080).is_ok());
        assert!(validate_dimensions(0, 1080).is_err());
        assert!(validate_dimensions(1920, 0).is_err());
        
        // Test format_bytes
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
        assert_eq!(format_bytes(500), "500 B");
        
        // Test calculate_frame_size
        assert_eq!(calculate_frame_size(1920, 1080, 3), 1920 * 1080 * 3);
    }
    
    #[test]
    fn test_performance_monitor() {
        use perf::PerformanceMonitor;
        use std::time::Duration;
        
        let mut monitor = PerformanceMonitor::new(100);
        
        // Initially no data
        assert_eq!(monitor.fps(), 0.0);
        assert_eq!(monitor.average_processing_time(), Duration::ZERO);
        
        // Add some measurements
        monitor.record_frame(Duration::from_millis(16));
        monitor.record_frame(Duration::from_millis(17));
        
        assert!(monitor.average_processing_time() > Duration::ZERO);
        assert!(monitor.uptime() > Duration::ZERO);
    }
}
