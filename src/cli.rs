// src/cli.rs - Command Line Interface for MiVi Medical Frame Viewer

use clap::{Parser, ValueEnum};
use std::path::PathBuf;

/// MiVi Medical Frame Viewer - Professional real-time DICOM frame streaming
#[derive(Parser, Debug, Clone)]
#[command(name = "MiVi Medical Frame Viewer")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "Professional real-time DICOM frame viewer with zero-latency streaming")]
#[command(long_about = r#"
MiVi - Medical Imaging Virtual Intelligence

A professional real-time DICOM frame viewer designed for medical imaging devices.
Features zero-copy frame processing, automatic reconnection, and a modern UI
built with Slint framework.

MEDICAL DEVICE COMPATIBILITY:
  - Ultrasound machines (Samsung WS80A, GE, Philips, etc.)
  - CT scanners with real-time preview
  - MRI machines with live imaging
  - Endoscopes and surgical cameras
  - Custom medical imaging devices

SUPPORTED FORMATS:
  - YUV (8-bit and 10-bit)
  - BGR/BGRA (common in medical cameras)
  - RGB/RGBA
  - Grayscale (8-bit and 16-bit)

EXAMPLES:
  # Connect to ultrasound machine
  mivi --shm-name ultrasound_frames --format yuv --catch-up

  # High-resolution CT preview
  mivi --shm-name ct_preview --format rgb --width 2048 --height 2048

  # Debug mode with verbose logging
  mivi --shm-name debug_frames --verbose --reconnect-delay 500
"#)]
pub struct Args {
    /// Name of the shared memory region
    #[arg(short = 's', long, default_value = "ultrasound_frames")]
    #[arg(help = "Shared memory region name (matches your medical device configuration)")]
    pub shm_name: String,

    /// Frame format from the medical device
    #[arg(short = 'f', long, default_value = "yuv")]
    #[arg(value_enum)]
    #[arg(help = "Frame format (yuv, bgr, rgb, rgba, grayscale)")]
    pub format: FrameFormat,

    /// Expected frame width in pixels
    #[arg(short = 'w', long, default_value_t = 1024)]
    #[arg(help = "Frame width in pixels")]
    pub width: usize,

    /// Expected frame height in pixels
    #[arg(short = 'h', long, default_value_t = 768)]
    #[arg(help = "Frame height in pixels")]
    pub height: usize,

    /// Skip to latest frame instead of processing sequentially
    #[arg(short = 'c', long, default_value_t = false)]
    #[arg(help = "Enable catch-up mode to skip to latest frame")]
    pub catch_up: bool,

    /// Enable verbose debug output
    #[arg(short = 'v', long, default_value_t = false)]
    #[arg(help = "Enable verbose logging and debug output")]
    pub verbose: bool,

    /// Reconnection delay in milliseconds
    #[arg(long, default_value_t = 1000)]
    #[arg(help = "Delay between reconnection attempts (ms)")]
    pub reconnect_delay: u64,

    /// Dump first few frames to files for debugging
    #[arg(long, default_value_t = false)]
    #[arg(help = "Save first few frames to disk for debugging")]
    pub dump_frames: bool,

    /// Maximum number of frames to dump
    #[arg(long, default_value_t = 5)]
    #[arg(help = "Maximum number of frames to dump (requires --dump-frames)")]
    pub max_dump_frames: u32,

    /// Output directory for dumped frames
    #[arg(long)]
    #[arg(help = "Directory to save dumped frames (default: current directory)")]
    pub dump_dir: Option<PathBuf>,

    /// Window width
    #[arg(long, default_value_t = 1400)]
    #[arg(help = "Initial window width")]
    pub window_width: u32,

    /// Window height
    #[arg(long, default_value_t = 900)]
    #[arg(help = "Initial window height")]
    pub window_height: u32,

    /// Start in fullscreen mode
    #[arg(long, default_value_t = false)]
    #[arg(help = "Start application in fullscreen mode")]
    pub fullscreen: bool,

    /// Disable automatic reconnection
    #[arg(long, default_value_t = false)]
    #[arg(help = "Disable automatic reconnection attempts")]
    pub no_auto_reconnect: bool,

    /// Configuration file path
    #[arg(long)]
    #[arg(help = "Load configuration from file")]
    pub config: Option<PathBuf>,

    /// Log file path
    #[arg(long)]
    #[arg(help = "Write logs to file instead of console")]
    pub log_file: Option<PathBuf>,

    /// Log level
    #[arg(long, default_value = "info")]
    #[arg(value_enum)]
    #[arg(help = "Logging level (error, warn, info, debug, trace)")]
    pub log_level: LogLevel,

    /// Performance monitoring
    #[arg(long, default_value_t = false)]
    #[arg(help = "Enable detailed performance monitoring")]
    pub perf_monitor: bool,

    /// Medical device type hint
    #[arg(long)]
    #[arg(value_enum)]
    #[arg(help = "Medical device type for optimized settings")]
    pub device_type: Option<DeviceType>,

    /// Patient ID for DICOM context
    #[arg(long)]
    #[arg(help = "Patient ID for medical context")]
    pub patient_id: Option<String>,

    /// Study description
    #[arg(long)]
    #[arg(help = "Study description for medical context")]
    pub study_description: Option<String>,

    /// Enable GPU acceleration if available
    #[arg(long, default_value_t = true)]
    #[arg(help = "Enable GPU acceleration for frame processing")]
    pub gpu_acceleration: bool,

    /// Number of processing threads
    #[arg(long)]
    #[arg(help = "Number of processing threads (default: auto-detect)")]
    pub threads: Option<usize>,
}

/// Frame format enumeration for CLI
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum FrameFormat {
    /// YUV format (common in ultrasound)
    Yuv,
    /// BGR format (common in medical cameras)
    Bgr,
    /// BGRA format with alpha channel
    Bgra,
    /// RGB format
    Rgb,
    /// RGBA format with alpha channel
    Rgba,
    /// 10-bit YUV format (high precision)
    Yuv10,
    /// 10-bit RGB format (high precision)
    Rgb10,
    /// Grayscale format
    Grayscale,
}

impl FrameFormat {
    /// Convert to backend frame format
    pub fn to_backend_format(self) -> crate::backend::types::FrameFormat {
        match self {
            FrameFormat::Yuv => crate::backend::types::FrameFormat::YUV,
            FrameFormat::Bgr => crate::backend::types::FrameFormat::BGR,
            FrameFormat::Bgra => crate::backend::types::FrameFormat::BGRA,
            FrameFormat::Rgb => crate::backend::types::FrameFormat::RGB,
            FrameFormat::Rgba => crate::backend::types::FrameFormat::RGBA,
            FrameFormat::Yuv10 => crate::backend::types::FrameFormat::YUV10,
            FrameFormat::Rgb10 => crate::backend::types::FrameFormat::RGB10,
            FrameFormat::Grayscale => crate::backend::types::FrameFormat::Grayscale,
        }
    }
}

impl std::fmt::Display for FrameFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FrameFormat::Yuv => write!(f, "yuv"),
            FrameFormat::Bgr => write!(f, "bgr"),
            FrameFormat::Bgra => write!(f, "bgra"),
            FrameFormat::Rgb => write!(f, "rgb"),
            FrameFormat::Rgba => write!(f, "rgba"),
            FrameFormat::Yuv10 => write!(f, "yuv10"),
            FrameFormat::Rgb10 => write!(f, "rgb10"),
            FrameFormat::Grayscale => write!(f, "grayscale"),
        }
    }
}

/// Log level enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LogLevel {
    /// Error level only
    Error,
    /// Warning and error levels
    Warn,
    /// Info, warning, and error levels
    Info,
    /// Debug and above levels
    Debug,
    /// All log levels (most verbose)
    Trace,
}

impl LogLevel {
    /// Convert to tracing level filter
    pub fn to_tracing_level(self) -> tracing::Level {
        match self {
            LogLevel::Error => tracing::Level::ERROR,
            LogLevel::Warn => tracing::Level::WARN,
            LogLevel::Info => tracing::Level::INFO,
            LogLevel::Debug => tracing::Level::DEBUG,
            LogLevel::Trace => tracing::Level::TRACE,
        }
    }
}

/// Medical device type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum DeviceType {
    /// Ultrasound machine
    Ultrasound,
    /// CT scanner
    Ct,
    /// MRI scanner
    Mri,
    /// X-Ray machine
    Xray,
    /// Endoscope
    Endoscope,
    /// General medical camera
    Camera,
    /// Custom device
    Custom,
}

impl DeviceType {
    /// Get optimal settings for device type
    pub fn get_optimal_settings(self) -> DeviceSettings {
        match self {
            DeviceType::Ultrasound => DeviceSettings {
                expected_fps: 30.0,
                typical_resolution: (640, 480),
                common_formats: vec![FrameFormat::Yuv, FrameFormat::Grayscale],
                latency_target_ms: 50.0,
                description: "Ultrasound machine with real-time imaging",
            },
            DeviceType::Ct => DeviceSettings {
                expected_fps: 10.0,
                typical_resolution: (512, 512),
                common_formats: vec![FrameFormat::Grayscale, FrameFormat::Rgb],
                latency_target_ms: 100.0,
                description: "CT scanner with preview capability",
            },
            DeviceType::Mri => DeviceSettings {
                expected_fps: 5.0,
                typical_resolution: (256, 256),
                common_formats: vec![FrameFormat::Grayscale],
                latency_target_ms: 200.0,
                description: "MRI scanner with real-time preview",
            },
            DeviceType::Xray => DeviceSettings {
                expected_fps: 30.0,
                typical_resolution: (1024, 1024),
                common_formats: vec![FrameFormat::Grayscale],
                latency_target_ms: 30.0,
                description: "X-Ray machine with fluoroscopy",
            },
            DeviceType::Endoscope => DeviceSettings {
                expected_fps: 60.0,
                typical_resolution: (1920, 1080),
                common_formats: vec![FrameFormat::Rgb, FrameFormat::Bgr],
                latency_target_ms: 20.0,
                description: "Surgical endoscope with HD video",
            },
            DeviceType::Camera => DeviceSettings {
                expected_fps: 30.0,
                typical_resolution: (1280, 720),
                common_formats: vec![FrameFormat::Rgb, FrameFormat::Bgr, FrameFormat::Yuv],
                latency_target_ms: 40.0,
                description: "Medical imaging camera",
            },
            DeviceType::Custom => DeviceSettings {
                expected_fps: 30.0,
                typical_resolution: (1024, 768),
                common_formats: vec![FrameFormat::Yuv, FrameFormat::Rgb],
                latency_target_ms: 50.0,
                description: "Custom medical imaging device",
            },
        }
    }

    /// Get device icon for UI
    pub fn icon(self) -> &'static str {
        match self {
            DeviceType::Ultrasound => "🔊",
            DeviceType::Ct => "🏥",
            DeviceType::Mri => "🧲",
            DeviceType::Xray => "☢️",
            DeviceType::Endoscope => "🔬",
            DeviceType::Camera => "📹",
            DeviceType::Custom => "🩺",
        }
    }
}

/// Device-specific settings
pub struct DeviceSettings {
    /// Expected frame rate
    pub expected_fps: f64,
    /// Typical resolution (width, height)
    pub typical_resolution: (u32, u32),
    /// Common formats for this device type
    pub common_formats: Vec<FrameFormat>,
    /// Target latency in milliseconds
    pub latency_target_ms: f64,
    /// Device description
    pub description: &'static str,
}

impl Args {
    /// Validate command line arguments
    pub fn validate(&self) -> Result<(), String> {
        // Validate shared memory name
        if self.shm_name.is_empty() {
            return Err("Shared memory name cannot be empty".to_string());
        }

        if self.shm_name.len() > 255 {
            return Err("Shared memory name too long (max 255 characters)".to_string());
        }

        // Validate dimensions
        if self.width == 0 || self.height == 0 {
            return Err("Width and height must be greater than 0".to_string());
        }

        if self.width > 8192 || self.height > 8192 {
            return Err("Frame dimensions too large (max 8192x8192)".to_string());
        }

        // Validate window dimensions
        if self.window_width < 800 || self.window_height < 600 {
            return Err("Window dimensions too small (min 800x600)".to_string());
        }

        // Validate reconnect delay
        if self.reconnect_delay == 0 {
            return Err("Reconnect delay must be greater than 0".to_string());
        }

        if self.reconnect_delay > 60000 {
            return Err("Reconnect delay too long (max 60 seconds)".to_string());
        }

        // Validate thread count
        if let Some(threads) = self.threads {
            if threads == 0 {
                return Err("Thread count must be greater than 0".to_string());
            }

            if threads > 32 {
                return Err("Too many threads specified (max 32)".to_string());
            }
        }

        // Validate dump frames settings
        if self.dump_frames && self.max_dump_frames == 0 {
            return Err("Max dump frames must be greater than 0 when frame dumping is enabled".to_string());
        }

        // Validate directories exist
        if let Some(ref dump_dir) = self.dump_dir {
            if !dump_dir.exists() {
                return Err(format!("Dump directory does not exist: {}", dump_dir.display()));
            }

            if !dump_dir.is_dir() {
                return Err(format!("Dump path is not a directory: {}", dump_dir.display()));
            }
        }

        if let Some(ref config_file) = self.config {
            if !config_file.exists() {
                return Err(format!("Configuration file does not exist: {}", config_file.display()));
            }
        }

        Ok(())
    }

    /// Get the effective number of processing threads
    pub fn effective_thread_count(&self) -> usize {
        self.threads.unwrap_or_else(|| {
            // Use 75% of available cores, minimum 1, maximum 8
            (num_cpus::get() * 3 / 4).max(1).min(8)
        })
    }

    /// Get dump directory or current directory
    pub fn effective_dump_dir(&self) -> PathBuf {
        self.dump_dir.clone().unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }

    /// Generate suggested window title based on settings
    pub fn generate_window_title(&self) -> String {
        let device_info = if let Some(device_type) = self.device_type {
            format!(" - {} {}", device_type.icon(), device_type.get_optimal_settings().description)
        } else {
            String::new()
        };

        format!("MiVi - {} ({}x{} @ {}){}",
                self.shm_name,
                self.width,
                self.height,
                self.format,
                device_info)
    }

    /// Print configuration summary
    pub fn print_summary(&self) {
        println!("📋 Configuration Summary:");
        println!("   🔗 Shared Memory: {}", self.shm_name);
        println!("   🎨 Format: {}", self.format);
        println!("   📐 Frame Size: {}x{}", self.width, self.height);
        println!("   🖥️ Window Size: {}x{}", self.window_width, self.window_height);
        println!("   ⚡ Catch-up Mode: {}", self.catch_up);
        println!("   🔄 Reconnect Delay: {}ms", self.reconnect_delay);
        println!("   🧵 Threads: {}", self.effective_thread_count());
        println!("   📊 Performance Monitor: {}", self.perf_monitor);
        println!("   🔧 GPU Acceleration: {}", self.gpu_acceleration);

        if let Some(device_type) = self.device_type {
            let settings = device_type.get_optimal_settings();
            println!("   🏥 Device Type: {} ({})", device_type.icon(), settings.description);
            println!("   📈 Expected FPS: {:.1}", settings.expected_fps);
            println!("   ⏱️ Target Latency: {:.1}ms", settings.latency_target_ms);
        }

        if self.dump_frames {
            println!("   💾 Frame Dumping: {} frames to {}",
                     self.max_dump_frames,
                     self.effective_dump_dir().display());
        }

        if let Some(ref patient_id) = self.patient_id {
            println!("   👤 Patient ID: {}", patient_id);
        }

        if let Some(ref study_desc) = self.study_description {
            println!("   📋 Study: {}", study_desc);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_args_validation() {
        let mut args = Args {
            shm_name: "test".to_string(),
            format: FrameFormat::Yuv,
            width: 1920,
            height: 1080,
            catch_up: false,
            verbose: false,
            reconnect_delay: 1000,
            dump_frames: false,
            max_dump_frames: 5,
            dump_dir: None,
            window_width: 1400,
            window_height: 900,
            fullscreen: false,
            no_auto_reconnect: false,
            config: None,
            log_file: None,
            log_level: LogLevel::Info,
            perf_monitor: false,
            device_type: None,
            patient_id: None,
            study_description: None,
            gpu_acceleration: true,
            threads: None,
        };

        // Valid args should pass
        assert!(args.validate().is_ok());

        // Invalid dimensions
        args.width = 0;
        assert!(args.validate().is_err());
        args.width = 1920;

        // Empty shm name
        args.shm_name = "".to_string();
        assert!(args.validate().is_err());
        args.shm_name = "test".to_string();

        // Invalid reconnect delay
        args.reconnect_delay = 0;
        assert!(args.validate().is_err());
        args.reconnect_delay = 1000;

        // Should be valid again
        assert!(args.validate().is_ok());
    }

    #[test]
    fn test_device_settings() {
        let ultrasound = DeviceType::Ultrasound;
        let settings = ultrasound.get_optimal_settings();

        assert_eq!(settings.expected_fps, 30.0);
        assert!(settings.common_formats.contains(&FrameFormat::Yuv));
        assert_eq!(ultrasound.icon(), "🔊");
    }

    #[test]
    fn test_cli_parsing() {
        // Test basic parsing
        let args = Args::try_parse_from(&[
            "mivi",
            "--shm-name", "test_shm",
            "--format", "yuv",
            "--width", "1920",
            "--height", "1080",
            "--verbose"
        ]).unwrap();

        assert_eq!(args.shm_name, "test_shm");
        assert_eq!(args.format, FrameFormat::Yuv);
        assert_eq!(args.width, 1920);
        assert_eq!(args.height, 1080);
        assert!(args.verbose);
    }
}