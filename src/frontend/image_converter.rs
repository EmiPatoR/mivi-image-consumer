// src/frontend/image_converter.rs - Zero-Copy Image Converter for Slint

use std::sync::Arc;
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use tracing::{debug, warn, error};
use lru::LruCache;
use crate::backend::types::ProcessedFrame;

/// Image converter for converting backend frames to Slint images
/// Optimized for zero-copy operations where possible
pub struct ImageConverter {
    // Conversion statistics
    conversion_stats: parking_lot::RwLock<ImageConversionStats>,
    
    // Performance settings
    enable_caching: bool,
    max_cache_size: usize,
    
    // Image cache for frequently used images
    image_cache: parking_lot::RwLock<LruCache<u64, Image>>,
}

impl ImageConverter {
    /// Create a new image converter
    pub fn new() -> Self {
        Self {
            conversion_stats: parking_lot::RwLock::new(ImageConversionStats::default()),
            enable_caching: false, // Disabled for medical imaging to ensure fresh data
            max_cache_size: 10,
            image_cache: parking_lot::RwLock::new(LruCache::new(
                std::num::NonZeroUsize::new(10).unwrap()
            )),
        }
    }
    
    /// Convert a processed frame to a Slint image (zero-copy optimized)
    pub async fn convert_to_slint_image(&self, frame: &ProcessedFrame) -> Result<Image, ImageConversionError> {
        let start_time = std::time::Instant::now();
        
        // Check cache first (if enabled and appropriate for medical imaging)
        if self.enable_caching {
            if let Some(cached_image) = self.get_cached_image(frame.header.frame_id) {
                let mut stats = self.conversion_stats.write();
                stats.cache_hits += 1;
                return Ok(cached_image);
            }
        }
        
        // Get frame dimensions
        let (width, height) = frame.dimensions();
        
        // Validate dimensions
        if width == 0 || height == 0 {
            return Err(ImageConversionError::InvalidDimensions { width, height });
        }
        
        // Validate data size (expecting RGBA format from backend)
        let expected_size = (width * height * 4) as usize;
        if frame.rgb_data.len() != expected_size {
            return Err(ImageConversionError::InvalidDataSize {
                expected: expected_size,
                actual: frame.rgb_data.len(),
                width,
                height,
            });
        }
        
        debug!("🖼️ Converting frame {} to Slint image: {}x{}", 
               frame.header.frame_id, width, height);
        
        // Create the Slint image with zero-copy where possible
        let image = self.create_slint_image_optimized(&frame.rgb_data, width, height)?;
        
        // Cache the image if enabled
        if self.enable_caching {
            self.cache_image(frame.header.frame_id, image.clone());
        }
        
        // Update statistics
        {
            let mut stats = self.conversion_stats.write();
            stats.images_converted += 1;
            stats.total_conversion_time += start_time.elapsed();
            stats.last_conversion_time = start_time.elapsed();
            stats.total_pixels_processed += (width * height) as u64;
            
            if stats.images_converted % 60 == 0 {
                stats.update_averages();
                debug!("📊 Image conversion stats: {} images, avg {:.2}ms, {:.1} fps", 
                       stats.images_converted, stats.average_conversion_time_ms, stats.conversion_fps());
            }
        }
        
        Ok(image)
    }
    
    /// Create Slint image with optimization for zero-copy
    fn create_slint_image_optimized(
        &self,
        rgba_data: &[u8],
        width: u32,
        height: u32,
    ) -> Result<Image, ImageConversionError> {
        // Create shared pixel buffer
        let mut pixel_buffer = SharedPixelBuffer::<Rgba8Pixel>::new(width, height);
        
        // Get mutable access to the pixel buffer
        let buffer_slice = pixel_buffer.make_mut_bytes();
        
        // Direct memory copy (most efficient for large medical images)
        if rgba_data.len() == buffer_slice.len() {
            buffer_slice.copy_from_slice(rgba_data);
        } else {
            return Err(ImageConversionError::BufferSizeMismatch {
                source: rgba_data.len(),
                target: buffer_slice.len(),
            });
        }
        
        // Create Slint image from pixel buffer
        Ok(Image::from_rgba8(pixel_buffer))
    }
    
    /// Get cached image if available
    fn get_cached_image(&self, frame_id: u64) -> Option<Image> {
        self.image_cache.write().get(&frame_id).cloned()
    }
    
    /// Cache an image
    fn cache_image(&self, frame_id: u64, image: Image) {
        self.image_cache.write().put(frame_id, image);
    }
    
    /// Create a placeholder image for when no frame is available
    pub async fn create_placeholder_image(&self, width: u32, height: u32) -> Result<Image, ImageConversionError> {
        debug!("🖼️ Creating placeholder image: {}x{}", width, height);
        
        // Create a simple gradient placeholder
        let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);
        
        for y in 0..height {
            for x in 0..width {
                // Create a subtle medical-themed gradient
                let intensity = ((x + y) % 128) as u8;
                let blue_tint = (intensity / 2) as u8;
                
                rgba_data.extend_from_slice(&[
                    30 + intensity / 8,      // R - subtle red
                    40 + intensity / 6,      // G - subtle green  
                    60 + blue_tint,          // B - medical blue tint
                    255,                     // A - fully opaque
                ]);
            }
        }
        
        self.create_slint_image_optimized(&rgba_data, width, height)
    }
    
    /// Create an error image when frame conversion fails
    pub async fn create_error_image(&self, width: u32, height: u32, error_msg: &str) -> Result<Image, ImageConversionError> {
        warn!("🖼️ Creating error image: {}x{} - {}", width, height, error_msg);
        
        // Create a red-tinted error image
        let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);
        
        for y in 0..height {
            for x in 0..width {
                // Create a pattern that indicates an error
                let is_border = x < 10 || x >= width - 10 || y < 10 || y >= height - 10;
                let is_diagonal = (x + y) % 40 < 20;
                
                if is_border || is_diagonal {
                    rgba_data.extend_from_slice(&[180, 50, 50, 255]); // Red error pattern
                } else {
                    rgba_data.extend_from_slice(&[60, 30, 30, 255]);  // Dark red background
                }
            }
        }
        
        self.create_slint_image_optimized(&rgba_data, width, height)
    }
    
    /// Convert raw medical imaging data to Slint image
    /// This is for cases where we need to bypass the backend processing
    pub async fn convert_raw_medical_data(
        &self,
        raw_data: &[u8],
        width: u32,
        height: u32,
        format: MedicalImageFormat,
    ) -> Result<Image, ImageConversionError> {
        debug!("🏥 Converting raw medical data: {}x{} {:?}", width, height, format);
        
        let rgba_data = match format {
            MedicalImageFormat::Grayscale8 => {
                self.convert_grayscale_to_rgba(raw_data, width, height)?
            }
            MedicalImageFormat::Grayscale16 => {
                self.convert_grayscale16_to_rgba(raw_data, width, height)?
            }
            MedicalImageFormat::RGB24 => {
                self.convert_rgb24_to_rgba(raw_data, width, height)?
            }
            MedicalImageFormat::BGR24 => {
                self.convert_bgr24_to_rgba(raw_data, width, height)?
            }
            MedicalImageFormat::RGBA32 => {
                raw_data.to_vec() // Already RGBA
            }
            MedicalImageFormat::YUV420 => {
                self.convert_yuv420_to_rgba(raw_data, width, height)?
            }
        };
        
        self.create_slint_image_optimized(&rgba_data, width, height)
    }
    
    /// Convert grayscale to RGBA
    fn convert_grayscale_to_rgba(&self, data: &[u8], width: u32, height: u32) -> Result<Vec<u8>, ImageConversionError> {
        let expected_size = (width * height) as usize;
        if data.len() != expected_size {
            return Err(ImageConversionError::InvalidDataSize {
                expected: expected_size,
                actual: data.len(),
                width,
                height,
            });
        }
        
        let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);
        for &gray in data {
            rgba_data.extend_from_slice(&[gray, gray, gray, 255]);
        }
        
        Ok(rgba_data)
    }
    
    /// Convert 16-bit grayscale to RGBA
    fn convert_grayscale16_to_rgba(&self, data: &[u8], width: u32, height: u32) -> Result<Vec<u8>, ImageConversionError> {
        let expected_size = (width * height * 2) as usize;
        if data.len() != expected_size {
            return Err(ImageConversionError::InvalidDataSize {
                expected: expected_size,
                actual: data.len(),
                width,
                height,
            });
        }
        
        let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);
        for chunk in data.chunks_exact(2) {
            let gray16 = u16::from_le_bytes([chunk[0], chunk[1]]);
            let gray8 = (gray16 >> 8) as u8; // Convert 16-bit to 8-bit
            rgba_data.extend_from_slice(&[gray8, gray8, gray8, 255]);
        }
        
        Ok(rgba_data)
    }
    
    /// Convert RGB24 to RGBA
    fn convert_rgb24_to_rgba(&self, data: &[u8], width: u32, height: u32) -> Result<Vec<u8>, ImageConversionError> {
        let expected_size = (width * height * 3) as usize;
        if data.len() != expected_size {
            return Err(ImageConversionError::InvalidDataSize {
                expected: expected_size,
                actual: data.len(),
                width,
                height,
            });
        }
        
        let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);
        for chunk in data.chunks_exact(3) {
            rgba_data.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
        }
        
        Ok(rgba_data)
    }
    
    /// Convert BGR24 to RGBA
    fn convert_bgr24_to_rgba(&self, data: &[u8], width: u32, height: u32) -> Result<Vec<u8>, ImageConversionError> {
        let expected_size = (width * height * 3) as usize;
        if data.len() != expected_size {
            return Err(ImageConversionError::InvalidDataSize {
                expected: expected_size,
                actual: data.len(),
                width,
                height,
            });
        }
        
        let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);
        for chunk in data.chunks_exact(3) {
            rgba_data.extend_from_slice(&[chunk[2], chunk[1], chunk[0], 255]); // BGR -> RGB
        }
        
        Ok(rgba_data)
    }
    
    /// Convert YUV420 to RGBA (simplified implementation)
    fn convert_yuv420_to_rgba(&self, data: &[u8], width: u32, height: u32) -> Result<Vec<u8>, ImageConversionError> {
        // This is a simplified YUV420 to RGB conversion
        // In a production medical imaging system, you'd want a more sophisticated conversion
        let y_size = (width * height) as usize;
        let expected_size = y_size + (y_size / 2); // YUV420 format
        
        if data.len() != expected_size {
            return Err(ImageConversionError::InvalidDataSize {
                expected: expected_size,
                actual: data.len(),
                width,
                height,
            });
        }
        
        let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);
        
        // For simplicity, just use the Y component (luminance) as grayscale
        for &y in &data[..y_size] {
            rgba_data.extend_from_slice(&[y, y, y, 255]);
        }
        
        Ok(rgba_data)
    }
    
    /// Get conversion statistics
    pub fn get_statistics(&self) -> ImageConversionStats {
        self.conversion_stats.read().clone()
    }
    
    /// Reset conversion statistics
    pub fn reset_statistics(&self) {
        let mut stats = self.conversion_stats.write();
        *stats = ImageConversionStats::default();
    }
    
    /// Clear image cache
    pub fn clear_cache(&self) {
        self.image_cache.write().clear();
        let mut stats = self.conversion_stats.write();
        stats.cache_clears += 1;
    }
    
    /// Enable or disable image caching
    pub fn set_caching_enabled(&mut self, enabled: bool) {
        self.enable_caching = enabled;
        if !enabled {
            self.clear_cache();
        }
    }
}

/// Medical image formats supported
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MedicalImageFormat {
    Grayscale8,    // 8-bit grayscale (common in ultrasound)
    Grayscale16,   // 16-bit grayscale (high-precision medical imaging)
    RGB24,         // 24-bit RGB
    BGR24,         // 24-bit BGR (common in medical cameras)
    RGBA32,        // 32-bit RGBA
    YUV420,        // YUV 4:2:0 (common in video streams)
}

/// Image conversion statistics
#[derive(Debug, Clone, Default)]
pub struct ImageConversionStats {
    pub images_converted: u64,
    pub total_conversion_time: std::time::Duration,
    pub last_conversion_time: std::time::Duration,
    pub average_conversion_time_ms: f64,
    pub total_pixels_processed: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub cache_clears: u64,
}

impl ImageConversionStats {
    /// Update average conversion time
    pub fn update_averages(&mut self) {
        if self.images_converted > 0 {
            self.average_conversion_time_ms = 
                self.total_conversion_time.as_millis() as f64 / self.images_converted as f64;
        }
    }
    
    /// Calculate conversion FPS
    pub fn conversion_fps(&self) -> f64 {
        if self.total_conversion_time.as_secs_f64() > 0.0 {
            self.images_converted as f64 / self.total_conversion_time.as_secs_f64()
        } else {
            0.0
        }
    }
    
    /// Get cache hit rate percentage
    pub fn cache_hit_rate(&self) -> f64 {
        let total_requests = self.cache_hits + self.cache_misses;
        if total_requests > 0 {
            (self.cache_hits as f64 / total_requests as f64) * 100.0
        } else {
            0.0
        }
    }
    
    /// Get average pixels per second
    pub fn pixels_per_second(&self) -> f64 {
        if self.total_conversion_time.as_secs_f64() > 0.0 {
            self.total_pixels_processed as f64 / self.total_conversion_time.as_secs_f64()
        } else {
            0.0
        }
    }
}

/// Image conversion errors
#[derive(Debug, thiserror::Error)]
pub enum ImageConversionError {
    #[error("Invalid dimensions: {width}x{height}")]
    InvalidDimensions {
        width: u32,
        height: u32,
    },

    #[error("Invalid data size: expected {expected} bytes for {width}x{height}, got {actual}")]
    InvalidDataSize {
        expected: usize,
        actual: usize,
        width: u32,
        height: u32,
    },

    #[error("Buffer size mismatch: source {source} bytes, target {target} bytes")]
    BufferSizeMismatch {
        source: usize,
        target: usize,
    },

    #[error("Unsupported format: {0:?}")]
    UnsupportedFormat(MedicalImageFormat),

    #[error("Slint image creation failed: {0}")]
    SlintImageCreation(String),

    #[error("Memory allocation failed: {0}")]
    MemoryAllocation(String),

    #[error("Other conversion error: {0}")]
    Other(String),
}
