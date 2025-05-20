use clap::Parser;
use memmap2::{MmapMut, MmapOptions};
use serde_json::Value;
use std::arch::x86_64::*;
use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui;
use eframe::epaint::StrokeKind;
use egui::{Color32, FontId, Pos2, Rect, RichText, Stroke, TextStyle, Vec2};

// Add SIMD feature detection
#[cfg(target_arch = "x86_64")]
fn is_simd_supported() -> bool {
    is_x86_feature_detected!("sse4.1") && is_x86_feature_detected!("sse2")
}

#[cfg(not(target_arch = "x86_64"))]
fn is_simd_supported() -> bool {
    false
}

#[derive(Parser, Debug)]
#[command(name = "Medical Echography Viewer")]
#[command(about = "Displays echography frames from shared memory in real-time")]
struct Args {
    /// Name of the shared memory region
    #[arg(short, long, default_value = "ultrasound_frames")]
    shm_name: String,

    /// Format of the frames (rgb, bgr, yuv)
    #[arg(short, long, default_value = "bgra")]
    format: String,

    /// Width of the window
    #[arg(short, long, default_value_t = 1920)]
    width: usize,

    /// Height of the window
    #[arg(short, long, default_value_t = 1080)]
    height: usize,

    /// Skip to latest frame rather than processing sequentially
    #[arg(short, long, default_value_t = true)] // Default changed to true for medical use
    catch_up: bool,

    /// Dump first few frames to files for debugging
    #[arg(long, default_value_t = false)]
    dump_frames: bool,

    /// Enable verbose debug output
    #[arg(short, long, default_value_t = false)]
    verbose: bool,

    /// Reconnection delay in milliseconds
    #[arg(long, default_value_t = 500)] // Reduced delay for medical use
    reconnect_delay: u64,

    /// CPU core to pin the application to (-1 for no pinning)
    #[arg(long, default_value_t = -1)]
    cpu_core: i32,

    /// Enable high priority thread scheduling
    #[arg(long, default_value_t = true)]
    high_priority: bool,
}

// Structure to match the C++ FrameHeader with correct alignment
#[repr(C, align(8))] // Match C++ alignas(8)
#[derive(Debug, Copy, Clone)]
struct FrameHeader {
    frame_id: u64,        // Unique frame identifier
    timestamp: u64,       // Frame timestamp (nanoseconds since epoch)
    width: u32,           // Frame width in pixels
    height: u32,          // Frame height in pixels
    bytes_per_pixel: u32, // Bytes per pixel
    data_size: u32,       // Size of frame data in bytes
    format_code: u32,     // Format identifier code
    flags: u32,           // Additional flags
    sequence_number: u64, // Sequence number for ordering
    metadata_offset: u32, // Offset to JSON metadata (if present)
    metadata_size: u32,   // Size of metadata in bytes
    padding: [u64; 4],    // Reserved for future use
}

// Structure to match the C++ ControlBlock with correct alignment
#[repr(C, align(64))] // Match C++ alignas(64)
#[derive(Debug)]
struct ControlBlock {
    write_index: AtomicU64,          // Current write position
    read_index: AtomicU64,           // Current read position
    frame_count: AtomicU64,          // Number of frames in the buffer
    total_frames_written: AtomicU64, // Total number of frames written
    total_frames_read: AtomicU64,    // Total number of frames read
    dropped_frames: AtomicU64,       // Frames dropped due to buffer full
    active: AtomicBool,              // Whether the shared memory is active
    _padding1: [u8; 7],              // Padding for alignment after bool
    last_write_time: AtomicU64,      // Timestamp of last write (ns since epoch)
    last_read_time: AtomicU64,       // Timestamp of last read (ns since epoch)
    metadata_offset: u32,            // Offset to metadata area
    metadata_size: u32,              // Size of metadata area
    flags: u32,                      // Additional flags
    _padding2: [u8; 184],            // Padding to ensure proper alignment
}

// SharedMemoryReader manages access to the shared memory
struct SharedMemoryReader {
    mmap: Option<MmapMut>, // Now optional to allow reconnection
    shm_name: String,      // Store the name for reconnection
    control_block_size: usize,
    metadata_area_size: usize,
    data_offset: usize,
    max_frames: usize,
    frame_slot_size: usize,
    last_processed_index: u64,
    verbose: bool,
    connected: bool,                  // Track connection state
    last_connection_attempt: Instant, // When we last tried to connect
    last_frame_time: Instant,         // Track when we last received a frame
    no_frames_timeout: Duration,      // How long to wait before considering connection stale
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
            max_frames: 7, // Default, will be updated
            frame_slot_size: 0,
            last_processed_index: 0,
            verbose,
            connected: false,
            last_connection_attempt: Instant::now(),
            last_frame_time: Instant::now(),
            no_frames_timeout: Duration::from_secs(2), // Reduced timeout for medical use
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
                return Err(format!(
                    "Shared memory region '{}' not found. Waiting for producer to start...",
                    self.shm_name
                )
                .into());
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
            println!(
                "Successfully mapped shared memory, size: {} bytes",
                mmap.len()
            );
        }

        // Pin memory in RAM to prevent swapping (critical for medical applications)
        unsafe {
            libc::mlock(mmap.as_ptr() as *const libc::c_void, mmap.len());

            // Lock memory pages to prevent paging - additional optimization
            libc::mlockall(libc::MCL_CURRENT | libc::MCL_FUTURE);
        }

        // Get the control block size
        let control_block_size = std::mem::size_of::<ControlBlock>();

        // Read the control block - ensure it's within bounds
        if control_block_size > mmap.len() {
            self.connected = false;
            return Err(format!(
                "Shared memory too small for control block: {} bytes needed, {} available",
                control_block_size,
                mmap.len()
            )
            .into());
        }

        let control_block_ptr = mmap.as_ptr() as *const ControlBlock;
        let control_block = unsafe { &*control_block_ptr };

        if self.verbose {
            println!(
                "Control block read: write_index={}, read_index={}, frame_count={}",
                control_block
                    .write_index
                    .load(std::sync::atomic::Ordering::Relaxed),
                control_block
                    .read_index
                    .load(std::sync::atomic::Ordering::Relaxed),
                control_block
                    .frame_count
                    .load(std::sync::atomic::Ordering::Relaxed)
            );
        }

        // Verify control block is valid (active flag should be true)
        if !control_block
            .active
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            if self.verbose {
                println!(
                    "Warning: Shared memory exists but is not active. Producer may be initializing..."
                );
            }
            // We'll continue anyway - it might become active soon
        }

        // Get metadata size from control block
        let metadata_area_size = control_block.metadata_size as usize;
        if self.verbose {
            println!(
                "Metadata area size from control block: {} bytes",
                metadata_area_size
            );
        }

        // Verify metadata offset is valid
        let metadata_offset = control_block.metadata_offset as usize;
        if metadata_offset + metadata_area_size > mmap.len() {
            self.connected = false;
            return Err(format!(
                "Invalid metadata area: offset {} + size {} exceeds shared memory size {}",
                metadata_offset,
                metadata_area_size,
                mmap.len()
            )
            .into());
        }

        // Calculate data offset (start of frame data)
        let data_offset = control_block_size + metadata_area_size;
        if self.verbose {
            println!("Calculated data offset: {} bytes", data_offset);
        }

        // Read the metadata to get frame slot size and max frames
        let metadata_slice =
            &mmap[metadata_offset..(metadata_offset + metadata_area_size).min(mmap.len())];
        // Find null terminator if present
        let metadata_end = metadata_slice
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(metadata_slice.len());
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
        let frame_slot_size =
            if metadata_frame_slot_size == 0 || metadata_frame_slot_size > 50_000_000 {
                // Calculate a reasonable default based on 4K resolution + header
                let default_size = 3840 * 2160 * 4 + std::mem::size_of::<FrameHeader>();
                if self.verbose {
                    println!(
                        "Warning: Invalid frame_slot_size in metadata, using default: {} bytes",
                        default_size
                    );
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
            println!(
                "Calculated max frames based on memory size: {}",
                calculated_max_frames
            );
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
                println!(
                    "No frames received for {:?}, marking as disconnected",
                    self.no_frames_timeout
                );
            }
            self.connected = false;
            return false;
        }

        // Check control block active flag
        let control_block_active = unsafe {
            // This is unsafe but necessary to check the control block without borrow issues
            if let Some(mmap) = &self.mmap {
                let control_block_ptr = mmap.as_ptr() as *const ControlBlock;
                (*control_block_ptr)
                    .active
                    .load(std::sync::atomic::Ordering::Acquire)
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

    // Zero-copy optimized frame reading with memory prefetching
    pub fn get_next_frame<'a>(
        &'a mut self,
        catchup: bool,
    ) -> Result<Option<(FrameHeader, &'a [u8])>, Box<dyn std::error::Error>> {
        if !self.is_connected() {
            return Err("Not connected to shared memory".into());
        }

        // Fast path: get all atomic data in one go with proper ordering
        let (write_index, control_block_ptr, mmap_ptr, mmap_len) = unsafe {
            let mmap = self.mmap.as_ref().unwrap();
            let control_block_ptr = mmap.as_ptr() as *const ControlBlock;
            let write_index = (*control_block_ptr)
                .write_index
                .load(std::sync::atomic::Ordering::Acquire);
            (write_index, control_block_ptr, mmap.as_ptr(), mmap.len())
        };

        // No new frames available
        if write_index <= self.last_processed_index {
            return Ok(None);
        }

        // Determine which frame to read - immediate catch-up for medical applications
        let frame_index = if catchup {
            // Just get the latest frame for minimal latency
            write_index - 1
        } else {
            // Get the next frame in sequence
            self.last_processed_index + 1
        };

        // Calculate frame offset with minimal logic
        let slot_index = (frame_index as usize) % self.max_frames;
        let frame_offset = self.data_offset + slot_index * self.frame_slot_size;

        // Range check
        if frame_offset >= mmap_len {
            self.last_processed_index = frame_index;
            return Ok(None);
        }

        // Get frame header directly from memory
        let header_size = std::mem::size_of::<FrameHeader>();
        if frame_offset + header_size > mmap_len {
            self.last_processed_index = frame_index;
            return Ok(None);
        }

        // Get header with a single dereference
        let header = unsafe { *(mmap_ptr.add(frame_offset) as *const FrameHeader) };

        // Fast validation of critical fields
        if header.width == 0 || header.height == 0 || header.data_size == 0 {
            self.last_processed_index = frame_index;
            return Ok(None);
        }

        // Get frame data as a direct slice - TRUE ZERO COPY
        let data_start = frame_offset + header_size;
        let data_end = data_start + header.data_size as usize;

        if data_end > mmap_len {
            self.last_processed_index = frame_index;
            return Ok(None);
        }

        // Create slice directly from shared memory - no copying
        let frame_data = unsafe {
            std::slice::from_raw_parts(mmap_ptr.add(data_start), header.data_size as usize)
        };

        // OPTIMIZATION: Prefetch the next frame's header to reduce latency
        #[cfg(target_arch = "x86_64")]
        unsafe {
            if is_simd_supported() {
                let next_slot_index = ((frame_index + 1) as usize) % self.max_frames;
                let next_frame_offset = self.data_offset + next_slot_index * self.frame_slot_size;

                if next_frame_offset < mmap_len {
                    // Use prefetch hint for next frame with compile-time constant parameter
                    _mm_prefetch::<_MM_HINT_T0>(mmap_ptr.add(next_frame_offset) as *const i8);
                }
            }
        }

        // Update indices atomically with proper memory ordering
        self.last_processed_index = frame_index;

        unsafe {
            // Update the read index in the control block
            (*control_block_ptr)
                .read_index
                .store(frame_index + 1, std::sync::atomic::Ordering::Release);

            // Update frame count atomically
            let frame_count = (*control_block_ptr)
                .frame_count
                .load(std::sync::atomic::Ordering::Acquire);
            if frame_count > 0 {
                (*control_block_ptr)
                    .frame_count
                    .store(frame_count - 1, std::sync::atomic::Ordering::Release);
            }

            // Update read stats counter
            (*control_block_ptr)
                .total_frames_read
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            (*control_block_ptr).last_read_time.store(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as u64,
                std::sync::atomic::Ordering::Relaxed,
            );
        }

        // Update timestamp
        self.last_frame_time = Instant::now();

        Ok(Some((header, frame_data)))
    }

    // Get statistics from the control block - optimized to read once
    pub fn get_stats(&self) -> Result<(u64, u64, u64), Box<dyn std::error::Error>> {
        if !self.is_connected() {
            return Err("Not connected to shared memory".into());
        }

        // Use a single unsafe block to get all stats at once
        let stats = unsafe {
            let mmap = self.mmap.as_ref().unwrap();
            let control_block_ptr = mmap.as_ptr() as *const ControlBlock;

            (
                (*control_block_ptr)
                    .total_frames_written
                    .load(std::sync::atomic::Ordering::Relaxed),
                (*control_block_ptr)
                    .frame_count
                    .load(std::sync::atomic::Ordering::Relaxed),
                (*control_block_ptr)
                    .dropped_frames
                    .load(std::sync::atomic::Ordering::Relaxed),
            )
        };

        Ok(stats)
    }
}

// Helper function to convert format code to string
fn format_code_to_string(format_code: u32) -> &'static str {
    match format_code {
        0x01 => "YUV",
        0x02 => "BGRA",
        0x03 => "YUV10",
        0x04 => "RGB10",
        0x10 => "GRAY",
        _ => "Unknown",
    }
}

// SIMD optimized BGRA to RGB conversion
#[cfg(target_arch = "x86_64")]
unsafe fn convert_bgra_to_rgb_simd(data: &[u8], width: usize, height: usize) -> Vec<Color32> {
    let mut rgb_data = vec![Color32::BLACK; width * height];

    // Only use SIMD if we have sufficient data
    if data.len() >= 16 && width >= 4 {
        let pixels_per_iteration = 4; // Process 4 pixels (16 bytes) at once
        let stride = width * 4; // 4 bytes per pixel for BGRA

        // SIMD shuffle mask for BGRA -> RGBA conversion
        let shuffle_mask = _mm_set_epi8(15, 12, 13, 14, 11, 8, 9, 10, 7, 4, 5, 6, 3, 0, 1, 2);

        for y in 0..height {
            let row_offset = y * stride;
            let mut x = 0;

            // Process chunks of 4 pixels with SIMD
            while x + pixels_per_iteration <= width {
                let offset = row_offset + x * 4;

                if offset + 16 <= data.len() {
                    // Load 16 bytes (4 BGRA pixels)
                    let bgra = _mm_loadu_si128(data.as_ptr().add(offset) as *const __m128i);

                    // Shuffle to convert BGRA to RGBA
                    let rgba = _mm_shuffle_epi8(bgra, shuffle_mask);

                    // First pixel (B,G,R,A at indices 0,1,2,3)
                    let r0 = _mm_extract_epi8::<0>(rgba) as u8;
                    let g0 = _mm_extract_epi8::<1>(rgba) as u8;
                    let b0 = _mm_extract_epi8::<2>(rgba) as u8;
                    rgb_data[y * width + x] = Color32::from_rgb(r0, g0, b0);

                    // Second pixel (B,G,R,A at indices 4,5,6,7)
                    let r1 = _mm_extract_epi8::<4>(rgba) as u8;
                    let g1 = _mm_extract_epi8::<5>(rgba) as u8;
                    let b1 = _mm_extract_epi8::<6>(rgba) as u8;
                    rgb_data[y * width + x + 1] = Color32::from_rgb(r1, g1, b1);

                    // Third pixel (B,G,R,A at indices 8,9,10,11)
                    let r2 = _mm_extract_epi8::<8>(rgba) as u8;
                    let g2 = _mm_extract_epi8::<9>(rgba) as u8;
                    let b2 = _mm_extract_epi8::<10>(rgba) as u8;
                    rgb_data[y * width + x + 2] = Color32::from_rgb(r2, g2, b2);

                    // Fourth pixel (B,G,R,A at indices 12,13,14,15)
                    let r3 = _mm_extract_epi8::<12>(rgba) as u8;
                    let g3 = _mm_extract_epi8::<13>(rgba) as u8;
                    let b3 = _mm_extract_epi8::<14>(rgba) as u8;
                    rgb_data[y * width + x + 3] = Color32::from_rgb(r3, g3, b3);
                } else {
                    // Handle edge case for last pixels
                    for i in 0..pixels_per_iteration {
                        let idx = offset + i * 4;
                        if idx + 3 < data.len() {
                            let b = data[idx];
                            let g = data[idx + 1];
                            let r = data[idx + 2];
                            rgb_data[y * width + x + i] = Color32::from_rgb(r, g, b);
                        }
                    }
                }

                x += pixels_per_iteration;
            }

            // Handle remaining pixels with scalar code
            while x < width {
                let idx = row_offset + x * 4;
                if idx + 3 < data.len() {
                    let b = data[idx];
                    let g = data[idx + 1];
                    let r = data[idx + 2];
                    rgb_data[y * width + x] = Color32::from_rgb(r, g, b);
                }
                x += 1;
            }
        }
    } else {
        // Fall back to scalar implementation
        let stride = width * 4;
        for y in 0..height {
            let row_offset = y * stride;
            for x in 0..width {
                let idx = row_offset + x * 4;
                if idx + 3 < data.len() {
                    let b = data[idx];
                    let g = data[idx + 1];
                    let r = data[idx + 2];
                    rgb_data[y * width + x] = Color32::from_rgb(r, g, b);
                }
            }
        }
    }

    rgb_data
}

// Convert YUV frame data to RGB for display (scalar implementation)
fn convert_yuv_to_rgb(data: &[u8], width: usize, height: usize) -> Vec<Color32> {
    // Check if we can use SIMD
    #[cfg(target_arch = "x86_64")]
    if is_x86_feature_detected!("avx2") && width >= 16 {
        unsafe {
            return convert_yuv_to_rgb_simd_avx2(data, width, height);
        }
    }

    // Fallback scalar implementation
    let mut rgb_data = vec![Color32::BLACK; width * height];
    let stride = width; // YUV is often packed

    for y in 0..height {
        for x in 0..width {
            let idx = y * stride + x;
            if idx < data.len() {
                let y_value = data[idx];
                rgb_data[y * width + x] = Color32::from_rgb(y_value, y_value, y_value);
            }
        }
    }

    rgb_data
}

// SIMD optimized YUV to RGB conversion using AVX2
#[cfg(target_arch = "x86_64")]
unsafe fn convert_yuv_to_rgb_simd_avx2(data: &[u8], width: usize, height: usize) -> Vec<Color32> {
    let mut rgb_data = vec![Color32::BLACK; width * height];
    let stride = width;

    // Process 16 pixels at once with AVX2
    let pixels_per_iteration = 16;

    for y in 0..height {
        let row_offset = y * stride;
        let mut x = 0;

        // Process chunks of 16 pixels with AVX2
        while x + pixels_per_iteration <= width {
            let offset = row_offset + x;

            if offset + pixels_per_iteration <= data.len() {
                // Load 16 Y values
                let y_values = _mm256_loadu_si256(data.as_ptr().add(offset) as *const __m256i);

                // Store pixels one by one - using compile-time constants for extraction
                let y0 = _mm256_extract_epi8::<0>(y_values) as u8;
                rgb_data[y * width + x] = Color32::from_rgb(y0, y0, y0);

                let y1 = _mm256_extract_epi8::<1>(y_values) as u8;
                rgb_data[y * width + x + 1] = Color32::from_rgb(y1, y1, y1);

                let y2 = _mm256_extract_epi8::<2>(y_values) as u8;
                rgb_data[y * width + x + 2] = Color32::from_rgb(y2, y2, y2);

                let y3 = _mm256_extract_epi8::<3>(y_values) as u8;
                rgb_data[y * width + x + 3] = Color32::from_rgb(y3, y3, y3);

                let y4 = _mm256_extract_epi8::<4>(y_values) as u8;
                rgb_data[y * width + x + 4] = Color32::from_rgb(y4, y4, y4);

                let y5 = _mm256_extract_epi8::<5>(y_values) as u8;
                rgb_data[y * width + x + 5] = Color32::from_rgb(y5, y5, y5);

                let y6 = _mm256_extract_epi8::<6>(y_values) as u8;
                rgb_data[y * width + x + 6] = Color32::from_rgb(y6, y6, y6);

                let y7 = _mm256_extract_epi8::<7>(y_values) as u8;
                rgb_data[y * width + x + 7] = Color32::from_rgb(y7, y7, y7);

                let y8 = _mm256_extract_epi8::<8>(y_values) as u8;
                rgb_data[y * width + x + 8] = Color32::from_rgb(y8, y8, y8);

                let y9 = _mm256_extract_epi8::<9>(y_values) as u8;
                rgb_data[y * width + x + 9] = Color32::from_rgb(y9, y9, y9);

                let y10 = _mm256_extract_epi8::<10>(y_values) as u8;
                rgb_data[y * width + x + 10] = Color32::from_rgb(y10, y10, y10);

                let y11 = _mm256_extract_epi8::<11>(y_values) as u8;
                rgb_data[y * width + x + 11] = Color32::from_rgb(y11, y11, y11);

                let y12 = _mm256_extract_epi8::<12>(y_values) as u8;
                rgb_data[y * width + x + 12] = Color32::from_rgb(y12, y12, y12);

                let y13 = _mm256_extract_epi8::<13>(y_values) as u8;
                rgb_data[y * width + x + 13] = Color32::from_rgb(y13, y13, y13);

                let y14 = _mm256_extract_epi8::<14>(y_values) as u8;
                rgb_data[y * width + x + 14] = Color32::from_rgb(y14, y14, y14);

                let y15 = _mm256_extract_epi8::<15>(y_values) as u8;
                rgb_data[y * width + x + 15] = Color32::from_rgb(y15, y15, y15);
            } else {
                // Handle edge case
                for i in 0..pixels_per_iteration {
                    let idx = offset + i;
                    if idx < data.len() {
                        let y_value = data[idx];
                        rgb_data[y * width + x + i] = Color32::from_rgb(y_value, y_value, y_value);
                    }
                }
            }

            x += pixels_per_iteration;
        }

        // Handle remaining pixels
        while x < width {
            let idx = row_offset + x;
            if idx < data.len() {
                let y_value = data[idx];
                rgb_data[y * width + x] = Color32::from_rgb(y_value, y_value, y_value);
            }
            x += 1;
        }
    }

    rgb_data
}

// High-performance BGRA to RGB conversion optimized for medical imaging (scalar implementation)
fn convert_bgr_to_rgb(
    data: &[u8],
    width: usize,
    height: usize,
    bytes_per_pixel: usize,
) -> Vec<Color32> {
    // Use SIMD for BGRA format when available
    #[cfg(target_arch = "x86_64")]
    if bytes_per_pixel == 4 && is_simd_supported() && width * height > 1000 {
        unsafe {
            return convert_bgra_to_rgb_simd(data, width, height);
        }
    }

    // Pre-allocate with capacity to avoid reallocation
    let mut rgb_data = vec![Color32::BLACK; width * height];

    // Process row by row to maximize cache efficiency
    let stride = width * bytes_per_pixel;

    for y in 0..height {
        let row_offset = y * stride;

        for x in 0..width {
            let idx = row_offset + x * bytes_per_pixel;

            if idx + bytes_per_pixel <= data.len() {
                if bytes_per_pixel >= 3 {
                    let b = data[idx];
                    let g = data[idx + 1];
                    let r = data[idx + 2];
                    // Always use full opacity for medical imaging clarity
                    rgb_data[y * width + x] = Color32::from_rgb(r, g, b);
                }
            }
        }
    }

    rgb_data
}

// Convert frame data to RGB for display based on format - unified function with SIMD dispatch
fn convert_frame_to_rgb(
    data: &[u8],
    frame_width: usize,
    frame_height: usize,
    bytes_per_pixel: usize,
    format_code: u32,
    _format_str: &str, // Prefix with underscore to indicate intentionally unused
) -> Vec<Color32> {
    // Direct SIMD dispatch for known formats
    match format_code {
        0x02 => {
            // BGRA format
            #[cfg(target_arch = "x86_64")]
            if is_simd_supported() && bytes_per_pixel == 4 {
                unsafe {
                    return convert_bgra_to_rgb_simd(data, frame_width, frame_height);
                }
            }
            // Otherwise fall back to scalar
            convert_bgr_to_rgb(data, frame_width, frame_height, bytes_per_pixel)
        }
        0x01 => {
            // YUV format
            #[cfg(target_arch = "x86_64")]
            if is_x86_feature_detected!("avx2") {
                unsafe {
                    return convert_yuv_to_rgb_simd_avx2(data, frame_width, frame_height);
                }
            }
            // Otherwise fall back to scalar
            convert_yuv_to_rgb(data, frame_width, frame_height)
        }
        0x03 => convert_yuv_to_rgb(data, frame_width, frame_height), // YUV10 simplified
        0x10 => convert_yuv_to_rgb(data, frame_width, frame_height), // GRAY as YUV
        _ => {
            // Format not explicitly handled, try to determine from bytes per pixel
            match bytes_per_pixel {
                1 => convert_yuv_to_rgb(data, frame_width, frame_height), // Grayscale
                3 => convert_bgr_to_rgb(data, frame_width, frame_height, bytes_per_pixel), // BGR
                4 => convert_bgr_to_rgb(data, frame_width, frame_height, bytes_per_pixel), // BGRA
                _ => convert_yuv_to_rgb(data, frame_width, frame_height), // Default fallback
            }
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
    gpu_buffer: Vec<u8>,
    process_time_us: u64,
    texture_time_us: u64,
    // UI
    show_info_panel: bool,
    show_tools_panel: bool,
    brightness: f32,
    contrast: f32,
    zoom_level: f32,
    region_of_interest: Option<Rect>,
    roi_active: bool,
    roi_start: Option<Pos2>,
    roi_end: Option<Pos2>,
    selected_tool: Tool,
    measurements: Vec<Measurement>,
    patient_info: PatientInfo,
    theme: Theme,
    show_grid: bool,
    show_rulers: bool,
    annotation_text: String,
    annotations: Vec<Annotation>,
}

// Enums and structs for UI state
#[derive(PartialEq)]
enum Tool {
    View,
    Zoom,
    Pan,
    ROI,
    Measure,
    Annotate,
}

enum Theme {
    Light,
    Dark,
    HighContrast,
}

struct Measurement {
    start: Pos2,
    end: Pos2,
    label: String,
}

struct Annotation {
    position: Pos2,
    text: String,
}

struct PatientInfo {
    id: String,
    name: String,
    dob: String,
    study_date: String,
    modality: String,
}

impl Default for PatientInfo {
    fn default() -> Self {
        Self {
            id: "ID12345".to_string(),
            name: "[Patient Name]".to_string(),
            dob: "YYYY-MM-DD".to_string(),
            study_date: "2025-05-20".to_string(),
            modality: "Echography".to_string(),
        }
    }
}

impl EchoViewer {
    fn new(args: Args) -> Self {
        // Try to connect to shared memory
        let shm_reader = match SharedMemoryReader::new(&args.shm_name, args.verbose) {
            Ok(reader) => Arc::new(Mutex::new(reader)),
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
                    no_frames_timeout: Duration::from_secs(2),
                }))
            }
        };

        // Apply CPU pinning if requested
        if args.cpu_core >= 0 {
            unsafe {
                let mut cpu_set: libc::cpu_set_t = std::mem::zeroed();
                libc::CPU_ZERO(&mut cpu_set);
                libc::CPU_SET(args.cpu_core as usize, &mut cpu_set);

                libc::pthread_setaffinity_np(
                    libc::pthread_self(),
                    std::mem::size_of::<libc::cpu_set_t>(),
                    &cpu_set,
                );

                println!("Application pinned to CPU core {}", args.cpu_core);
            }
        }

        // Set high priority for UI thread if requested
        if args.high_priority {
            unsafe {
                let mut sched_param: libc::sched_param = std::mem::zeroed();
                sched_param.sched_priority = 90; // High priority
                let result =
                    libc::pthread_setschedparam(libc::pthread_self(), libc::SCHED_RR, &sched_param);

                if result == 0 {
                    println!("Thread priority set to high (SCHED_RR, 90)");

                    // New: Set I/O priority to real-time
                    let io_result = libc::syscall(libc::SYS_ioprio_set, 1, 0, 4 << 13);
                    if io_result == 0 {
                        println!("I/O priority set to real-time");
                    }
                } else {
                    println!("Failed to set thread priority, error: {}", result);
                }
            }
        }

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
            gpu_buffer: Vec::new(),
            process_time_us: 0,
            texture_time_us: 0,
            // UI
            show_info_panel: true,
            show_tools_panel: true,
            brightness: 0.0,
            contrast: 0.0,
            zoom_level: 1.0,
            region_of_interest: None,
            roi_active: false,
            roi_start: None,
            roi_end: None,
            selected_tool: Tool::View,
            measurements: Vec::new(),
            patient_info: PatientInfo::default(),
            theme: Theme::Dark,
            show_grid: false,
            show_rulers: true,
            annotation_text: String::new(),
            annotations: Vec::new(),
        }
    }

    // Configure UI styles at startup
    fn configure_styles(&self, ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();

        // Configure text styles
        style.text_styles = [
            (
                TextStyle::Heading,
                FontId::new(24.0, egui::FontFamily::Proportional),
            ),
            (
                TextStyle::Body,
                FontId::new(16.0, egui::FontFamily::Proportional),
            ),
            (
                TextStyle::Monospace,
                FontId::new(14.0, egui::FontFamily::Monospace),
            ),
            (
                TextStyle::Button,
                FontId::new(16.0, egui::FontFamily::Proportional),
            ),
            (
                TextStyle::Small,
                FontId::new(12.0, egui::FontFamily::Proportional),
            ),
        ]
        .into();

        // Set colors for a professional medical application
        match self.theme {
            Theme::Dark => {
                // Dark theme
                style.visuals.dark_mode = true;
                style.visuals.panel_fill = Color32::from_rgb(22, 25, 37); // Dark blue-gray
                style.visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(30, 34, 46);
                style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(40, 44, 56);
                style.visuals.widgets.active.bg_fill = Color32::from_rgb(48, 107, 185); // Medical blue
                style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(58, 117, 195);
                style.visuals.window_fill = Color32::from_rgb(22, 25, 37);
                style.visuals.window_stroke = Stroke::new(1.0, Color32::from_rgb(40, 44, 56));
            }
            Theme::Light => {
                // Light theme
                style.visuals.dark_mode = false;
                style.visuals.panel_fill = Color32::from_rgb(240, 244, 248); // Light blue-gray
                style.visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(230, 236, 242);
                style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(220, 228, 236);
                style.visuals.widgets.active.bg_fill = Color32::from_rgb(70, 130, 210);
                style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(90, 150, 230);
                style.visuals.window_fill = Color32::from_rgb(240, 244, 248);
                style.visuals.window_stroke = Stroke::new(1.0, Color32::from_rgb(200, 210, 220));
            }
            Theme::HighContrast => {
                // High contrast theme
                style.visuals.dark_mode = true;
                style.visuals.panel_fill = Color32::BLACK;
                style.visuals.widgets.noninteractive.bg_fill = Color32::BLACK;
                style.visuals.widgets.inactive.bg_fill = Color32::DARK_GRAY;
                style.visuals.widgets.active.bg_fill = Color32::WHITE;
                style.visuals.widgets.hovered.bg_fill = Color32::LIGHT_GRAY;
                style.visuals.window_fill = Color32::BLACK;
                style.visuals.window_stroke = Stroke::new(2.0, Color32::WHITE);
            }
        }

        // Add button rounding
        style.visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(4);
        style.visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(4);
        style.visuals.widgets.active.corner_radius = egui::CornerRadius::same(4);
        style.visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(4);

        // Set window rounding
        style.visuals.window_corner_radius = egui::CornerRadius::same(6);
        style.visuals.popup_shadow.spread = 8;

        // Apply the style
        ctx.set_style(style);
    }

    // Optimized method to update or create texture with minimal allocations
    fn update_or_create_texture(&mut self, ctx: &egui::Context) -> Option<egui::TextureId> {
        if self.frame_width == 0 || self.frame_height == 0 || self.frame_data.is_empty() {
            return None;
        }

        // Start texture creation timer
        let texture_start = Instant::now();

        // Reuse the GPU buffer if possible to avoid allocation
        let buffer_size = self.frame_width * self.frame_height * 4;
        if self.gpu_buffer.len() != buffer_size {
            self.gpu_buffer = vec![0u8; buffer_size];
        }

        // Fast path: direct memory copy from frame_data to gpu_buffer
        let mut idx = 0;
        for color in &self.frame_data {
            self.gpu_buffer[idx] = color.r();
            self.gpu_buffer[idx + 1] = color.g();
            self.gpu_buffer[idx + 2] = color.b();
            self.gpu_buffer[idx + 3] = 255; // Force full alpha
            idx += 4;
        }

        // Create or update texture
        let texture_handle = ctx.load_texture(
            "frame_image",
            egui::ColorImage::from_rgba_unmultiplied(
                [self.frame_width, self.frame_height],
                &self.gpu_buffer,
            ),
            egui::TextureOptions::LINEAR,
        );

        // Record texture processing time
        self.texture_time_us = texture_start.elapsed().as_micros() as u64;

        Some(texture_handle.id())
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
        // Start frame processing timer
        let process_start = Instant::now();

        // Try to get a new frame with minimal latency
        let mut reader = self.shm_reader.lock().unwrap();

        match reader.get_next_frame(self.catch_up) {
            Ok(Some((header, data))) => {
                // Successfully got a frame
                self.frame_header = Some(header);
                self.frame_width = header.width as usize;
                self.frame_height = header.height as usize;

                // Calculate latency (producer timestamp to now)
                let now = Instant::now();
                let current_time_ns = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as u64;

                // Calculate latency from producer's timestamp
                let latency_ns = if current_time_ns > header.timestamp {
                    current_time_ns - header.timestamp
                } else {
                    0 // Handle clock misalignment
                };

                self.latency_ms = latency_ns as f64 / 1_000_000.0; // ns to ms

                // Optimized format dispatch with SIMD when available
                self.frame_data = match header.format_code {
                    0x02 => {
                        // BGRA format from Black Magic
                        #[cfg(target_arch = "x86_64")]
                        if is_simd_supported() && header.bytes_per_pixel == 4 {
                            unsafe {
                                convert_bgra_to_rgb_simd(data, self.frame_width, self.frame_height)
                            }
                        } else {
                            convert_bgr_to_rgb(
                                data,
                                self.frame_width,
                                self.frame_height,
                                header.bytes_per_pixel as usize,
                            )
                        }

                        #[cfg(not(target_arch = "x86_64"))]
                        convert_bgr_to_rgb(
                            data,
                            self.frame_width,
                            self.frame_height,
                            header.bytes_per_pixel as usize,
                        )
                    }
                    0x01 => {
                        // YUV format
                        #[cfg(target_arch = "x86_64")]
                        if is_x86_feature_detected!("avx2") {
                            unsafe {
                                convert_yuv_to_rgb_simd_avx2(
                                    data,
                                    self.frame_width,
                                    self.frame_height,
                                )
                            }
                        } else {
                            convert_yuv_to_rgb(data, self.frame_width, self.frame_height)
                        }

                        #[cfg(not(target_arch = "x86_64"))]
                        convert_yuv_to_rgb(data, self.frame_width, self.frame_height)
                    }
                    _ => convert_frame_to_rgb(
                        data,
                        self.frame_width,
                        self.frame_height,
                        header.bytes_per_pixel as usize,
                        header.format_code,
                        &self.format,
                    ),
                };

                // Update format string
                self.format = format_code_to_string(header.format_code).to_string();

                // Update FPS tracking
                self.frames_received += 1;
                self.last_frame_time = now;

                // Update FPS counter every 500ms for more stable readings
                if self.last_fps_update.elapsed() >= Duration::from_millis(500) {
                    self.fps =
                        self.frames_received as f64 / self.last_fps_update.elapsed().as_secs_f64();
                    self.frames_received = 0;
                    self.last_fps_update = now;

                    // Update total frames count
                    if let Ok((total_written, _, _)) = reader.get_stats() {
                        self.total_frames = total_written;
                    }
                }

                // Update connection status
                self.connection_status = "Connected".to_string();

                // Record frame processing time
                self.process_time_us = process_start.elapsed().as_micros() as u64;
            }
            Ok(None) => {
                // No new frames, but still connected
            }
            Err(e) => {
                // Error reading frame - likely disconnected
                self.connection_status = format!("Connection error: {}", e);
                if self.verbose {
                    println!("Error reading frame: {}", e);
                }
            }
        }
    }

    fn draw_top_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("header_panel")
            .height_range(48.0..=48.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    // Logo/Application name
                    ui.add_space(8.0);
                    ui.heading("Medical Echography Viewer");

                    // Spacer
                    ui.add_space(32.0);

                    // Patient information
                    ui.group(|ui| {
                        ui.set_width(400.0);
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(RichText::new("Patient:").strong().size(12.0));
                                ui.label(RichText::new("ID:").strong().size(12.0));
                            });

                            ui.vertical(|ui| {
                                ui.label(RichText::new(&self.patient_info.name).size(12.0));
                                ui.label(RichText::new(&self.patient_info.id).size(12.0));
                            });

                            ui.add_space(20.0);

                            ui.vertical(|ui| {
                                ui.label(RichText::new("DOB:").strong().size(12.0));
                                ui.label(RichText::new("Study:").strong().size(12.0));
                            });

                            ui.vertical(|ui| {
                                ui.label(RichText::new(&self.patient_info.dob).size(12.0));
                                ui.label(RichText::new(&self.patient_info.study_date).size(12.0));
                            });
                        });
                    });

                    // Flexible space
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(8.0);

                        // Theme selection
                        let theme_button_text = match self.theme {
                            Theme::Dark => "🌙 Dark",
                            Theme::Light => "☀️ Light",
                            Theme::HighContrast => "🔍 High Contrast",
                        };

                        if ui.button(theme_button_text).clicked() {
                            // Cycle through themes
                            self.theme = match self.theme {
                                Theme::Dark => Theme::Light,
                                Theme::Light => Theme::HighContrast,
                                Theme::HighContrast => Theme::Dark,
                            };
                        }

                        // Connection status with a professional look
                        let (status_text, status_color) =
                            if self.connection_status.starts_with("Connected") {
                                ("Connected", Color32::from_rgb(80, 210, 130)) // Softer green
                            } else {
                                ("Disconnected", Color32::from_rgb(230, 90, 90)) // Softer red
                            };

                        ui.horizontal(|ui| {
                            ui.label("Status:");
                            ui.label(RichText::new(status_text).color(status_color).strong());
                        });

                        // Reconnect button with icon
                        if ui.button("🔄 Reconnect").clicked() {
                            self.try_connect();
                        }
                    });
                });
            });
    }

    fn draw_tools_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("tools_panel")
            .resizable(true)
            .default_width(48.0)
            .width_range(48.0..=200.0)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);

                    // Tool selection
                    ui.heading("Tools");
                    ui.add_space(8.0);

                    // Tool buttons
                    ui.selectable_value(&mut self.selected_tool, Tool::View, "👁️ View");
                    ui.selectable_value(&mut self.selected_tool, Tool::Zoom, "🔍 Zoom");
                    ui.selectable_value(&mut self.selected_tool, Tool::Pan, "✋ Pan");
                    ui.selectable_value(&mut self.selected_tool, Tool::ROI, "⬚ ROI");
                    ui.selectable_value(&mut self.selected_tool, Tool::Measure, "📏 Measure");
                    ui.selectable_value(&mut self.selected_tool, Tool::Annotate, "✏️ Annotate");

                    ui.separator();

                    // Display options
                    ui.heading("Display");
                    ui.add_space(8.0);

                    ui.checkbox(&mut self.show_grid, "Grid");
                    ui.checkbox(&mut self.show_rulers, "Rulers");

                    ui.separator();

                    // Image adjustments
                    ui.heading("Adjustments");
                    ui.add_space(8.0);

                    ui.label("Brightness:");
                    ui.add(egui::Slider::new(&mut self.brightness, -1.0..=1.0).text(""));

                    ui.label("Contrast:");
                    ui.add(egui::Slider::new(&mut self.contrast, -1.0..=1.0).text(""));

                    ui.separator();

                    // Bottom part - expand to show more information
                    if ui.button("ℹ️ Frame Info").clicked() {
                        self.show_info_panel = !self.show_info_panel;
                    }

                    // Annotation text input when annotation tool is selected
                    if matches!(self.selected_tool, Tool::Annotate) {
                        ui.separator();
                        ui.label("Annotation Text:");
                        ui.text_edit_singleline(&mut self.annotation_text);
                    }
                });
            });
    }

    fn draw_info_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("info_panel")
            .resizable(true)
            .default_width(250.0)
            .width_range(200.0..=400.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.heading("Frame Information");
                ui.add_space(8.0);

                // Frame information with a professional layout
                egui::Grid::new("frame_info_grid")
                    .num_columns(2)
                    .spacing([10.0, 6.0])
                    .striped(true)
                    .show(ui, |ui| {
                        if let Some(header) = self.frame_header {
                            // Frame data
                            ui.label(RichText::new("Resolution:").strong());
                            ui.label(format!("{}×{}", header.width, header.height));
                            ui.end_row();

                            ui.label(RichText::new("Format:").strong());
                            ui.label(&self.format);
                            ui.end_row();

                            ui.label(RichText::new("Frame ID:").strong());
                            ui.label(format!("{}", header.frame_id));
                            ui.end_row();

                            ui.label(RichText::new("Sequence:").strong());
                            ui.label(format!("{}", header.sequence_number));
                            ui.end_row();

                            // Performance metrics
                            ui.label(RichText::new("FPS:").strong());
                            ui.label(format!("{:.1}", self.fps));
                            ui.end_row();

                            ui.label(RichText::new("Latency:").strong());
                            ui.label(format!("{:.2} ms", self.latency_ms));
                            ui.end_row();

                            ui.label(RichText::new("Process Time:").strong());
                            ui.label(format!("{:.2} ms", self.process_time_us as f64 / 1000.0));
                            ui.end_row();

                            ui.label(RichText::new("Texture Time:").strong());
                            ui.label(format!("{:.2} ms", self.texture_time_us as f64 / 1000.0));
                            ui.end_row();

                            ui.label(RichText::new("Total Frames:").strong());
                            ui.label(format!("{}", self.total_frames));
                            ui.end_row();
                        } else {
                            ui.label("No frame data available");
                            ui.end_row();
                        }
                    });

                ui.add_space(20.0);

                // Measurements section
                ui.heading("Measurements");
                ui.add_space(8.0);

                if self.measurements.is_empty() {
                    ui.label("No measurements recorded");
                } else {
                    egui::Grid::new("measurements_grid")
                        .num_columns(3)
                        .spacing([10.0, 6.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label(RichText::new("Label").strong());
                            ui.label(RichText::new("Length").strong());
                            ui.label(RichText::new("Action").strong());
                            ui.end_row();

                            for (i, measurement) in self.measurements.iter().enumerate() {
                                ui.label(&measurement.label);

                                // Calculate pixel distance
                                let dx = measurement.end.x - measurement.start.x;
                                let dy = measurement.end.y - measurement.start.y;
                                let distance = (dx * dx + dy * dy).sqrt();
                                ui.label(format!("{:.1} px", distance));

                                if ui.button("🗑").clicked() {
                                    self.measurements.remove(i);
                                    break;
                                }
                                ui.end_row();
                            }
                        });
                }

                ui.add_space(20.0);

                // Annotations section
                ui.heading("Annotations");
                ui.add_space(8.0);

                if self.annotations.is_empty() {
                    ui.label("No annotations added");
                } else {
                    egui::Grid::new("annotations_grid")
                        .num_columns(3)
                        .spacing([10.0, 6.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label(RichText::new("Text").strong());
                            ui.label(RichText::new("Position").strong());
                            ui.label(RichText::new("Action").strong());
                            ui.end_row();

                            for (i, annotation) in self.annotations.iter().enumerate() {
                                let text = if annotation.text.len() > 15 {
                                    format!("{}...", &annotation.text[0..12])
                                } else {
                                    annotation.text.clone()
                                };

                                ui.label(text);
                                ui.label(format!(
                                    "({:.0},{:.0})",
                                    annotation.position.x, annotation.position.y
                                ));

                                if ui.button("🗑").clicked() {
                                    self.annotations.remove(i);
                                    break;
                                }
                                ui.end_row();
                            }
                        });
                }

                ui.add_space(ui.available_height() - 30.0);

                // Help text at bottom
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.label(RichText::new("Use mouse wheel to zoom").size(10.0));
                    ui.label(RichText::new("Drag to pan when zoomed").size(10.0));
                });
            });
    }

    fn draw_central_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // If we're not connected, show a message
            if !self.shm_reader.lock().unwrap().is_connected() {
                ui.centered_and_justified(|ui| {
                    // Professional-looking "no connection" message
                    let text_color = match self.theme {
                        Theme::Dark => Color32::from_rgb(200, 200, 210),
                        Theme::Light => Color32::from_rgb(80, 80, 100),
                        Theme::HighContrast => Color32::WHITE,
                    };

                    ui.vertical_centered(|ui| {
                        ui.add_space(50.0);
                        ui.add(egui::Label::new(
                            RichText::new("🔄").color(text_color).size(36.0),
                        ));
                        ui.add_space(20.0);
                        ui.add(egui::Label::new(
                            RichText::new("Waiting for Connection...")
                                .color(text_color)
                                .size(24.0),
                        ));
                        ui.add_space(10.0);
                        ui.add(egui::Label::new(
                            RichText::new("Attempting to connect to ultrasound device")
                                .color(text_color)
                                .size(16.0),
                        ));
                        ui.add_space(20.0);

                        // Reconnect button
                        if ui.button("Reconnect Now").clicked() {
                            self.try_connect();
                        }
                    });
                });
                return;
            }

            // Update or create texture
            self.image_texture_id = self.update_or_create_texture(ctx);

            if let Some(texture_id) = self.image_texture_id {
                // Calculate available space and size for the image
                let available_size = ui.available_size();
                let image_aspect_ratio = self.frame_width as f32 / self.frame_height as f32;
                let panel_aspect_ratio = available_size.x / available_size.y;

                let mut display_size = if image_aspect_ratio > panel_aspect_ratio {
                    // Width constrained
                    Vec2::new(available_size.x, available_size.x / image_aspect_ratio)
                } else {
                    // Height constrained
                    Vec2::new(available_size.y * image_aspect_ratio, available_size.y)
                };

                // Apply zoom level
                display_size.x *= self.zoom_level;
                display_size.y *= self.zoom_level;

                // Get the response for interaction
                let image_response = ui
                    .centered_and_justified(|ui| ui.image((texture_id, display_size)))
                    .inner;

                // Draw rulers if enabled
                if self.show_rulers {
                    self.draw_rulers(ui, image_response.rect);
                }

                // Draw grid if enabled
                if self.show_grid {
                    self.draw_grid(ui, image_response.rect);
                }

                // Handle interactions based on selected tool
                if image_response.hovered() {
                    let pointer_pos = ui.input(|i| i.pointer.hover_pos());

                    if let Some(pos) = pointer_pos {
                        // Handle different tools
                        match self.selected_tool {
                            Tool::ROI => self.handle_roi_tool(ui, image_response, pos),
                            Tool::Measure => self.handle_measure_tool(ui, image_response, pos),
                            Tool::Annotate => self.handle_annotate_tool(ui, image_response, pos),
                            // Other tools handled separately
                            _ => {}
                        }
                    }
                }

                // Draw existing measurements
                for measurement in &self.measurements {
                    // Draw measurement line
                    let stroke = Stroke::new(2.0, Color32::from_rgb(255, 220, 0));
                    ui.painter()
                        .line_segment([measurement.start, measurement.end], stroke);

                    // Draw measurement label
                    let mid_point = Pos2::new(
                        (measurement.start.x + measurement.end.x) / 2.0,
                        (measurement.start.y + measurement.end.y) / 2.0 - 15.0,
                    );

                    // Add a background for the text
                    let text_size = egui::Vec2::new(60.0, 20.0);
                    let text_rect = Rect::from_center_size(mid_point, text_size);
                    ui.painter().rect_filled(
                        text_rect,
                        egui::CornerRadius::same(3),
                        Color32::from_rgba_premultiplied(0, 0, 0, 180),
                    );

                    // Calculate distance in pixels
                    let dx = measurement.end.x - measurement.start.x;
                    let dy = measurement.end.y - measurement.start.y;
                    let distance = (dx * dx + dy * dy).sqrt();

                    ui.painter().text(
                        mid_point,
                        egui::Align2::CENTER_CENTER,
                        format!("{}: {:.1}px", measurement.label, distance),
                        FontId::proportional(12.0),
                        Color32::WHITE,
                    );
                }

                // Draw annotations
                for annotation in &self.annotations {
                    // Background for annotation
                    let text_size = ui
                        .fonts(|f| {
                            f.layout_no_wrap(
                                annotation.text.clone(),
                                FontId::proportional(12.0),
                                Color32::WHITE,
                            )
                        })
                        .rect
                        .size();

                    let text_rect =
                        Rect::from_min_size(annotation.position, text_size + egui::vec2(10.0, 6.0));

                    ui.painter().rect_filled(
                        text_rect,
                        egui::CornerRadius::same(3),
                        Color32::from_rgba_premultiplied(30, 30, 120, 220),
                    );

                    // Draw text
                    ui.painter().text(
                        annotation.position + egui::vec2(5.0, 3.0),
                        egui::Align2::LEFT_TOP,
                        &annotation.text,
                        FontId::proportional(12.0),
                        Color32::WHITE,
                    );
                }

                // Draw active ROI if any
                if let Some(roi) = self.region_of_interest {
                    // Draw ROI rectangle
                    ui.painter().rect_stroke(
                        roi,
                        egui::CornerRadius::same(0),
                        Stroke::new(2.0, Color32::from_rgb(255, 200, 0)),
                        StrokeKind::Inside,
                    );

                    // Draw ROI dimensions text
                    let text = format!("ROI: {}x{}", roi.width() as i32, roi.height() as i32);
                    let text_pos = Pos2::new(roi.min.x, roi.min.y - 20.0);

                    // Background for text
                    let text_size = ui
                        .fonts(|f| {
                            f.layout_no_wrap(
                                text.clone(),
                                FontId::proportional(12.0),
                                Color32::WHITE,
                            )
                        })
                        .rect
                        .size();

                    let text_rect =
                        Rect::from_min_size(text_pos, text_size + egui::vec2(10.0, 6.0));

                    ui.painter().rect_filled(
                        text_rect,
                        egui::CornerRadius::same(3),
                        Color32::from_rgba_premultiplied(0, 0, 0, 180),
                    );

                    // Draw text
                    ui.painter().text(
                        text_pos + egui::vec2(5.0, 3.0),
                        egui::Align2::LEFT_TOP,
                        text,
                        FontId::proportional(12.0),
                        Color32::WHITE,
                    );
                }
            } else {
                // No valid frame yet
                ui.centered_and_justified(|ui| {
                    let text_color = match self.theme {
                        Theme::Dark => Color32::from_rgb(200, 200, 210),
                        Theme::Light => Color32::from_rgb(80, 80, 100),
                        Theme::HighContrast => Color32::WHITE,
                    };

                    ui.vertical_centered(|ui| {
                        ui.add_space(50.0);
                        ui.add(egui::Label::new(
                            RichText::new("🎬").color(text_color).size(36.0),
                        ));
                        ui.add_space(20.0);
                        ui.add(egui::Label::new(
                            RichText::new("Waiting for Frames...")
                                .color(text_color)
                                .size(24.0),
                        ));
                        ui.add_space(10.0);
                        ui.add(egui::Label::new(
                            RichText::new("Connected to device, awaiting video stream")
                                .color(text_color)
                                .size(16.0),
                        ));
                    });
                });
            }
        });
    }

    fn draw_bottom_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("bottom_panel")
            .height_range(40.0..=40.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(8.0);

                    // Zoom controls
                    ui.label("Zoom:");
                    if ui.button("-").clicked() {
                        self.zoom_level = (self.zoom_level - 0.1).max(0.5);
                    }

                    ui.label(format!("{:.1}×", self.zoom_level));

                    if ui.button("+").clicked() {
                        self.zoom_level = (self.zoom_level + 0.1).min(4.0);
                    }

                    ui.separator();

                    // Frame information
                    if let Some(header) = self.frame_header {
                        ui.label(format!("Frame: {}", header.frame_id));
                        ui.separator();
                        ui.label(format!("FPS: {:.1}", self.fps));
                        ui.separator();
                        ui.label(format!("Latency: {:.1} ms", self.latency_ms));
                    }

                    // Right-aligned controls
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(8.0);
                        ui.checkbox(&mut self.catch_up, "Low Latency Mode");

                        // Mode indicator (static text for this demo)
                        ui.separator();
                        ui.label(RichText::new("Mode: B-Mode").strong());
                        ui.separator();
                        ui.label(RichText::new("Depth: 10 cm").strong());
                    });
                });
            });
    }

    // Helper methods for tool interactions
    fn handle_roi_tool(
        &mut self,
        ui: &mut egui::Ui,
        image_response: egui::Response,
        cursor_pos: Pos2,
    ) {
        // ROI tool implementation
        if ui.input(|i| i.pointer.primary_pressed()) {
            self.roi_active = true;
            self.roi_start = Some(cursor_pos);
            self.roi_end = Some(cursor_pos);
        }

        if self.roi_active {
            if ui.input(|i| i.pointer.primary_down()) {
                self.roi_end = Some(cursor_pos);

                // Update region of interest rectangle
                if let (Some(start), Some(end)) = (self.roi_start, self.roi_end) {
                    let min_x = start.x.min(end.x);
                    let min_y = start.y.min(end.y);
                    let max_x = start.x.max(end.x);
                    let max_y = start.y.max(end.y);

                    self.region_of_interest = Some(Rect::from_min_max(
                        Pos2::new(min_x, min_y),
                        Pos2::new(max_x, max_y),
                    ));
                }
            }

            if ui.input(|i| i.pointer.primary_released()) {
                self.roi_active = false;
                // Keep the ROI rectangle
            }
        }

        // Preview the ROI while drawing
        if self.roi_active {
            if let (Some(start), Some(end)) = (self.roi_start, self.roi_end) {
                let rect = Rect::from_two_pos(start, end);
                ui.painter().rect_stroke(
                    rect,
                    egui::CornerRadius::same(0),
                    Stroke::new(1.0, Color32::from_rgb(255, 200, 0)),
                    StrokeKind::Inside,
                );
            }
        }
    }

    fn handle_measure_tool(
        &mut self,
        ui: &mut egui::Ui,
        image_response: egui::Response,
        cursor_pos: Pos2,
    ) {
        // Measurement tool implementation
        static mut MEASURING_ACTIVE: bool = false;
        static mut MEASURE_START: Option<Pos2> = None;

        unsafe {
            if ui.input(|i| i.pointer.primary_pressed()) {
                MEASURING_ACTIVE = true;
                MEASURE_START = Some(cursor_pos);
            }

            if MEASURING_ACTIVE {
                if let Some(start) = MEASURE_START {
                    // Draw the measurement line
                    ui.painter().line_segment(
                        [start, cursor_pos],
                        Stroke::new(2.0, Color32::from_rgb(255, 220, 0)),
                    );

                    // Show distance while dragging
                    let dx = cursor_pos.x - start.x;
                    let dy = cursor_pos.y - start.y;
                    let distance = (dx * dx + dy * dy).sqrt();

                    let mid_point = Pos2::new(
                        (start.x + cursor_pos.x) / 2.0,
                        (start.y + cursor_pos.y) / 2.0 - 15.0,
                    );

                    // Background
                    let text = format!("{:.1} px", distance);
                    let text_size = ui
                        .fonts(|f| {
                            f.layout_no_wrap(
                                text.clone(),
                                FontId::proportional(12.0),
                                Color32::WHITE,
                            )
                        })
                        .rect
                        .size();

                    let text_rect =
                        Rect::from_center_size(mid_point, text_size + egui::vec2(10.0, 6.0));
                    ui.painter().rect_filled(
                        text_rect,
                        egui::CornerRadius::same(3),
                        Color32::from_rgba_premultiplied(0, 0, 0, 180),
                    );

                    // Draw text
                    ui.painter().text(
                        mid_point,
                        egui::Align2::CENTER_CENTER,
                        text,
                        FontId::proportional(12.0),
                        Color32::WHITE,
                    );
                }

                if ui.input(|i| i.pointer.primary_released()) {
                    if let Some(start) = MEASURE_START {
                        // Finalize measurement
                        let dx = cursor_pos.x - start.x;
                        let dy = cursor_pos.y - start.y;
                        let distance = (dx * dx + dy * dy).sqrt();

                        // Only add if it's a meaningful measurement (not just a click)
                        if distance > 5.0 {
                            // Generate a default label
                            let label = format!("M{}", self.measurements.len() + 1);

                            self.measurements.push(Measurement {
                                start,
                                end: cursor_pos,
                                label,
                            });
                        }
                    }

                    MEASURING_ACTIVE = false;
                    MEASURE_START = None;
                }
            }
        }
    }

    fn handle_annotate_tool(
        &mut self,
        ui: &mut egui::Ui,
        image_response: egui::Response,
        cursor_pos: Pos2,
    ) {
        // Annotation tool implementation
        if ui.input(|i| i.pointer.primary_clicked()) {
            if !self.annotation_text.is_empty() {
                self.annotations.push(Annotation {
                    position: cursor_pos,
                    text: self.annotation_text.clone(),
                });

                // Clear the text input after adding
                self.annotation_text.clear();
            } else {
                // If no text is entered, show a small popup
                let text_pos = cursor_pos + egui::vec2(10.0, 10.0);
                let text = "Enter annotation text in sidebar";

                // Background
                let text_size = ui
                    .fonts(|f| {
                        f.layout_no_wrap(
                            text.parse().unwrap(),
                            FontId::proportional(12.0),
                            Color32::WHITE,
                        )
                    })
                    .rect
                    .size();

                let text_rect = Rect::from_min_size(text_pos, text_size + egui::vec2(10.0, 6.0));

                ui.painter().rect_filled(
                    text_rect,
                    egui::CornerRadius::same(3),
                    Color32::from_rgba_premultiplied(40, 40, 40, 220),
                );

                // Draw text
                ui.painter().text(
                    text_pos + egui::vec2(5.0, 3.0),
                    egui::Align2::LEFT_TOP,
                    text,
                    FontId::proportional(12.0),
                    Color32::WHITE,
                );
            }
        }
    }

    // Draw rulers around the image
    fn draw_rulers(&self, ui: &egui::Ui, image_rect: Rect) {
        let stroke = Stroke::new(1.0, Color32::from_rgba_premultiplied(200, 200, 200, 180));
        let text_color = Color32::from_rgba_premultiplied(200, 200, 200, 220);

        // Horizontal ruler (top)
        let ruler_height = 20.0;
        let ruler_rect = Rect::from_min_max(
            Pos2::new(image_rect.min.x, image_rect.min.y - ruler_height),
            Pos2::new(image_rect.max.x, image_rect.min.y),
        );

        ui.painter().rect_filled(
            ruler_rect,
            egui::CornerRadius::same(0),
            Color32::from_rgba_premultiplied(30, 30, 30, 180),
        );

        // Ticks every 50 pixels
        let tick_interval = 50.0;
        let mut x = image_rect.min.x;
        while x <= image_rect.max.x {
            let tick_height = if (x - image_rect.min.x) % 100.0 < 1.0 {
                8.0
            } else {
                5.0
            };

            ui.painter().line_segment(
                [
                    Pos2::new(x, image_rect.min.y - tick_height),
                    Pos2::new(x, image_rect.min.y),
                ],
                stroke,
            );

            // Labels at major ticks
            if (x - image_rect.min.x) % 100.0 < 1.0 {
                let label = format!("{}", ((x - image_rect.min.x) / self.zoom_level) as i32);
                ui.painter().text(
                    Pos2::new(x, image_rect.min.y - 15.0),
                    egui::Align2::CENTER_CENTER,
                    label,
                    FontId::proportional(10.0),
                    text_color,
                );
            }

            x += tick_interval;
        }

        // Vertical ruler (left)
        let ruler_width = 20.0;
        let ruler_rect = Rect::from_min_max(
            Pos2::new(image_rect.min.x - ruler_width, image_rect.min.y),
            Pos2::new(image_rect.min.x, image_rect.max.y),
        );

        ui.painter().rect_filled(
            ruler_rect,
            egui::CornerRadius::same(0),
            Color32::from_rgba_premultiplied(30, 30, 30, 180),
        );

        // Ticks every 50 pixels
        let mut y = image_rect.min.y;
        while y <= image_rect.max.y {
            let tick_width = if (y - image_rect.min.y) % 100.0 < 1.0 {
                8.0
            } else {
                5.0
            };

            ui.painter().line_segment(
                [
                    Pos2::new(image_rect.min.x - tick_width, y),
                    Pos2::new(image_rect.min.x, y),
                ],
                stroke,
            );

            // Labels at major ticks
            if (y - image_rect.min.y) % 100.0 < 1.0 {
                let label = format!("{}", ((y - image_rect.min.y) / self.zoom_level) as i32);
                ui.painter().text(
                    Pos2::new(image_rect.min.x - 10.0, y),
                    egui::Align2::CENTER_CENTER,
                    label,
                    FontId::proportional(10.0),
                    text_color,
                );
            }

            y += tick_interval;
        }
    }

    // Draw grid over the image
    fn draw_grid(&self, ui: &egui::Ui, image_rect: Rect) {
        let stroke = Stroke::new(1.0, Color32::from_rgba_premultiplied(150, 150, 150, 100));

        // Grid size
        let grid_size = 50.0;

        // Vertical lines
        let mut x = image_rect.min.x + grid_size;
        while x < image_rect.max.x {
            ui.painter().line_segment(
                [
                    Pos2::new(x, image_rect.min.y),
                    Pos2::new(x, image_rect.max.y),
                ],
                stroke,
            );
            x += grid_size;
        }

        // Horizontal lines
        let mut y = image_rect.min.y + grid_size;
        while y < image_rect.max.y {
            ui.painter().line_segment(
                [
                    Pos2::new(image_rect.min.x, y),
                    Pos2::new(image_rect.max.x, y),
                ],
                stroke,
            );
            y += grid_size;
        }
    }
}

impl eframe::App for EchoViewer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Configure styles if first time
        self.configure_styles(ctx);

        // Request a repaint for the next frame
        ctx.request_repaint();

        // Check connection and update frame (your existing code)
        self.check_connection();
        self.update_frame();

        // Top panel with patient info and application bar
        self.draw_top_panel(ctx);

        // Left panel with tools
        if self.show_tools_panel {
            self.draw_tools_panel(ctx);
        }

        // Right panel with image info and measurements
        if self.show_info_panel {
            self.draw_info_panel(ctx);
        }

        // Central panel with the image
        self.draw_central_panel(ctx);

        // Bottom panel with timeline and controls
        self.draw_bottom_panel(ctx);
    }
}

fn main() -> Result<(), eframe::Error> {
    // Parse command line arguments
    let args = Args::parse();

    // Create eframe options
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([args.width as f32, args.height as f32])
            .with_min_inner_size([800.0, 600.0])
            .with_resizable(true)
            .with_decorations(true) // Show window decorations
            .with_transparent(false), // No transparency
        //.with_icon(EguiIcon::default_raw()),
        vsync: false, // Disable VSync for minimal latency
        hardware_acceleration: eframe::HardwareAcceleration::Required,
        ..Default::default()
    };

    // Run the application
    eframe::run_native(
        "Medical Echography Viewer",
        native_options,
        Box::new(|_cc| Ok(Box::new(EchoViewer::new(args)))),
    )
}
