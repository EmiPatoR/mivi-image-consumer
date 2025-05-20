// shared_memory.rs - Implementation of the shared memory interface for reading frame data

use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::fs::OpenOptions;
use std::io::ErrorKind;
use memmap2::{MmapOptions, MmapMut};
use egui::Color32;
use std::arch::x86_64::*;

// Add SIMD feature detection
#[cfg(target_arch = "x86_64")]
pub fn is_simd_supported() -> bool {
    is_x86_feature_detected!("sse4.1") && is_x86_feature_detected!("sse2")
}

#[cfg(not(target_arch = "x86_64"))]
pub fn is_simd_supported() -> bool {
    false
}

// Structure to match the C++ FrameHeader with correct alignment
#[repr(C, align(8))]  // Match C++ alignas(8)
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

// Structure to match the C++ ControlBlock with correct alignment
#[repr(C, align(64))]  // Match C++ alignas(64)
#[derive(Debug)]
pub struct ControlBlock {
    pub write_index: AtomicU64,      // Current write position
    pub read_index: AtomicU64,       // Current read position
    pub frame_count: AtomicU64,      // Number of frames in the buffer
    pub total_frames_written: AtomicU64, // Total number of frames written
    pub total_frames_read: AtomicU64,    // Total number of frames read
    pub dropped_frames: AtomicU64,   // Frames dropped due to buffer full
    pub active: AtomicBool,          // Whether the shared memory is active
    pub _padding1: [u8; 7],          // Padding for alignment after bool
    pub last_write_time: AtomicU64,  // Timestamp of last write (ns since epoch)
    pub last_read_time: AtomicU64,   // Timestamp of last read (ns since epoch)
    pub metadata_offset: u32,        // Offset to metadata area
    pub metadata_size: u32,          // Size of metadata area
    pub flags: u32,                  // Additional flags
    pub _padding2: [u8; 184],        // Padding to ensure proper alignment
}

// SharedMemoryReader manages access to the shared memory
pub struct SharedMemoryReader {
    pub mmap: Option<MmapMut>,        // Now optional to allow reconnection
    pub shm_name: String,             // Store the name for reconnection
    pub control_block_size: usize,
    pub metadata_area_size: usize,
    pub data_offset: usize,
    pub max_frames: usize,
    pub frame_slot_size: usize,
    pub last_processed_index: u64,
    pub verbose: bool,
    pub connected: bool,              // Track connection state
    pub last_connection_attempt: Instant, // When we last tried to connect
    pub last_frame_time: Instant,     // Track when we last received a frame
    pub no_frames_timeout: Duration,  // How long to wait before considering connection stale
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
            return Err(format!("Shared memory too small for control block: {} bytes needed, {} available",
                               control_block_size, mmap.len()).into());
        }

        let control_block_ptr = mmap.as_ptr() as *const ControlBlock;
        let control_block = unsafe { &*control_block_ptr };

        if self.verbose {
            println!("Control block read: write_index={}, read_index={}, frame_count={}",
                     control_block.write_index.load(Ordering::Relaxed),
                     control_block.read_index.load(Ordering::Relaxed),
                     control_block.frame_count.load(Ordering::Relaxed));
        }

        // Verify control block is valid (active flag should be true)
        if !control_block.active.load(Ordering::Relaxed) {
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
        let metadata: serde_json::Value = serde_json::from_str(metadata_str).unwrap_or_else(|_| {
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
                (*control_block_ptr).active.load(Ordering::Acquire)
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
    pub fn get_next_frame<'a>(&'a mut self, catchup: bool) -> Result<Option<(FrameHeader, &'a [u8])>, Box<dyn std::error::Error>> {
        if !self.is_connected() {
            return Err("Not connected to shared memory".into());
        }

        // Fast path: get all atomic data in one go with proper ordering
        let (write_index, control_block_ptr, mmap_ptr, mmap_len) = unsafe {
            let mmap = self.mmap.as_ref().unwrap();
            let control_block_ptr = mmap.as_ptr() as *const ControlBlock;
            let write_index = (*control_block_ptr).write_index.load(Ordering::Acquire);
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
            std::slice::from_raw_parts(
                mmap_ptr.add(data_start),
                header.data_size as usize
            )
        };

        // OPTIMIZATION: Prefetch the next frame's header to reduce latency
        #[cfg(target_arch = "x86_64")]
        unsafe {
            if is_simd_supported() {
                let next_slot_index = ((frame_index + 1) as usize) % self.max_frames;
                let next_frame_offset = self.data_offset + next_slot_index * self.frame_slot_size;

                if next_frame_offset < mmap_len {
                    // Use prefetch hint for next frame with compile-time constant parameter
                    _mm_prefetch::<_MM_HINT_T0>(
                        mmap_ptr.add(next_frame_offset) as *const i8
                    );
                }
            }
        }

        // Update indices atomically with proper memory ordering
        self.last_processed_index = frame_index;

        unsafe {
            // Update the read index in the control block
            (*control_block_ptr).read_index.store(frame_index + 1, Ordering::Release);

            // Update frame count atomically
            let frame_count = (*control_block_ptr).frame_count.load(Ordering::Acquire);
            if frame_count > 0 {
                (*control_block_ptr).frame_count.store(frame_count - 1, Ordering::Release);
            }

            // Update read stats counter
            (*control_block_ptr).total_frames_read.fetch_add(1, Ordering::Relaxed);
            (*control_block_ptr).last_read_time.store(
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos() as u64,
                Ordering::Relaxed
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
                (*control_block_ptr).total_frames_written.load(Ordering::Relaxed),
                (*control_block_ptr).frame_count.load(Ordering::Relaxed),
                (*control_block_ptr).dropped_frames.load(Ordering::Relaxed)
            )
        };

        Ok(stats)
    }
}

// Helper function to convert format code to string
pub fn format_code_to_string(format_code: u32) -> &'static str {
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
pub unsafe fn convert_bgra_to_rgb_simd(data: &[u8], width: usize, height: usize) -> Vec<Color32> {
    let mut rgb_data = vec![Color32::BLACK; width * height];

    // Only use SIMD if we have sufficient data
    if data.len() >= 16 && width >= 4 {
        let pixels_per_iteration = 4; // Process 4 pixels (16 bytes) at once
        let stride = width * 4; // 4 bytes per pixel for BGRA

        // SIMD shuffle mask for BGRA -> RGBA conversion
        let shuffle_mask = _mm_set_epi8(
            15, 12, 13, 14, 11, 8, 9, 10, 7, 4, 5, 6, 3, 0, 1, 2
        );

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
pub fn convert_yuv_to_rgb(data: &[u8], width: usize, height: usize) -> Vec<Color32> {
    // Check if we can use SIMD
    #[cfg(target_arch = "x86_64")]
    if is_x86_feature_detected!("avx2") && width >= 16 {
        unsafe { return convert_yuv_to_rgb_simd_avx2(data, width, height); }
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
pub unsafe fn convert_yuv_to_rgb_simd_avx2(data: &[u8], width: usize, height: usize) -> Vec<Color32> {
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

// High-performance BGR(A) to RGB conversion optimized for medical imaging
pub fn convert_bgr_to_rgb(data: &[u8], width: usize, height: usize, bytes_per_pixel: usize) -> Vec<Color32> {
    // Use SIMD for BGRA format when available
    #[cfg(target_arch = "x86_64")]
    if bytes_per_pixel == 4 && is_simd_supported() && width * height > 1000 {
        unsafe { return convert_bgra_to_rgb_simd(data, width, height); }
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
pub fn convert_frame_to_rgb(
    data: &[u8],
    frame_width: usize,
    frame_height: usize,
    bytes_per_pixel: usize,
    format_code: u32,
    _format_str: &str, // Prefix with underscore to indicate intentionally unused
) -> Vec<Color32> {
    // Direct SIMD dispatch for known formats
    match format_code {
        0x02 => { // BGRA format
            #[cfg(target_arch = "x86_64")]
            if is_simd_supported() && bytes_per_pixel == 4 {
                unsafe { return convert_bgra_to_rgb_simd(data, frame_width, frame_height); }
            }
            // Otherwise fall back to scalar
            convert_bgr_to_rgb(data, frame_width, frame_height, bytes_per_pixel)
        }
        0x01 => { // YUV format
            #[cfg(target_arch = "x86_64")]
            if is_x86_feature_detected!("avx2") {
                unsafe { return convert_yuv_to_rgb_simd_avx2(data, frame_width, frame_height); }
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