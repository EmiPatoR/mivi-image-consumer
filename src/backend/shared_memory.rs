// src/backend/shared_memory.rs - Zero-Copy Shared Memory Implementation

use std::sync::Arc;
use std::time::{Duration, Instant};
use std::fs::OpenOptions;
use std::io::ErrorKind;
use memmap2::{MmapOptions, MmapMut};
use parking_lot::RwLock;
use tracing::{info, warn, error, debug};

use crate::backend::types::{
    FrameHeader, ControlBlock, RawFrame, ConnectionConfig
};

/// Shared memory reader with zero-copy frame access
pub struct SharedMemoryReader {
    // Memory mapping (protected by RwLock for thread safety)
    mmap: Arc<RwLock<Option<MmapMut>>>,
    
    // Configuration
    shm_name: String,
    config: ConnectionConfig,
    
    // Memory layout information
    control_block_size: usize,
    metadata_area_size: usize,
    data_offset: usize,
    max_frames: usize,
    frame_slot_size: usize,
    
    // State tracking
    last_processed_index: Arc<RwLock<u64>>,
    connected: Arc<RwLock<bool>>,
    last_connection_attempt: Arc<RwLock<Instant>>,
    last_frame_time: Arc<RwLock<Instant>>,
    
    // Performance monitoring
    frame_count: Arc<RwLock<u64>>,
    error_count: Arc<RwLock<u64>>,
}

impl SharedMemoryReader {
    /// Create a new shared memory reader
    pub fn new(shm_name: &str, config: ConnectionConfig) -> Result<Self, SharedMemoryError> {
        let reader = Self {
            mmap: Arc::new(RwLock::new(None)),
            shm_name: shm_name.to_string(),
            config,
            control_block_size: std::mem::size_of::<ControlBlock>(),
            metadata_area_size: 4096, // Default, will be updated
            data_offset: 0,
            max_frames: 7, // Default, will be updated
            frame_slot_size: 0,
            last_processed_index: Arc::new(RwLock::new(0)),
            connected: Arc::new(RwLock::new(false)),
            last_connection_attempt: Arc::new(RwLock::new(Instant::now() - Duration::from_secs(10))),
            last_frame_time: Arc::new(RwLock::new(Instant::now())),
            frame_count: Arc::new(RwLock::new(0)),
            error_count: Arc::new(RwLock::new(0)),
        };
        
        Ok(reader)
    }
    
    /// Attempt to connect to shared memory
    pub async fn connect(&mut self) -> Result<(), SharedMemoryError> {
        *self.last_connection_attempt.write() = Instant::now();
        
        let file_path = format!("/dev/shm/{}", self.shm_name);
        if self.config.verbose_logging {
            info!("🔌 Opening shared memory: {}", file_path);
        }
        
        // Open the shared memory file
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&file_path)
            .map_err(|e| match e.kind() {
                ErrorKind::NotFound => SharedMemoryError::NotFound(self.shm_name.clone()),
                _ => SharedMemoryError::Io(e),
            })?;
        
        // Memory map the file
        let mmap = unsafe { 
            MmapOptions::new()
                .map_mut(&file)
                .map_err(|e| SharedMemoryError::MappingFailed(e.to_string()))?
        };
        
        if self.config.verbose_logging {
            info!("✅ Mapped shared memory: {} bytes", mmap.len());
        }
        
        // Validate and initialize memory layout
        self.initialize_memory_layout(&mmap)?;
        
        // Store the memory map
        *self.mmap.write() = Some(mmap);
        *self.connected.write() = true;
        *self.last_frame_time.write() = Instant::now();
        
        info!("🔗 Connected to shared memory: {}", self.shm_name);
        Ok(())
    }
    
    /// Initialize memory layout from control block
    fn initialize_memory_layout(&mut self, mmap: &MmapMut) -> Result<(), SharedMemoryError> {
        // Validate memory size
        if mmap.len() < self.control_block_size {
            return Err(SharedMemoryError::InvalidLayout(
                format!("Memory too small: {} < {}", mmap.len(), self.control_block_size)
            ));
        }
        
        // Read control block
        let control_block = unsafe {
            &*(mmap.as_ptr() as *const ControlBlock)
        };
        
        if self.config.verbose_logging {
            debug!("📊 Control block: write_index={}, active={}, frame_count={}", 
                   control_block.write_index, control_block.active, control_block.frame_count);
        }
        
        // Extract metadata area size
        self.metadata_area_size = control_block.metadata_size as usize;
        if self.metadata_area_size == 0 {
            self.metadata_area_size = 4096; // Default fallback
        }
        
        // Calculate data offset
        self.data_offset = self.control_block_size + self.metadata_area_size;
        
        // Read metadata to get frame configuration
        let metadata_offset = control_block.metadata_offset as usize;
        if metadata_offset + self.metadata_area_size <= mmap.len() {
            let metadata_slice = &mmap[metadata_offset..metadata_offset + self.metadata_area_size];
            if let Some(null_pos) = metadata_slice.iter().position(|&b| b == 0) {
                if let Ok(metadata_str) = std::str::from_utf8(&metadata_slice[..null_pos]) {
                    if let Ok(metadata_json) = serde_json::from_str::<serde_json::Value>(metadata_str) {
                        // Extract frame slot size
                        if let Some(slot_size) = metadata_json["frame_slot_size"].as_u64() {
                            self.frame_slot_size = slot_size as usize;
                        }
                        
                        // Extract max frames
                        if let Some(max_frames) = metadata_json["max_frames"].as_u64() {
                            self.max_frames = max_frames as usize;
                        }
                        
                        if self.config.verbose_logging {
                            debug!("📋 Metadata: frame_slot_size={}, max_frames={}", 
                                   self.frame_slot_size, self.max_frames);
                        }
                    }
                }
            }
        }
        
        // Validate configuration
        if self.frame_slot_size == 0 {
            // Calculate default frame slot size for 4K + header
            self.frame_slot_size = 3840 * 2160 * 4 + std::mem::size_of::<FrameHeader>();
            warn!("⚠️ Using default frame slot size: {}", self.frame_slot_size);
        }
        
        if self.max_frames == 0 {
            self.max_frames = 7;
            warn!("⚠️ Using default max frames: {}", self.max_frames);
        }
        
        // Final validation
        let required_size = self.data_offset + (self.max_frames * self.frame_slot_size);
        if mmap.len() < required_size {
            return Err(SharedMemoryError::InvalidLayout(
                format!("Memory too small for frame buffer: {} < {}", mmap.len(), required_size)
            ));
        }
        
        info!("✅ Memory layout initialized: data_offset={}, frame_slot_size={}, max_frames={}", 
              self.data_offset, self.frame_slot_size, self.max_frames);
        
        Ok(())
    }
    
    /// Check if connected to shared memory
    pub fn is_connected(&self) -> bool {
        *self.connected.read()
    }
    
    /// Check connection health
    pub fn check_connection_health(&self) -> bool {
        if !self.is_connected() {
            return false;
        }
        
        // Check if we haven't received frames for too long
        if self.last_frame_time.read().elapsed() > self.config.frame_timeout {
            if self.config.verbose_logging {
                warn!("⚠️ No frames received for {:?}", self.config.frame_timeout);
            }
            *self.connected.write() = false;
            return false;
        }
        
        // Check control block active flag
        if let Some(mmap) = self.mmap.read().as_ref() {
            let control_block = unsafe {
                &*(mmap.as_ptr() as *const ControlBlock)
            };
            
            if !control_block.active {
                if self.config.verbose_logging {
                    warn!("⚠️ Control block marked as inactive");
                }
                *self.connected.write() = false;
                return false;
            }
        }
        
        true
    }
    
    /// Get next frame with zero-copy semantics
    pub async fn get_next_frame(&self, catch_up: bool) -> Result<Option<RawFrame>, SharedMemoryError> {
        if !self.is_connected() {
            return Err(SharedMemoryError::NotConnected);
        }
        
        let mmap_lock = self.mmap.read();
        let mmap = mmap_lock.as_ref()
            .ok_or(SharedMemoryError::NotConnected)?;
        
        // Read control block
        let control_block = unsafe {
            &*(mmap.as_ptr() as *const ControlBlock)
        };
        
        if self.config.verbose_logging && *self.frame_count.read() % 60 == 0 {
            debug!("📊 Control: write={}, read={}, count={}, active={}", 
                   control_block.write_index, control_block.read_index, 
                   control_block.frame_count, control_block.active);
        }
        
        // Check if control block is still active
        if !control_block.active {
            *self.connected.write() = false;
            return Err(SharedMemoryError::ConnectionLost);
        }
        
        let last_processed = *self.last_processed_index.read();
        
        // Check if new frames are available
        if control_block.write_index <= last_processed {
            return Ok(None);
        }
        
        // Determine which frame to read
        let frame_index = if catch_up {
            control_block.write_index - 1 // Latest frame
        } else {
            last_processed + 1 // Next frame in sequence
        };
        
        // Calculate frame offset
        let slot_index = (frame_index as usize) % self.max_frames;
        let frame_offset = self.data_offset + slot_index * self.frame_slot_size;
        
        // Validate frame offset
        if frame_offset >= mmap.len() {
            *self.error_count.write() += 1;
            return Err(SharedMemoryError::InvalidFrameOffset(frame_offset));
        }
        
        // Read frame header
        let header_size = std::mem::size_of::<FrameHeader>();
        if frame_offset + header_size > mmap.len() {
            *self.error_count.write() += 1;
            return Err(SharedMemoryError::InvalidFrameOffset(frame_offset));
        }
        
        let header = unsafe {
            *(mmap.as_ptr().add(frame_offset) as *const FrameHeader)
        };
        
        // Validate header
        if header.width == 0 || header.height == 0 || header.data_size == 0 {
            if self.config.verbose_logging {
                debug!("⚠️ Invalid frame header at offset {}: {}x{}, size={}", 
                       frame_offset, header.width, header.height, header.data_size);
            }
            *self.last_processed_index.write() = frame_index;
            return Ok(None);
        }
        
        // Calculate data boundaries
        let data_start = frame_offset + header_size;
        let data_end = data_start + header.data_size as usize;
        
        if data_end > mmap.len() {
            *self.error_count.write() += 1;
            return Err(SharedMemoryError::InvalidFrameSize {
                start: data_start,
                end: data_end,
                total: mmap.len(),
            });
        }
        
        // Create zero-copy frame data
        let frame_data = unsafe {
            let ptr = mmap.as_ptr().add(data_start);
            let slice = std::slice::from_raw_parts(ptr, header.data_size as usize);
            Arc::from(slice)
        };
        
        // Read metadata if present
        let metadata = if header.metadata_size > 0 {
            let metadata_start = frame_offset + header.metadata_offset as usize;
            let metadata_end = metadata_start + header.metadata_size as usize;
            
            if metadata_end <= mmap.len() {
                let metadata_slice = &mmap[metadata_start..metadata_end];
                if let Some(null_pos) = metadata_slice.iter().position(|&b| b == 0) {
                    std::str::from_utf8(&metadata_slice[..null_pos])
                        .ok()
                        .map(|s| s.to_string())
                } else {
                    std::str::from_utf8(metadata_slice)
                        .ok()
                        .map(|s| s.to_string())
                }
            } else {
                None
            }
        } else {
            None
        };
        
        // Update processed index and statistics
        *self.last_processed_index.write() = frame_index;
        *self.last_frame_time.write() = Instant::now();
        *self.frame_count.write() += 1;
        
        // Update control block read index (unsafe but required for shared memory protocol)
        unsafe {
            let control_block_mut = mmap.as_ptr() as *mut ControlBlock;
            (*control_block_mut).read_index = frame_index + 1;
            (*control_block_mut).last_read_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64;
            
            // Decrement frame count
            if (*control_block_mut).frame_count > 0 {
                (*control_block_mut).frame_count -= 1;
            }
            
            // Update total frames read
            (*control_block_mut).total_frames_read += 1;
        }
        
        if self.config.verbose_logging && *self.frame_count.read() <= 5 {
            info!("📺 Frame {}: {}x{}, format={}, size={} bytes", 
                  frame_index, header.width, header.height, 
                  crate::backend::types::format_code_to_string(header.format_code),
                  header.data_size);
        }
        
        // Create and return raw frame
        let raw_frame = RawFrame::new(header, frame_data, metadata);
        Ok(Some(raw_frame))
    }
    
    /// Disconnect from shared memory
    pub async fn disconnect(&mut self) {
        *self.mmap.write() = None;
        *self.connected.write() = false;
        
        info!("🔌 Disconnected from shared memory: {}", self.shm_name);
    }
    
    /// Get connection statistics
    pub fn get_statistics(&self) -> ConnectionStatistics {
        let mmap_lock = self.mmap.read();
        let control_stats = if let Some(mmap) = mmap_lock.as_ref() {
            let control_block = unsafe {
                &*(mmap.as_ptr() as *const ControlBlock)
            };
            
            Some(ControlBlockStats {
                total_frames_written: control_block.total_frames_written,
                total_frames_read: control_block.total_frames_read,
                frames_in_buffer: control_block.frame_count,
                dropped_frames: control_block.dropped_frames,
                active: control_block.active,
            })
        } else {
            None
        };
        
        ConnectionStatistics {
            connected: self.is_connected(),
            shm_name: self.shm_name.clone(),
            frames_processed: *self.frame_count.read(),
            error_count: *self.error_count.read(),
            last_frame_elapsed: self.last_frame_time.read().elapsed(),
            control_block: control_stats,
        }
    }
    
    /// Force reconnection attempt
    pub async fn force_reconnect(&mut self) -> Result<(), SharedMemoryError> {
        self.disconnect().await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        self.connect().await
    }
}

/// Shared memory error types
#[derive(Debug, thiserror::Error)]
pub enum SharedMemoryError {
    #[error("Shared memory region '{0}' not found")]
    NotFound(String),
    
    #[error("Not connected to shared memory")]
    NotConnected,
    
    #[error("Connection lost to shared memory")]
    ConnectionLost,
    
    #[error("Memory mapping failed: {0}")]
    MappingFailed(String),
    
    #[error("Invalid memory layout: {0}")]
    InvalidLayout(String),
    
    #[error("Invalid frame offset: {0}")]
    InvalidFrameOffset(usize),
    
    #[error("Invalid frame size: start={start}, end={end}, total={total}")]
    InvalidFrameSize {
        start: usize,
        end: usize,
        total: usize,
    },
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("Other error: {0}")]
    Other(String),
}

/// Connection statistics
#[derive(Debug, Clone)]
pub struct ConnectionStatistics {
    pub connected: bool,
    pub shm_name: String,
    pub frames_processed: u64,
    pub error_count: u64,
    pub last_frame_elapsed: Duration,
    pub control_block: Option<ControlBlockStats>,
}

/// Control block statistics
#[derive(Debug, Clone)]
pub struct ControlBlockStats {
    pub total_frames_written: u64,
    pub total_frames_read: u64,
    pub frames_in_buffer: u64,
    pub dropped_frames: u64,
    pub active: bool,
}
