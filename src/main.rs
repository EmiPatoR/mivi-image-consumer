use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex};
use memmap2::{MmapOptions, MmapMut};
use std::fs::OpenOptions;
use std::io::ErrorKind;
use serde_json::Value;
use clap::Parser;

use eframe::egui;
use egui::{Color32, RichText, Stroke, Vec2};
use egui::StrokeKind::Inside;

#[derive(Parser, Debug)]
#[command(name = "MiVi Echography Viewer")]
#[command(about = "Displays echography frames from shared memory in real-time")]
struct Args {
    /// Name of the shared memory region
    #[arg(short, long, default_value = "ultrasound_frames")]
    shm_name: String,

    /// Format of the frames (rgb, bgr, yuv)
    #[arg(short, long, default_value = "yuv")]
    format: String,

    /// Width of the window
    #[arg(short, long, default_value_t = 1024)]
    width: usize,

    /// Height of the window
    #[arg(short, long, default_value_t = 768)]
    height: usize,

    /// Skip to latest frame rather than processing sequentially
    #[arg(short, long, default_value_t = false)]
    catch_up: bool,

    /// Dump first few frames to files for debugging
    #[arg(long, default_value_t = false)]
    dump_frames: bool,

    /// Enable verbose debug output
    #[arg(short, long, default_value_t = false)]
    verbose: bool,

    /// Reconnection delay in milliseconds
    #[arg(long, default_value_t = 1000)]
    reconnect_delay: u64,
}

// Structure to match the C++ FrameHeader with correct alignment
#[repr(C, align(8))]  // Match C++ alignas(8)
#[derive(Debug, Copy, Clone)]
struct FrameHeader {
    frame_id: u64,             // Unique frame identifier
    timestamp: u64,            // Frame timestamp (nanoseconds since epoch)
    width: u32,                // Frame width in pixels
    height: u32,               // Frame height in pixels
    bytes_per_pixel: u32,      // Bytes per pixel
    data_size: u32,            // Size of frame data in bytes
    format_code: u32,          // Format identifier code
    flags: u32,                // Additional flags
    sequence_number: u64,      // Sequence number for ordering
    metadata_offset: u32,      // Offset to JSON metadata (if present)
    metadata_size: u32,        // Size of metadata in bytes
    padding: [u64; 4],         // Reserved for future use
}

// Structure to match the C++ ControlBlock with correct alignment
#[repr(C, align(64))]  // Match C++ alignas(64)
#[derive(Debug, Copy, Clone)]
struct ControlBlock {
    write_index: u64,          // Current write position
    read_index: u64,           // Current read position
    frame_count: u64,          // Number of frames in the buffer
    total_frames_written: u64, // Total number of frames written
    total_frames_read: u64,    // Total number of frames read
    dropped_frames: u64,       // Frames dropped due to buffer full
    active: bool,              // Whether the shared memory is active
    _padding1: [u8; 7],        // Padding for alignment after bool
    last_write_time: u64,      // Timestamp of last write (ns since epoch)
    last_read_time: u64,       // Timestamp of last read (ns since epoch)
    metadata_offset: u32,      // Offset to metadata area
    metadata_size: u32,        // Size of metadata area
    flags: u32,                // Additional flags
    _padding2: [u8; 184],      // Padding to ensure proper alignment
}

// SharedMemoryReader manages access to the shared memory
struct SharedMemoryReader {
    mmap: Option<MmapMut>,        // Now optional to allow reconnection
    shm_name: String,             // Store the name for reconnection
    control_block_size: usize,
    metadata_area_size: usize,
    data_offset: usize,
    max_frames: usize,
    frame_slot_size: usize,
    last_processed_index: u64,
    verbose: bool,
    connected: bool,              // Track connection state
    last_connection_attempt: Instant, // When we last tried to connect
    last_frame_time: Instant,     // Track when we last received a frame
    no_frames_timeout: Duration,  // How long to wait before considering connection stale
}

impl SharedMemoryReader {
    pub fn new(shm_name: &str, verbose: bool) -> Result<Self, Box<dyn std::error::Error>> {
        // Try to open and initialize
        let mut reader = Self {
            mmap: None,
            shm_name: shm_name.to_string(),
            control_block_size: std::mem::size_of::<ControlBlock>(),
            metadata_area_size: 4096, // Default, will be updated
            data_offset: 0,
            max_frames: 7,    // Default, will be updated
            frame_slot_size: 0,
            last_processed_index: 0,
            verbose,
            connected: false,
            last_connection_attempt: Instant::now(),
            last_frame_time: Instant::now(),
            no_frames_timeout: Duration::from_secs(5), // 5 seconds without frames triggers reconnect
        };

        // Initial connection attempt
        reader.try_connect()?;

        Ok(reader)
    }

    pub fn try_connect(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Update last attempt time
        self.last_connection_attempt = Instant::now();

        // Open the shared memory file
        let file_path = format!("/dev/shm/{}", self.shm_name);
        if self.verbose {
            println!("Opening shared memory at: {}", file_path);
        }

        let file = match OpenOptions::new().read(true).write(true).open(&file_path) {
            Ok(f) => f,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                self.connected = false;
                return Err(format!("Shared memory region '{}' not found. Waiting for producer to start...", self.shm_name).into());
            }
            Err(e) => {
                self.connected = false;
                return Err(e.into());
            }
        };

        // Memory map the file
        let mmap = match unsafe { MmapOptions::new().map_mut(&file) } {
            Ok(m) => m,
            Err(e) => {
                self.connected = false;
                return Err(format!("Failed to map shared memory: {}", e).into());
            }
        };

        if self.verbose {
            println!("Successfully mapped shared memory, size: {} bytes", mmap.len());
        }

        // Get the control block size
        let control_block_size = std::mem::size_of::<ControlBlock>();

        // Read the control block - ensure it's within bounds
        if control_block_size > mmap.len() {
            self.connected = false;
            return Err(format!("Shared memory too small for control block: {} bytes needed, {} available",
                               control_block_size, mmap.len()).into());
        }

        let control_block_ptr = mmap.as_ptr() as *const ControlBlock;
        let control_block = unsafe { &*control_block_ptr };

        if self.verbose {
            println!("Control block read: write_index={}, read_index={}, frame_count={}",
                     control_block.write_index, control_block.read_index, control_block.frame_count);
        }

        // Verify control block is valid (active flag should be true)
        if !control_block.active {
            if self.verbose {
                println!("Warning: Shared memory exists but is not active. Producer may be initializing...");
            }
            // We'll continue anyway - it might become active soon
        }

        // Get metadata size from control block
        let metadata_area_size = control_block.metadata_size as usize;
        if self.verbose {
            println!("Metadata area size from control block: {} bytes", metadata_area_size);
        }

        // Verify metadata offset is valid
        let metadata_offset = control_block.metadata_offset as usize;
        if metadata_offset + metadata_area_size > mmap.len() {
            self.connected = false;
            return Err(format!("Invalid metadata area: offset {} + size {} exceeds shared memory size {}",
                               metadata_offset, metadata_area_size, mmap.len()).into());
        }

        // Calculate data offset (start of frame data)
        let data_offset = control_block_size + metadata_area_size;
        if self.verbose {
            println!("Calculated data offset: {} bytes", data_offset);
        }

        // Read the metadata to get frame slot size and max frames
        let metadata_slice = &mmap[metadata_offset..(metadata_offset + metadata_area_size).min(mmap.len())];
        // Find null terminator if present
        let metadata_end = metadata_slice.iter().position(|&b| b == 0).unwrap_or(metadata_slice.len());
        let metadata_str = std::str::from_utf8(&metadata_slice[..metadata_end]).unwrap_or("{}");

        if self.verbose {
            println!("Metadata: {}", metadata_str);
        }

        // Parse metadata JSON
        let metadata: Value = serde_json::from_str(metadata_str).unwrap_or_else(|_| {
            if self.verbose {
                println!("Warning: Failed to parse metadata JSON, using defaults");
            }
            serde_json::json!({})
        });

        // Extract frame_slot_size with safety checks
        let metadata_frame_slot_size = metadata["frame_slot_size"].as_u64().unwrap_or(0) as usize;
        // Validate frame slot size from metadata or use safe default
        let frame_slot_size = if metadata_frame_slot_size == 0 || metadata_frame_slot_size > 50_000_000 {
            // Calculate a reasonable default based on 4K resolution + header
            let default_size = 3840 * 2160 * 4 + std::mem::size_of::<FrameHeader>();
            if self.verbose {
                println!("Warning: Invalid frame_slot_size in metadata, using default: {} bytes", default_size);
            }
            default_size
        } else {
            metadata_frame_slot_size
        };

        // Extract max_frames with safety checks
        let metadata_max_frames = metadata["max_frames"].as_u64().unwrap_or(0) as usize;
        // Ensure max_frames is reasonable
        let max_frames_from_metadata = if metadata_max_frames == 0 || metadata_max_frames > 1000 {
            7 // Default to 7 frames if metadata value is suspicious
        } else {
            metadata_max_frames
        };

        // Calculate max frames based on available memory (as a safety check)
        let available_space = mmap.len().saturating_sub(data_offset);
        let calculated_max_frames = if frame_slot_size > 0 {
            available_space / frame_slot_size
        } else {
            0
        };

        if self.verbose {
            println!("Frame slot size from metadata: {} bytes", frame_slot_size);
            println!("Max frames from metadata: {}", max_frames_from_metadata);
            println!("Calculated max frames based on memory size: {}", calculated_max_frames);
        }

        // Use the minimum to be safe
        let max_frames = if calculated_max_frames == 0 {
            max_frames_from_metadata
        } else {
            std::cmp::min(max_frames_from_metadata, calculated_max_frames)
        };

        // Final validation
        if max_frames == 0 {
            self.connected = false;
            return Err("Invalid max_frames: cannot be zero".into());
        }

        if self.verbose {
            println!("Connected to shared memory: {}", self.shm_name);
            println!("Using max frames: {}", max_frames);
            println!("Using frame slot size: {} bytes", frame_slot_size);
        }

        // Update our state
        self.mmap = Some(mmap);
        self.control_block_size = control_block_size;
        self.metadata_area_size = metadata_area_size;
        self.data_offset = data_offset;
        self.max_frames = max_frames;
        self.frame_slot_size = frame_slot_size;
        // Reset processing index only on reconnection (not first connection)
        if !self.connected {
            self.last_processed_index = 0;
        }

        self.connected = true;
        self.last_frame_time = Instant::now(); // Reset the frame timeout

        Ok(())
    }

    // Check if we're connected to shared memory
    pub fn is_connected(&self) -> bool {
        self.connected && self.mmap.is_some()
    }

    // Check if connection is healthy (active and receiving frames)
    pub fn check_connection_health(&mut self) -> bool {
        if !self.connected || self.mmap.is_none() {
            return false;
        }

        // Check if we haven't received frames for too long
        if self.last_frame_time.elapsed() > self.no_frames_timeout {
            if self.verbose {
                println!("No frames received for {:?}, marking as disconnected", self.no_frames_timeout);
            }
            self.connected = false;
            return false;
        }

        // Check control block active flag
        let control_block_active = unsafe {
            // This is unsafe but necessary to check the control block without borrow issues
            if let Some(mmap) = &self.mmap {
                let control_block_ptr = mmap.as_ptr() as *const ControlBlock;
                (*control_block_ptr).active
            } else {
                false
            }
        };

        if !control_block_active {
            if self.verbose {
                println!("Control block marked as inactive, producer likely restarted");
            }
            self.connected = false;
            return false;
        }

        true
    }

    // Method to force reopening the connection
    pub fn reopen(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.verbose {
            println!("Attempting to reopen shared memory connection");
        }
        self.connected = false;
        self.mmap = None;
        self.try_connect()
    }

    // Get next frame in sequence, or latest if catchup is true
    pub fn get_next_frame<'a>(&'a mut self, catchup: bool) -> Result<Option<(FrameHeader, &'a [u8])>, Box<dyn std::error::Error>> {
        // Check connection first
        if !self.is_connected() {
            return Err("Not connected to shared memory".into());
        }

        // We need to do some unsafe operations to avoid borrow conflicts
        // Store a local copy of needed values
        let control_block_size = self.control_block_size;
        let data_offset = self.data_offset;
        let max_frames = self.max_frames;
        let frame_slot_size = self.frame_slot_size;
        let verbose = self.verbose;
        let last_processed_index = self.last_processed_index;

        // Get raw pointer to the mmap to avoid borrow issues
        let mmap_ptr = unsafe {
            self.mmap.as_ref().unwrap().as_ptr()
        };

        // Get mmap length
        let mmap_len = self.mmap.as_ref().unwrap().len();

        // Read the control block to get current write index
        let control_block = unsafe {
            &*(mmap_ptr as *const ControlBlock)
        };

        if verbose {
            println!("Control block: write_index={}, read_index={}, frame_count={}",
                     control_block.write_index, control_block.read_index, control_block.frame_count);
        }

        // Verify control block is still active
        if !control_block.active {
            self.connected = false;
            return Err("Shared memory is no longer active".into());
        }

        // If no new frames, return None (but still connected)
        if control_block.write_index <= last_processed_index {
            if verbose {
                println!("No new frames available (last_processed_index={})", last_processed_index);
            }
            return Ok(None);
        }

        // Determine which frame to read
        let frame_index = if catchup {
            // Just get the latest frame
            control_block.write_index - 1
        } else {
            // Get the next frame in sequence
            last_processed_index + 1
        };

        if verbose {
            println!("Reading frame at index: {}", frame_index);
        }

        // Calculate frame offset inline
        let slot_index = (frame_index as usize) % max_frames;
        let frame_offset = data_offset + slot_index * frame_slot_size;

        if verbose {
            println!("Frame offset: {} bytes", frame_offset);
        }

        // Validate frame offset is within bounds
        if frame_offset >= mmap_len {
            if verbose {
                println!("Error: Frame offset {} exceeds shared memory size {}", frame_offset, mmap_len);
            }
            self.last_processed_index = frame_index; // Skip this frame
            return Ok(None);
        }

        // Get frame header size
        let header_size = std::mem::size_of::<FrameHeader>();

        // Make sure there's enough space for the header
        if frame_offset + header_size > mmap_len {
            if verbose {
                println!("Error: Not enough space for frame header at offset {}", frame_offset);
            }
            self.last_processed_index = frame_index; // Skip this frame
            return Ok(None);
        }

        // Get frame header
        let header_ptr = unsafe {
            mmap_ptr.add(frame_offset) as *const FrameHeader
        };
        let header = unsafe { *header_ptr };

        if verbose {
            println!("Frame header: id={}, w={}, h={}, bpp={}, size={}, format={}",
                     header.frame_id, header.width, header.height,
                     header.bytes_per_pixel, header.data_size, header.format_code);
        }

        // Validate header data
        if header.width == 0 || header.height == 0 || header.data_size == 0 {
            if verbose {
                println!("Warning: Invalid frame header detected at offset {}: w={}, h={}, size={}",
                         frame_offset, header.width, header.height, header.data_size);
            }
            self.last_processed_index = frame_index; // Skip this frame
            return Ok(None);
        }

        // Get frame data
        let data_start = frame_offset + header_size;
        let data_end = data_start + header.data_size as usize;

        // Check bounds
        if data_end > mmap_len {
            if verbose {
                println!("Warning: Frame data extends beyond shared memory boundaries!");
                println!("  data_start={}, data_end={}, mmap_len={}", data_start, data_end, mmap_len);
                println!("  frame_index={}, header.data_size={}", frame_index, header.data_size);
            }
            self.last_processed_index = frame_index; // Skip this frame
            return Ok(None);
        }

        // Update the last processed index
        self.last_processed_index = frame_index;

        // Update the read index in the control block using raw pointer to avoid borrow conflicts
        unsafe {
            let control_block_mut_ptr = mmap_ptr as *mut ControlBlock;
            (*control_block_mut_ptr).read_index = frame_index + 1;
            // Also decrement frame count to reflect consumed frames
            if (*control_block_mut_ptr).frame_count > 0 {
                (*control_block_mut_ptr).frame_count -= 1;
            }
            // Update total frames read count
            (*control_block_mut_ptr).total_frames_read += 1;
        }

        // Print some debug info for the first few frames
        if frame_index < 5 && verbose {
            println!("Successfully read frame {}: size {}x{}, format {}, data size {}",
                     frame_index, header.width, header.height,
                     header.format_code, header.data_size);
        }

        // Create a slice from the raw memory - this is safe because:
        // 1. We've verified data_start and data_end are within bounds
        // 2. The mmap will remain valid for the lifetime of self
        let frame_data = unsafe {
            std::slice::from_raw_parts(
                mmap_ptr.add(data_start),
                header.data_size as usize
            )
        };

        // Update the last time we received a frame
        self.last_frame_time = Instant::now();

        // Return the frame data as a slice
        Ok(Some((header, frame_data)))
    }

    // Get statistics from the control block
    pub fn get_stats(&self) -> Result<(u64, u64, u64), Box<dyn std::error::Error>> {
        if !self.is_connected() {
            return Err("Not connected to shared memory".into());
        }

        // Use raw pointer to access the control block to avoid borrow conflicts
        let stats = unsafe {
            let mmap = self.mmap.as_ref().unwrap();
            let control_block_ptr = mmap.as_ptr() as *const ControlBlock;
            let control_block = &*control_block_ptr;

            (
                control_block.total_frames_written,
                control_block.frame_count,
                control_block.dropped_frames
            )
        };

        Ok(stats)
    }
}

// Helper function to convert format code to string
fn format_code_to_string(format_code: u32) -> &'static str {
    match format_code {
        0x01 => "YUV",
        0x02 => "BGRA/BGR",
        0x03 => "YUV10",
        0x04 => "RGB10",
        0x10 => "GRAY",
        _ => "Unknown",
    }
}

// Convert YUV frame data to RGB for display
fn convert_yuv_to_rgb(data: &[u8], width: usize, height: usize) -> Vec<Color32> {
    let mut rgb_data = vec![Color32::BLACK; width * height];

    // Simple YUV to RGB conversion for single-plane YUV
    // This is a simplified version - for real applications, proper YUV->RGB matrix conversion is needed
    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            if idx < data.len() {
                let y_value = data[idx] as u8;
                // Simple conversion: Just using Y as grayscale
                rgb_data[idx] = Color32::from_rgb(y_value, y_value, y_value);
            }
        }
    }

    rgb_data
}

// Convert BGR(A) frame data to RGB for display
fn convert_bgr_to_rgb(data: &[u8], width: usize, height: usize, bytes_per_pixel: usize) -> Vec<Color32> {
    let mut rgb_data = vec![Color32::BLACK; width * height];

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) * bytes_per_pixel;
            if idx + bytes_per_pixel <= data.len() {
                if bytes_per_pixel >= 3 {
                    let b = data[idx];
                    let g = data[idx + 1];
                    let r = data[idx + 2];
                    // Forced alpha to 255 for BGRA format
                    rgb_data[y * width + x] = Color32::from_rgb(r, g, b); // from_rgb automatically sets alpha=255
                }
            }
        }
    }

    rgb_data
}

// Convert RGB frame data to RGB for display
fn convert_rgb_to_rgb(data: &[u8], width: usize, height: usize, bytes_per_pixel: usize) -> Vec<Color32> {
    let mut rgb_data = vec![Color32::BLACK; width * height];

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) * bytes_per_pixel;
            if idx + bytes_per_pixel <= data.len() {
                if bytes_per_pixel >= 3 {
                    let r = data[idx];
                    let g = data[idx + 1];
                    let b = data[idx + 2];
                    let a = if bytes_per_pixel >= 4 { data[idx + 3] } else { 255 };
                    rgb_data[y * width + x] = Color32::from_rgba_unmultiplied(r, g, b, a);
                }
            }
        }
    }

    rgb_data
}

// Convert frame data to RGB for display based on format
fn convert_frame_to_rgb(
    data: &[u8],
    frame_width: usize,
    frame_height: usize,
    bytes_per_pixel: usize,
    format_code: u32,
    format_str: &str,
) -> Vec<Color32> {
    // Determine format based on the header's format code
    let format = match format_code {
        0x01 => "YUV",
        0x02 => {
            // Choose BGR or BGRA based on bytes_per_pixel
            if bytes_per_pixel == 3 {
                "BGR"
            } else if bytes_per_pixel == 4 {
                "BGRA"
            } else {
                "BGRA" // Default for 0x02
            }
        },
        0x03 => "YUV10",
        0x04 => "RGB10",
        0x10 => "GRAY", // Custom code for grayscale
        _ => {
            // Format code not recognized, use the format string or detect from bytes_per_pixel
            match format_str.to_lowercase().as_str() {
                "yuv" => "YUV",
                "bgr" => "BGR",
                "bgra" => "BGRA",
                "rgb" => "RGB",
                _ => {
                    // Last resort: guess from bytes_per_pixel
                    match bytes_per_pixel {
                        1 => "GRAY",
                        3 => "BGR", // Default for 3 channels is BGR in OpenCV
                        4 => "BGRA",
                        _ => "RGB" // Default fallback
                    }
                }
            }
        }
    };

    match format {
        "YUV" | "GRAY" => convert_yuv_to_rgb(data, frame_width, frame_height),
        "BGR" | "BGRA" => convert_bgr_to_rgb(data, frame_width, frame_height, bytes_per_pixel),
        "RGB" => convert_rgb_to_rgb(data, frame_width, frame_height, bytes_per_pixel),
        _ => {
            // Unknown format, default to grayscale
            convert_yuv_to_rgb(data, frame_width, frame_height)
        }
    }
}

// Application state
struct EchoViewer {
    shm_reader: Arc<Mutex<SharedMemoryReader>>,
    image_texture_id: Option<egui::TextureId>,
    frame_data: Vec<Color32>,
    frame_width: usize,
    frame_height: usize,
    connection_status: String,
    fps: f64,
    latency_ms: f64,
    format: String,
    total_frames: u64,
    last_frame_time: Instant,
    frames_received: u64,
    last_fps_update: Instant,
    catch_up: bool,
    last_connection_attempt: Instant,
    reconnect_delay: Duration,
    frame_header: Option<FrameHeader>,
    verbose: bool,
}

impl EchoViewer {
    fn new(args: Args) -> Self {
        // Try to connect to shared memory
        let shm_reader = match SharedMemoryReader::new(&args.shm_name, args.verbose) {
            Ok(reader) => {
                Arc::new(Mutex::new(reader))
            },
            Err(e) => {
                println!("Failed to connect to shared memory: {}", e);
                Arc::new(Mutex::new(SharedMemoryReader {
                    mmap: None,
                    shm_name: args.shm_name.clone(),
                    control_block_size: std::mem::size_of::<ControlBlock>(),
                    metadata_area_size: 4096,
                    data_offset: 0,
                    max_frames: 7,
                    frame_slot_size: 0,
                    last_processed_index: 0,
                    verbose: args.verbose,
                    connected: false,
                    last_connection_attempt: Instant::now(),
                    last_frame_time: Instant::now(),
                    no_frames_timeout: Duration::from_secs(5),
                }))
            }
        };

        Self {
            shm_reader,
            image_texture_id: None,
            frame_data: vec![Color32::BLACK; args.width * args.height],
            frame_width: 0,
            frame_height: 0,
            connection_status: "Disconnected - Waiting for producer".to_string(),
            fps: 0.0,
            latency_ms: 0.0,
            format: "Unknown".to_string(),
            total_frames: 0,
            last_frame_time: Instant::now(),
            frames_received: 0,
            last_fps_update: Instant::now(),
            catch_up: args.catch_up,
            last_connection_attempt: Instant::now() - Duration::from_secs(10), // Try immediately
            reconnect_delay: Duration::from_millis(args.reconnect_delay),
            frame_header: None,
            verbose: args.verbose,
        }
    }

    // Helper method to convert Color32 vec to RGBA bytes for texture
    fn frame_data_as_rgba_bytes(&self) -> Vec<u8> {
        let mut rgba_bytes = Vec::with_capacity(self.frame_width * self.frame_height * 4);
        for color in &self.frame_data {
            rgba_bytes.push(color.r());
            rgba_bytes.push(color.g());
            rgba_bytes.push(color.b());
            rgba_bytes.push(255); // Force full alpha for all pixels
        }
        rgba_bytes
    }

    fn try_connect(&mut self) {
        let mut reader = self.shm_reader.lock().unwrap();
        self.last_connection_attempt = Instant::now();

        if let Err(e) = reader.try_connect() {
            self.connection_status = format!("Disconnected - {}", e);
            if self.verbose {
                println!("Connection attempt failed: {}", e);
            }
        } else {
            self.connection_status = "Connected".to_string();
            if self.verbose {
                println!("Successfully connected to shared memory");
            }
        }
    }

    fn check_connection(&mut self) {
        // Check if we need to attempt reconnection
        if !self.shm_reader.lock().unwrap().is_connected() {
            if self.last_connection_attempt.elapsed() >= self.reconnect_delay {
                self.try_connect();
            }
            return;
        }

        // Check connection health
        let mut reader = self.shm_reader.lock().unwrap();
        if !reader.check_connection_health() {
            // Try to reopen the connection
            if let Err(e) = reader.reopen() {
                self.connection_status = format!("Disconnected - {}", e);
                if self.verbose {
                    println!("Failed to reopen connection: {}", e);
                }
            }
        }
    }

    fn update_frame(&mut self) {
        // Try to get a new frame
        let mut reader = self.shm_reader.lock().unwrap();

        match reader.get_next_frame(self.catch_up) {
            Ok(Some((header, data))) => {
                // Successfully got a frame
                self.frame_header = Some(header);
                self.frame_width = header.width as usize;
                self.frame_height = header.height as usize;

                // Calculate latency
                let now = Instant::now();

                // A simple latency calculation based on system time
                let current_time_ns = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as u64;

                // Calculate latency
                let latency_ns = if current_time_ns > header.timestamp {
                    current_time_ns - header.timestamp
                } else {
                    0 // Handle the case of clock misalignment
                };

                self.latency_ms = latency_ns as f64 / 1_000_000.0; // Convert ns to ms

                // Convert frame data to RGB for display
                self.frame_data = convert_frame_to_rgb(
                    data,
                    header.width as usize,
                    header.height as usize,
                    header.bytes_per_pixel as usize,
                    header.format_code,
                    "yuv", // Default format string
                );

                // Update format string
                self.format = format_code_to_string(header.format_code).to_string();

                // Update FPS tracking
                self.frames_received += 1;
                self.last_frame_time = now;

                // Update FPS counter every second
                if self.last_fps_update.elapsed() >= Duration::from_secs(1) {
                    self.fps = self.frames_received as f64 / self.last_fps_update.elapsed().as_secs_f64();
                    self.frames_received = 0;
                    self.last_fps_update = now;

                    // Update total frames count
                    if let Ok((total_written, _, _)) = reader.get_stats() {
                        self.total_frames = total_written;
                    }
                }

                // Update connection status
                self.connection_status = "Connected".to_string();
            },
            Ok(None) => {
                // No new frames, but still connected
            },
            Err(e) => {
                // Error reading frame - likely disconnected
                self.connection_status = format!("Connection error: {}", e);
                if self.verbose {
                    println!("Error reading frame: {}", e);
                }
            }
        }
    }
}

impl eframe::App for EchoViewer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Request a repaint for the next frame (for continuous updates)
        ctx.request_repaint();

        // Check connection and update frame
        self.check_connection();
        self.update_frame();

        // Top panel for status information
        egui::TopBottomPanel::top("status_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Connection status with color indicator
                let status_color = if self.connection_status.starts_with("Connected") {
                    Color32::from_rgb(50, 200, 70) // Green for connected
                } else {
                    Color32::from_rgb(220, 50, 50) // Red for disconnected
                };

                ui.label(RichText::new("Status:").strong());
                ui.label(RichText::new(&self.connection_status).color(status_color));

                ui.separator();

                // Frame info if we have a valid frame
                if let Some(header) = self.frame_header {
                    ui.label(RichText::new(format!("Resolution: {}x{}", header.width, header.height)).strong());
                    ui.separator();
                    ui.label(RichText::new(format!("Format: {}", self.format)).strong());
                    ui.separator();
                    ui.label(RichText::new(format!("FPS: {:.1}", self.fps)).strong());
                    ui.separator();
                    ui.label(RichText::new(format!("Latency: {:.1} ms", self.latency_ms)).strong());
                }

                // Force reconnect button
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Reconnect").clicked() {
                        self.try_connect();
                    }
                });
            });
        });

        // Bottom panel for additional stats
        egui::TopBottomPanel::bottom("stats_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(header) = self.frame_header {
                    ui.label(RichText::new(format!("Frame ID: {}", header.frame_id)).monospace());
                    ui.separator();
                    ui.label(RichText::new(format!("Sequence: {}", header.sequence_number)).monospace());
                    ui.separator();
                    ui.label(RichText::new(format!("Total Frames: {}", self.total_frames)).monospace());
                }

                // Toggle catch-up mode
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.checkbox(&mut self.catch_up, "Skip to Latest Frame");
                });
            });
        });

        // Central panel for the image
        egui::CentralPanel::default().show(ctx, |ui| {
            // If we're not connected, show a message
            if !self.shm_reader.lock().unwrap().is_connected() {
                ui.centered_and_justified(|ui| {
                    ui.add(egui::Label::new(
                        RichText::new("Waiting for Connection...")
                            .color(Color32::LIGHT_GRAY)
                            .size(24.0)
                    ));
                });
                return;
            }

            // If we have a valid frame, show it
            if self.frame_width > 0 && self.frame_height > 0 {
                // Create texture from scratch each frame (less efficient but more reliable)
                // This avoids texture update issues that might be causing problems

                // Create RGBA data directly
                let rgba_data = self.frame_data_as_rgba_bytes();

                // Create a new texture each frame
                let texture_handle = ctx.load_texture(
                    format!("frame_image_{}", self.frame_header.map_or(0, |h| h.frame_id)),
                    egui::ColorImage::from_rgba_unmultiplied(
                        [self.frame_width, self.frame_height],
                        &rgba_data
                    ),
                    egui::TextureOptions::LINEAR
                );

                // Store the new texture ID
                let texture_id = texture_handle.id();
                self.image_texture_id = Some(texture_id);

                // Calculate display size with aspect ratio preservation
                let available_size = ui.available_size();
                let image_aspect_ratio = self.frame_width as f32 / self.frame_height as f32;
                let panel_aspect_ratio = available_size.x / available_size.y;

                let display_size = if image_aspect_ratio > panel_aspect_ratio {
                    // Width constrained
                    Vec2::new(
                        available_size.x,
                        available_size.x / image_aspect_ratio
                    )
                } else {
                    // Height constrained
                    Vec2::new(
                        available_size.y * image_aspect_ratio,
                        available_size.y
                    )
                };

                // Display the image centered
                ui.centered_and_justified(|ui| {
                    // Draw a debug border to ensure image positioning is correct
                    let rect = ui.min_rect().expand2(display_size * 0.5);
                    ui.painter().rect_stroke(rect, 0.0, Stroke::new(2.0, Color32::RED), Inside);

                    // Display the image
                    ui.image((texture_id, display_size));
                });
            } else {
                // No valid frame yet
                ui.centered_and_justified(|ui| {
                    ui.add(egui::Label::new(
                        RichText::new("Waiting for Frames...")
                            .color(Color32::LIGHT_GRAY)
                            .size(24.0)
                    ));
                });
            }
        });
    }
}

fn main() -> Result<(), eframe::Error> {
    // Parse command line arguments
    let args = Args::parse();

    // Create eframe options
    let options = eframe::NativeOptions {
        centered: true,
        depth_buffer: 0,
        stencil_buffer: 0,
        multisampling: 0,
        hardware_acceleration: eframe::HardwareAcceleration::Required,
        ..Default::default()
    };

    // Run the application
    eframe::run_native(
        "Medical Echography Frame Viewer",
        options,
        Box::new(|_cc| Ok(Box::new(EchoViewer::new(args)))
    )
    )?;
    Ok(())
}