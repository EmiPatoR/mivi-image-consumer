// src/backend/types.rs - Data types for medical frame streaming (Zero-Copy Optimized)

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};

/// Frame header structure matching C++ implementation
#[repr(C, align(8))]
#[derive(Debug, Copy, Clone)]
pub struct FrameHeader {
    pub frame_id: u64,             // Unique frame identifier
    pub timestamp: u64,            // Frame timestamp (nanoseconds since epoch)
    pub width: u32,                // Frame width in pixels
    pub height: u32,               // Frame height in pixels
    pub bytes_per_pixel: u32,      // Bytes per pixel
    pub data_size: u32,            // Size of frame data in bytes
    pub format_code: u32,          // Format identifier code
    pub flags: u32,                // Additional flags
    pub sequence_number: u64,      // Sequence number for ordering
    pub metadata_offset: u32,      // Offset to JSON metadata (if present)
    pub metadata_size: u32,        // Size of metadata in bytes
    pub padding: [u64; 4],         // Reserved for future use
}

/// Control block structure matching C++ implementation
#[repr(C, align(64))]
#[derive(Debug, Copy, Clone)]
pub struct ControlBlock {
    pub write_index: u64,          // Current write position
    pub read_index: u64,           // Current read position
    pub frame_count: u64,          // Number of frames in the buffer
    pub total_frames_written: u64, // Total number of frames written
    pub total_frames_read: u64,    // Total number of frames read
    pub dropped_frames: u64,       // Frames dropped due to buffer full
    pub active: bool,              // Whether the shared memory is active
    pub _padding1: [u8; 7],        // Padding for alignment after bool
    pub last_write_time: u64,      // Timestamp of last write (ns since epoch)
    pub last_read_time: u64,       // Timestamp of last read (ns since epoch)
    pub metadata_offset: u32,      // Offset to metadata area
    pub metadata_size: u32,        // Size of metadata area
    pub flags: u32,                // Additional flags
    pub _padding2: [u8; 184],      // Padding to ensure proper alignment
}

/// Raw frame data from shared memory (Zero-Copy)
#[derive(Debug, Clone)]
pub struct RawFrame {
    pub header: FrameHeader,
    pub data: Arc<[u8]>,           // Zero-copy shared data
    pub metadata: Option<String>,
    pub received_at: Instant,
}

impl RawFrame {
    /// Create a new raw frame with zero-copy data
    pub fn new(header: FrameHeader, data: Arc<[u8]>, metadata: Option<String>) -> Self {
        Self {
            header,
            data,
            metadata,
            received_at: Instant::now(),
        }
    }
    
    /// Get frame format as string
    pub fn format_string(&self) -> &'static str {
        format_code_to_string(self.header.format_code)
    }
    
    /// Calculate latency in milliseconds
    pub fn latency_ms(&self) -> f64 {
        let current_time_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
            
        if current_time_ns > self.header.timestamp {
            (current_time_ns - self.header.timestamp) as f64 / 1_000_000.0
        } else {
            0.0
        }
    }
    
    /// Get resolution as string
    pub fn resolution_string(&self) -> String {
        format!("{}x{}", self.header.width, self.header.height)
    }
}

/// Processed frame ready for display (Zero-Copy optimized)
#[derive(Debug, Clone)]
pub struct ProcessedFrame {
    pub header: FrameHeader,
    pub rgb_data: Arc<[u8]>,       // Zero-copy RGB data for display
    pub metadata: Option<String>,
    pub received_at: Instant,
    pub processed_at: Instant,
    pub format: FrameFormat,
}

impl ProcessedFrame {
    /// Create a new processed frame
    pub fn new(
        header: FrameHeader,
        rgb_data: Arc<[u8]>,
        metadata: Option<String>,
        received_at: Instant,
        format: FrameFormat,
    ) -> Self {
        Self {
            header,
            rgb_data,
            metadata,
            received_at,
            processed_at: Instant::now(),
            format,
        }
    }
    
    /// Get frame dimensions
    pub fn dimensions(&self) -> (u32, u32) {
        (self.header.width, self.header.height)
    }
    
    /// Get frame format as string
    pub fn format_string(&self) -> String {
        format!("{:?}", self.format)
    }
    
    /// Get resolution as string
    pub fn resolution_string(&self) -> String {
        format!("{}x{}", self.header.width, self.header.height)
    }
    
    /// Calculate total latency (capture + processing)
    pub fn total_latency_ms(&self) -> f64 {
        let current_time_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
            
        if current_time_ns > self.header.timestamp {
            (current_time_ns - self.header.timestamp) as f64 / 1_000_000.0
        } else {
            0.0
        }
    }
    
    /// Calculate processing latency
    pub fn processing_latency_ms(&self) -> f64 {
        self.processed_at.duration_since(self.received_at).as_millis() as f64
    }
}

/// Frame format enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrameFormat {
    YUV,
    BGR,
    BGRA,
    RGB,
    RGBA,
    YUV10,
    RGB10,
    Grayscale,
    Unknown,
}

impl FrameFormat {
    /// Get bytes per pixel for this format
    pub fn bytes_per_pixel(&self) -> u32 {
        match self {
            FrameFormat::YUV | FrameFormat::Grayscale => 1,
            FrameFormat::BGR | FrameFormat::RGB => 3,
            FrameFormat::BGRA | FrameFormat::RGBA => 4,
            FrameFormat::YUV10 | FrameFormat::RGB10 => 2,
            FrameFormat::Unknown => 1,
        }
    }
    
    /// Create from format code
    pub fn from_code(code: u32) -> Self {
        match code {
            0x01 => FrameFormat::YUV,
            0x02 => FrameFormat::BGR,
            0x03 => FrameFormat::YUV10,
            0x04 => FrameFormat::RGB10,
            0x10 => FrameFormat::Grayscale,
            _ => FrameFormat::Unknown,
        }
    }
    
    /// Get format code
    pub fn to_code(&self) -> u32 {
        match self {
            FrameFormat::YUV => 0x01,
            FrameFormat::BGR => 0x02,
            FrameFormat::YUV10 => 0x03,
            FrameFormat::RGB10 => 0x04,
            FrameFormat::Grayscale => 0x10,
            _ => 0x00,
        }
    }
}

/// Frame statistics for performance monitoring
#[derive(Debug, Clone, Default)]
pub struct FrameStatistics {
    pub total_frames_received: u64,
    pub total_frames_processed: u64,
    pub frames_dropped: u64,
    pub current_fps: f64,
    pub average_latency_ms: f64,
    pub last_frame_time: Option<Instant>,
    pub fps_measurement_start: Instant,
    pub fps_frame_count: u64,
    pub latency_samples: Vec<f64>,
    pub max_latency_samples: usize,
}

impl FrameStatistics {
    /// Create new frame statistics
    pub fn new() -> Self {
        Self {
            fps_measurement_start: Instant::now(),
            max_latency_samples: 100,
            ..Default::default()
        }
    }
    
    /// Update statistics when a frame is received
    pub fn update_frame_received(&mut self) {
        self.total_frames_received += 1;
        self.fps_frame_count += 1;
        self.last_frame_time = Some(Instant::now());
    }
    
    /// Update statistics when a frame is processed
    pub fn update_frame_processed(&mut self, latency_ms: f64) {
        self.total_frames_processed += 1;
        
        // Update latency statistics
        self.latency_samples.push(latency_ms);
        if self.latency_samples.len() > self.max_latency_samples {
            self.latency_samples.remove(0);
        }
        
        // Calculate average latency
        if !self.latency_samples.is_empty() {
            self.average_latency_ms = self.latency_samples.iter().sum::<f64>() / self.latency_samples.len() as f64;
        }
    }
    
    /// Calculate current FPS
    pub fn calculate_fps(&mut self) {
        let elapsed = self.fps_measurement_start.elapsed();
        if elapsed >= Duration::from_secs(1) {
            self.current_fps = self.fps_frame_count as f64 / elapsed.as_secs_f64();
            self.fps_frame_count = 0;
            self.fps_measurement_start = Instant::now();
        }
    }
    
    /// Get maximum latency
    pub fn max_latency_ms(&self) -> f64 {
        self.latency_samples.iter().fold(0.0, |a, &b| a.max(b))
    }
    
    /// Get minimum latency
    pub fn min_latency_ms(&self) -> f64 {
        self.latency_samples.iter().fold(f64::INFINITY, |a, &b| a.min(b))
    }
    
    /// Get frame drop rate as percentage
    pub fn drop_rate_percent(&self) -> f64 {
        if self.total_frames_received > 0 {
            (self.frames_dropped as f64 / self.total_frames_received as f64) * 100.0
        } else {
            0.0
        }
    }
}

/// Medical device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub manufacturer: String,
    pub model: String,
    pub serial_number: String,
    pub software_version: String,
    pub device_type: DeviceType,
}

/// Medical device types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceType {
    Ultrasound,
    CTScan,
    MRI,
    XRay,
    Endoscope,
    Unknown,
}

impl DeviceType {
    /// Get device type icon
    pub fn icon(&self) -> &'static str {
        match self {
            DeviceType::Ultrasound => "🔊",
            DeviceType::CTScan => "🏥",
            DeviceType::MRI => "🧲",
            DeviceType::XRay => "☢️",
            DeviceType::Endoscope => "🔬",
            DeviceType::Unknown => "🩺",
        }
    }
    
    /// Get device type name
    pub fn name(&self) -> &'static str {
        match self {
            DeviceType::Ultrasound => "Ultrasound",
            DeviceType::CTScan => "CT Scanner",
            DeviceType::MRI => "MRI Scanner",
            DeviceType::XRay => "X-Ray Machine",
            DeviceType::Endoscope => "Endoscope",
            DeviceType::Unknown => "Unknown Device",
        }
    }
}

/// Patient information for DICOM context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatientInfo {
    pub patient_id: String,
    pub patient_name: String,
    pub birth_date: String,
    pub sex: String,
    pub study_description: String,
    pub modality: String,
}

/// Connection configuration
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    pub reconnect_delay: Duration,
    pub max_reconnect_attempts: u32,
    pub frame_timeout: Duration,
    pub buffer_size: usize,
    pub verbose_logging: bool,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            reconnect_delay: Duration::from_secs(1),
            max_reconnect_attempts: 10,
            frame_timeout: Duration::from_secs(5),
            buffer_size: 1024 * 1024 * 50, // 50MB buffer
            verbose_logging: false,
        }
    }
}

/// Helper function to convert format code to string
pub fn format_code_to_string(format_code: u32) -> &'static str {
    match format_code {
        0x01 => "YUV",
        0x02 => "BGR/BGRA",
        0x03 => "YUV10",
        0x04 => "RGB10",
        0x10 => "Grayscale",
        _ => "Unknown",
    }
}

/// Memory usage statistics
#[derive(Debug, Clone, Default)]
pub struct MemoryStats {
    pub shared_memory_size: usize,
    pub frame_buffer_size: usize,
    pub processed_frames_memory: usize,
    pub peak_memory_usage: usize,
}

impl MemoryStats {
    /// Update memory statistics
    pub fn update(&mut self, shm_size: usize, processed_size: usize) {
        self.shared_memory_size = shm_size;
        self.processed_frames_memory = processed_size;
        
        let total = shm_size + processed_size;
        if total > self.peak_memory_usage {
            self.peak_memory_usage = total;
        }
    }
    
    /// Get total memory usage in MB
    pub fn total_memory_mb(&self) -> f64 {
        (self.shared_memory_size + self.processed_frames_memory) as f64 / (1024.0 * 1024.0)
    }
    
    /// Get peak memory usage in MB
    pub fn peak_memory_mb(&self) -> f64 {
        self.peak_memory_usage as f64 / (1024.0 * 1024.0)
    }
}
