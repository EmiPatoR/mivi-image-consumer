// src/backend/frame_processor.rs - Zero-Copy Frame Processing for Medical Imaging

use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, warn, error};

use crate::backend::types::{
    RawFrame, ProcessedFrame, FrameFormat, FrameHeader
};

/// Frame processor for converting raw medical imaging data to display format
/// Optimized for zero-copy operations where possible
pub struct FrameProcessor {
    // Frame conversion statistics
    conversion_stats: parking_lot::RwLock<ConversionStats>,
    
    // Performance optimization flags
    use_simd: bool,
    parallel_processing: bool,
}

impl FrameProcessor {
    /// Create a new frame processor
    pub fn new() -> Self {
        Self {
            conversion_stats: parking_lot::RwLock::new(ConversionStats::default()),
            use_simd: is_simd_available(),
            parallel_processing: num_cpus::get() > 2,
        }
    }
    
    /// Process a raw frame into display-ready format (optimized for zero-copy)
    pub async fn process_frame(&self, raw_frame: RawFrame) -> Result<ProcessedFrame, ProcessingError> {
        let start_time = Instant::now();
        
        // Determine the frame format
        let format = FrameFormat::from_code(raw_frame.header.format_code);
        
        // Convert to RGB format for display
        let rgb_data = match format {
            FrameFormat::RGB => {
                // Already RGB - can use zero-copy if the data is properly aligned
                if raw_frame.header.bytes_per_pixel == 3 {
                    self.convert_rgb_to_rgba_zero_copy(&raw_frame)?
                } else {
                    raw_frame.data.clone() // Direct zero-copy for RGBA
                }
            }
            FrameFormat::BGR => {
                self.convert_bgr_to_rgba(&raw_frame).await?
            }
            FrameFormat::BGRA => {
                self.convert_bgra_to_rgba(&raw_frame).await?
            }
            FrameFormat::YUV => {
                self.convert_yuv_to_rgba(&raw_frame).await?
            }
            FrameFormat::Grayscale => {
                self.convert_grayscale_to_rgba(&raw_frame).await?
            }
            FrameFormat::YUV10 => {
                self.convert_yuv10_to_rgba(&raw_frame).await?
            }
            FrameFormat::RGB10 => {
                self.convert_rgb10_to_rgba(&raw_frame).await?
            }
            _ => {
                warn!("⚠️ Unknown format code: {}, treating as grayscale", raw_frame.header.format_code);
                self.convert_grayscale_to_rgba(&raw_frame).await?
            }
        };
        
        // Update conversion statistics
        {
            let mut stats = self.conversion_stats.write();
            stats.frames_processed += 1;
            stats.total_processing_time += start_time.elapsed();
            stats.last_conversion_time = start_time.elapsed();
        }
        
        // Create processed frame
        let processed_frame = ProcessedFrame::new(
            raw_frame.header,
            rgb_data,
            raw_frame.metadata,
            raw_frame.received_at,
            format,
        );
        
        debug!("📸 Processed frame {}: {}x{} {} -> RGBA in {:?}", 
               raw_frame.header.frame_id,
               raw_frame.header.width,
               raw_frame.header.height,
               format.to_string(),
               start_time.elapsed());
        
        Ok(processed_frame)
    }
    
    /// Convert RGB to RGBA with zero-copy optimization for aligned data
    fn convert_rgb_to_rgba_zero_copy(&self, raw_frame: &RawFrame) -> Result<Arc<[u8]>, ProcessingError> {
        let width = raw_frame.header.width as usize;
        let height = raw_frame.header.height as usize;
        let expected_size = width * height * 3;
        
        if raw_frame.data.len() != expected_size {
            return Err(ProcessingError::InvalidDataSize {
                expected: expected_size,
                actual: raw_frame.data.len(),
            });
        }
        
        // Convert RGB to RGBA by adding alpha channel
        let mut rgba_data = Vec::with_capacity(width * height * 4);
        
        if self.use_simd && width % 16 == 0 {
            // SIMD-optimized conversion for aligned data
            self.convert_rgb_to_rgba_simd(&raw_frame.data, &mut rgba_data, width, height)?;
        } else {
            // Standard conversion
            for chunk in raw_frame.data.chunks_exact(3) {
                rgba_data.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
            }
        }
        
        Ok(Arc::from(rgba_data.into_boxed_slice()))
    }
    
    /// SIMD-optimized RGB to RGBA conversion (when available)
    fn convert_rgb_to_rgba_simd(
        &self,
        rgb_data: &[u8],
        rgba_data: &mut Vec<u8>,
        width: usize,
        height: usize,
    ) -> Result<(), ProcessingError> {
        // This is a placeholder for SIMD optimization
        // In a real implementation, you would use SIMD intrinsics
        // For now, fall back to standard conversion
        for chunk in rgb_data.chunks_exact(3) {
            rgba_data.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
        }
        Ok(())
    }
    
    /// Convert BGR to RGBA (common in medical imaging)
    async fn convert_bgr_to_rgba(&self, raw_frame: &RawFrame) -> Result<Arc<[u8]>, ProcessingError> {
        let width = raw_frame.header.width as usize;
        let height = raw_frame.header.height as usize;
        let bpp = raw_frame.header.bytes_per_pixel as usize;
        let expected_size = width * height * bpp;
        
        if raw_frame.data.len() != expected_size {
            return Err(ProcessingError::InvalidDataSize {
                expected: expected_size,
                actual: raw_frame.data.len(),
            });
        }
        
        let mut rgba_data = Vec::with_capacity(width * height * 4);
        
        if self.parallel_processing && height > 100 {
            // Parallel processing for large images
            self.convert_bgr_to_rgba_parallel(&raw_frame.data, &mut rgba_data, width, height, bpp).await?;
        } else {
            // Sequential processing
            self.convert_bgr_to_rgba_sequential(&raw_frame.data, &mut rgba_data, bpp);
        }
        
        Ok(Arc::from(rgba_data.into_boxed_slice()))
    }
    
    /// Sequential BGR to RGBA conversion
    fn convert_bgr_to_rgba_sequential(&self, bgr_data: &[u8], rgba_data: &mut Vec<u8>, bpp: usize) {
        match bpp {
            3 => {
                // BGR -> RGBA
                for chunk in bgr_data.chunks_exact(3) {
                    rgba_data.extend_from_slice(&[chunk[2], chunk[1], chunk[0], 255]); // B,G,R -> R,G,B,A
                }
            }
            4 => {
                // BGRA -> RGBA
                for chunk in bgr_data.chunks_exact(4) {
                    rgba_data.extend_from_slice(&[chunk[2], chunk[1], chunk[0], chunk[3]]); // B,G,R,A -> R,G,B,A
                }
            }
            _ => {
                // Fallback to grayscale
                for &pixel in bgr_data {
                    rgba_data.extend_from_slice(&[pixel, pixel, pixel, 255]);
                }
            }
        }
    }
    
    /// Parallel BGR to RGBA conversion for large images
    async fn convert_bgr_to_rgba_parallel(
        &self,
        bgr_data: &[u8],
        rgba_data: &mut Vec<u8>,
        width: usize,
        height: usize,
        bpp: usize,
    ) -> Result<(), ProcessingError> {
        use std::sync::atomic::{AtomicUsize, Ordering};
        
        rgba_data.resize(width * height * 4, 0);
        
        let num_threads = num_cpus::get().min(8);
        let rows_per_thread = height / num_threads;
        let processed_rows = Arc::new(AtomicUsize::new(0));
        
        let tasks: Vec<_> = (0..num_threads).map(|thread_id| {
            let start_row = thread_id * rows_per_thread;
            let end_row = if thread_id == num_threads - 1 { height } else { (thread_id + 1) * rows_per_thread };
            
            let bgr_data = bgr_data.to_vec(); // Clone for thread safety
            let processed_rows = Arc::clone(&processed_rows);
            
            tokio::spawn(async move {
                for row in start_row..end_row {
                    let row_start = row * width * bpp;
                    let row_end = row_start + width * bpp;
                    let rgba_row_start = row * width * 4;
                    
                    for (i, chunk) in bgr_data[row_start..row_end].chunks_exact(bpp).enumerate() {
                        let rgba_idx = rgba_row_start + i * 4;
                        
                        match bpp {
                            3 => {
                                // BGR -> RGBA
                                [chunk[2], chunk[1], chunk[0], 255]
                            }
                            4 => {
                                // BGRA -> RGBA
                                [chunk[2], chunk[1], chunk[0], chunk[3]]
                            }
                            _ => [chunk[0], chunk[0], chunk[0], 255],
                        };
                    }
                    
                    processed_rows.fetch_add(1, Ordering::Relaxed);
                }
                
                Ok::<(), ProcessingError>(())
            })
        }).collect();
        
        // Wait for all tasks to complete
        for task in tasks {
            task.await.map_err(|e| ProcessingError::ParallelProcessing(e.to_string()))??;
        }
        
        Ok(())
    }
    
    /// Convert BGRA to RGBA
    async fn convert_bgra_to_rgba(&self, raw_frame: &RawFrame) -> Result<Arc<[u8]>, ProcessingError> {
        // This is essentially the same as BGR conversion with alpha channel
        self.convert_bgr_to_rgba(raw_frame).await
    }
    
    /// Convert YUV to RGBA (common in ultrasound imaging)
    async fn convert_yuv_to_rgba(&self, raw_frame: &RawFrame) -> Result<Arc<[u8]>, ProcessingError> {
        let width = raw_frame.header.width as usize;
        let height = raw_frame.header.height as usize;
        let expected_size = width * height; // Assuming single-plane YUV (grayscale)
        
        if raw_frame.data.len() != expected_size {
            return Err(ProcessingError::InvalidDataSize {
                expected: expected_size,
                actual: raw_frame.data.len(),
            });
        }
        
        // For medical ultrasound, YUV is often just Y (luminance/grayscale)
        let mut rgba_data = Vec::with_capacity(width * height * 4);
        
        for &y_value in raw_frame.data.iter() {
            rgba_data.extend_from_slice(&[y_value, y_value, y_value, 255]);
        }
        
        Ok(Arc::from(rgba_data.into_boxed_slice()))
    }
    
    /// Convert grayscale to RGBA
    async fn convert_grayscale_to_rgba(&self, raw_frame: &RawFrame) -> Result<Arc<[u8]>, ProcessingError> {
        let width = raw_frame.header.width as usize;
        let height = raw_frame.header.height as usize;
        let expected_size = width * height;
        
        if raw_frame.data.len() != expected_size {
            return Err(ProcessingError::InvalidDataSize {
                expected: expected_size,
                actual: raw_frame.data.len(),
            });
        }
        
        let mut rgba_data = Vec::with_capacity(width * height * 4);
        
        for &gray_value in raw_frame.data.iter() {
            rgba_data.extend_from_slice(&[gray_value, gray_value, gray_value, 255]);
        }
        
        Ok(Arc::from(rgba_data.into_boxed_slice()))
    }
    
    /// Convert YUV10 (10-bit) to RGBA
    async fn convert_yuv10_to_rgba(&self, raw_frame: &RawFrame) -> Result<Arc<[u8]>, ProcessingError> {
        let width = raw_frame.header.width as usize;
        let height = raw_frame.header.height as usize;
        let expected_size = width * height * 2; // 10-bit packed data
        
        if raw_frame.data.len() != expected_size {
            return Err(ProcessingError::InvalidDataSize {
                expected: expected_size,
                actual: raw_frame.data.len(),
            });
        }
        
        let mut rgba_data = Vec::with_capacity(width * height * 4);
        
        // Convert 10-bit to 8-bit by right-shifting 2 bits
        for chunk in raw_frame.data.chunks_exact(2) {
            let value_10bit = u16::from_le_bytes([chunk[0], chunk[1]]);
            let value_8bit = (value_10bit >> 2) as u8; // Convert 10-bit to 8-bit
            rgba_data.extend_from_slice(&[value_8bit, value_8bit, value_8bit, 255]);
        }
        
        Ok(Arc::from(rgba_data.into_boxed_slice()))
    }
    
    /// Convert RGB10 (10-bit) to RGBA
    async fn convert_rgb10_to_rgba(&self, raw_frame: &RawFrame) -> Result<Arc<[u8]>, ProcessingError> {
        let width = raw_frame.header.width as usize;
        let height = raw_frame.header.height as usize;
        let expected_size = width * height * 6; // 3 channels * 2 bytes per 10-bit value
        
        if raw_frame.data.len() != expected_size {
            return Err(ProcessingError::InvalidDataSize {
                expected: expected_size,
                actual: raw_frame.data.len(),
            });
        }
        
        let mut rgba_data = Vec::with_capacity(width * height * 4);
        
        // Convert 10-bit RGB to 8-bit RGBA
        for chunk in raw_frame.data.chunks_exact(6) {
            let r_10bit = u16::from_le_bytes([chunk[0], chunk[1]]);
            let g_10bit = u16::from_le_bytes([chunk[2], chunk[3]]);
            let b_10bit = u16::from_le_bytes([chunk[4], chunk[5]]);
            
            let r_8bit = (r_10bit >> 2) as u8;
            let g_8bit = (g_10bit >> 2) as u8;
            let b_8bit = (b_10bit >> 2) as u8;
            
            rgba_data.extend_from_slice(&[r_8bit, g_8bit, b_8bit, 255]);
        }
        
        Ok(Arc::from(rgba_data.into_boxed_slice()))
    }
    
    /// Get processing statistics
    pub fn get_statistics(&self) -> ConversionStats {
        self.conversion_stats.read().clone()
    }
    
    /// Reset statistics
    pub fn reset_statistics(&self) {
        let mut stats = self.conversion_stats.write();
        *stats = ConversionStats::default();
    }
}

/// Check if SIMD instructions are available
fn is_simd_available() -> bool {
    // This is a simplified check - in a real implementation,
    // you would check for specific SIMD instruction sets
    #[cfg(target_arch = "x86_64")]
    {
        is_x86_feature_detected!("sse2") && is_x86_feature_detected!("avx2")
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}

/// Frame processing statistics
#[derive(Debug, Clone, Default)]
pub struct ConversionStats {
    pub frames_processed: u64,
    pub total_processing_time: std::time::Duration,
    pub last_conversion_time: std::time::Duration,
    pub average_processing_time_ms: f64,
}

impl ConversionStats {
    /// Update average processing time
    pub fn update_average(&mut self) {
        if self.frames_processed > 0 {
            self.average_processing_time_ms = 
                self.total_processing_time.as_millis() as f64 / self.frames_processed as f64;
        }
    }
    
    /// Get processing rate (frames per second)
    pub fn processing_rate(&self) -> f64 {
        if self.total_processing_time.as_secs_f64() > 0.0 {
            self.frames_processed as f64 / self.total_processing_time.as_secs_f64()
        } else {
            0.0
        }
    }
}

/// Frame processing errors
#[derive(Debug, thiserror::Error)]
pub enum ProcessingError {
    #[error("Invalid data size: expected {expected}, got {actual}")]
    InvalidDataSize {
        expected: usize,
        actual: usize,
    },
    
    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),
    
    #[error("Parallel processing error: {0}")]
    ParallelProcessing(String),
    
    #[error("SIMD processing error: {0}")]
    SimdProcessing(String),
    
    #[error("Memory allocation error: {0}")]
    MemoryAllocation(String),
    
    #[error("Other processing error: {0}")]
    Other(String),
}

impl FrameFormat {
    /// Convert to string representation
    pub fn to_string(&self) -> String {
        match self {
            FrameFormat::YUV => "YUV".to_string(),
            FrameFormat::BGR => "BGR".to_string(),
            FrameFormat::BGRA => "BGRA".to_string(),
            FrameFormat::RGB => "RGB".to_string(),
            FrameFormat::RGBA => "RGBA".to_string(),
            FrameFormat::YUV10 => "YUV10".to_string(),
            FrameFormat::RGB10 => "RGB10".to_string(),
            FrameFormat::Grayscale => "Grayscale".to_string(),
            FrameFormat::Unknown => "Unknown".to_string(),
        }
    }
}
