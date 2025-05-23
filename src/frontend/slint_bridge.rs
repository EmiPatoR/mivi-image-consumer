// src/frontend/slint_bridge.rs - Bridge between Rust backend and Slint UI

use std::sync::Arc;
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use tracing::{info, error, debug};

// Include the generated Slint code
slint::include_modules!();

/// Bridge for interfacing with Slint UI
pub struct SlintBridge {
    main_window: MainWindow,
}

impl SlintBridge {
    /// Create a new Slint bridge
    pub fn new() -> Result<Self, SlintBridgeError> {
        info!("🎨 Initializing Slint UI bridge");

        let main_window = MainWindow::new()
            .map_err(|e| SlintBridgeError::Creation(e.to_string()))?;

        // Initialize UI state
        Self::initialize_ui_state(&main_window)?;

        Ok(Self { main_window })
    }

    /// Initialize default UI state
    fn initialize_ui_state(window: &MainWindow) -> Result<(), SlintBridgeError> {
        // Set initial values
        window.set_connection_status("Disconnected - Waiting for medical device".into());
        window.set_shm_name("ultrasound_frames".into());
        window.set_format("YUV".into());
        window.set_resolution("0x0".into());
        window.set_fps(0.0);
        window.set_latency_ms(0.0);
        window.set_total_frames(0);
        window.set_catch_up_mode(false);
        window.set_is_connected(false);
        window.set_has_frame(false);
        window.set_frame_id(0);
        window.set_sequence_number(0);
        window.set_frame_format("Unknown".into());

        info!("✅ Slint UI state initialized");
        Ok(())
    }

    /// Setup reconnect button callback
    pub async fn on_reconnect_clicked<F>(&self, callback: F) -> Result<(), SlintBridgeError>
    where
        F: Fn() + Send + Sync + 'static,
    {
        let callback = Arc::new(callback);
        self.main_window.on_reconnect_clicked(move || {
            callback();
        });
        Ok(())
    }

    /// Setup catch-up mode toggle callback
    pub async fn on_toggle_catch_up<F>(&self, callback: F) -> Result<(), SlintBridgeError>
    where
        F: Fn(bool) + Send + Sync + 'static,
    {
        let callback = Arc::new(callback);
        let main_window_weak = self.main_window.as_weak();
        self.main_window.on_toggle_catch_up(move || {
            if let Some(window) = main_window_weak.upgrade() {
                let current_mode = window.get_catch_up_mode();
                callback(!current_mode);
            }
        });
        Ok(())
    }

    /// Setup settings button callback
    pub async fn on_settings_clicked<F>(&self, callback: F) -> Result<(), SlintBridgeError>
    where
        F: Fn() + Send + Sync + 'static,
    {
        let callback = Arc::new(callback);
        self.main_window.on_settings_clicked(move || {
            callback();
        });
        Ok(())
    }

    /// Setup about button callback
    pub async fn on_about_clicked<F>(&self, callback: F) -> Result<(), SlintBridgeError>
    where
        F: Fn() + Send + Sync + 'static,
    {
        let callback = Arc::new(callback);
        self.main_window.on_about_clicked(move || {
            callback();
        });
        Ok(())
    }

    /// Update connection status in the UI
    pub async fn update_connection_status(&self, status: &str, connected: bool) -> Result<(), SlintBridgeError> {
        let status = status.to_string();
        let main_window = self.main_window.as_weak();

        slint::invoke_from_event_loop(move || {
            if let Some(window) = main_window.upgrade() {
                window.set_connection_status(status.clone().into());
                window.set_is_connected(connected);

                debug!("🔄 UI connection status updated: {} (connected: {})", status.clone(), connected);
            }
        }).map_err(|e| SlintBridgeError::UiUpdate(e.to_string()))?;

        Ok(())
    }

    /// Update frame in the UI
    pub async fn update_frame(
        &self,
        image: Image,
        resolution: &str,
        format: &str,
        frame_id: i32,
        sequence_number: i32,
    ) -> Result<(), SlintBridgeError> {
        let resolution = resolution.to_string();
        let format = format.to_string();
        let main_window = self.main_window.as_weak();

        // Move the image to the UI thread
        slint::invoke_from_event_loop(move || {
            if let Some(window) = main_window.upgrade() {
                window.set_current_frame(image);
                window.set_resolution(resolution.clone().into());
                window.set_frame_format(format.clone().into());
                window.set_frame_id(frame_id);
                window.set_sequence_number(sequence_number);
                window.set_has_frame(true);

                debug!("🖼️ UI frame updated: {} {}", resolution.clone(), format.clone());
            }
        }).map_err(|e| SlintBridgeError::UiUpdate(e.to_string()))?;

        Ok(())
    }

    /// Update statistics in the UI
    pub async fn update_statistics(
        &self,
        fps: f32,
        latency_ms: f32,
        total_frames: i32,
    ) -> Result<(), SlintBridgeError> {
        let main_window = self.main_window.as_weak();

        slint::invoke_from_event_loop(move || {
            if let Some(window) = main_window.upgrade() {
                window.set_fps(fps);
                window.set_latency_ms(latency_ms);
                window.set_total_frames(total_frames);

                if fps > 0.0 {
                    debug!("📊 UI stats updated: {:.1} FPS, {:.1}ms latency, {} frames",
                           fps, latency_ms, total_frames);
                }
            }
        }).map_err(|e| SlintBridgeError::UiUpdate(e.to_string()))?;

        Ok(())
    }

    /// Update configuration in the UI
    pub async fn update_config(&self, shm_name: &str, format: &str) -> Result<(), SlintBridgeError> {
        let shm_name = shm_name.to_string();
        let format = format.to_string();
        let main_window = self.main_window.as_weak();

        slint::invoke_from_event_loop(move || {
            let shm_str_name = shm_name.clone();
            let format_str = format.clone();

            if let Some(window) = main_window.upgrade() {
                window.set_shm_name(shm_name.into());
                window.set_format(format.into());

                debug!("⚙️ UI config updated: {} ({})", shm_str_name, format_str);
            }
        }).map_err(|e| SlintBridgeError::UiUpdate(e.to_string()))?;

        Ok(())
    }

    /// Set catch-up mode in the UI
    pub async fn set_catch_up_mode(&self, enabled: bool) -> Result<(), SlintBridgeError> {
        let main_window = self.main_window.as_weak();

        slint::invoke_from_event_loop(move || {
            if let Some(window) = main_window.upgrade() {
                window.set_catch_up_mode(enabled);
                debug!("⚙️ UI catch-up mode: {}", enabled);
            }
        }).map_err(|e| SlintBridgeError::UiUpdate(e.to_string()))?;

        Ok(())
    }

    /// Get current catch-up mode from UI
    pub fn catch_up_mode(&self) -> bool {
        self.main_window.get_catch_up_mode()
    }

    /// Get current shared memory name from UI
    pub fn shm_name(&self) -> String {
        self.main_window.get_shm_name().to_string()
    }

    /// Show a notification or status message
    pub async fn show_notification(&self, message: &str, is_error: bool) -> Result<(), SlintBridgeError> {
        let message = message.to_string();
        let main_window = self.main_window.as_weak();

        slint::invoke_from_event_loop(move || {
            if let Some(window) = main_window.upgrade() {
                // For now, update the connection status to show the notification
                // In a more complex implementation, you might have a separate notification area
                let status = if is_error {
                    format!("Error: {}", message)
                } else {
                    format!("Info: {}", message)
                };
                window.set_connection_status(status.into());

                info!("📢 UI notification: {} (error: {})", message, is_error);
            }
        }).map_err(|e| SlintBridgeError::UiUpdate(e.to_string()))?;

        Ok(())
    }

    /// Clear the current frame from the UI
    pub async fn clear_frame(&self) -> Result<(), SlintBridgeError> {
        let main_window = self.main_window.as_weak();

        slint::invoke_from_event_loop(move || {
            if let Some(window) = main_window.upgrade() {
                window.set_has_frame(false);
                window.set_frame_id(0);
                window.set_sequence_number(0);
                window.set_resolution("0x0".into());
                window.set_frame_format("Unknown".into());

                debug!("🧹 UI frame cleared");
            }
        }).map_err(|e| SlintBridgeError::UiUpdate(e.to_string()))?;

        Ok(())
    }

    /// Run the Slint UI event loop
    pub async fn run(&self) -> Result<(), SlintBridgeError> {
        info!("🚀 Starting Slint UI event loop");

        // Show the window
        self.main_window.show()
            .map_err(|e| SlintBridgeError::Display(e.to_string()))?;

        // Run the event loop
        slint::run_event_loop()
            .map_err(|e| SlintBridgeError::EventLoop(e.to_string()))?;

        info!("✅ Slint UI event loop finished");
        Ok(())
    }

    /// Hide the main window
    pub async fn hide(&self) -> Result<(), SlintBridgeError> {
        self.main_window.hide()
            .map_err(|e| SlintBridgeError::Display(e.to_string()))?;
        Ok(())
    }

    /// Get a weak reference to the main window for callbacks
    pub fn get_weak_window(&self) -> slint::Weak<MainWindow> {
        self.main_window.as_weak()
    }

    /// Create a Slint image from RGBA data
    pub fn create_image_from_rgba(
        &self,
        rgba_data: &[u8],
        width: u32,
        height: u32,
    ) -> Result<Image, SlintBridgeError> {
        // Ensure data size is correct
        let expected_size = (width * height * 4) as usize;
        if rgba_data.len() != expected_size {
            return Err(SlintBridgeError::InvalidImageData {
                expected: expected_size,
                actual: rgba_data.len(),
            });
        }

        // Create pixel buffer
        let mut pixel_buffer = SharedPixelBuffer::<Rgba8Pixel>::new(width, height);

        // Copy RGBA data
        let pixels = pixel_buffer.make_mut_bytes();
        pixels.copy_from_slice(rgba_data);

        // Create Slint image
        Ok(Image::from_rgba8(pixel_buffer))
    }

    /// Quit the application
    pub async fn quit(&self) -> Result<(), SlintBridgeError> {
        info!("🛑 Quitting application");
        slint::quit_event_loop();
        Ok(())
    }
}

/// Slint bridge errors
#[derive(Debug, thiserror::Error)]
pub enum SlintBridgeError {
    #[error("Failed to create Slint UI: {0}")]
    Creation(String),

    #[error("Failed to display UI: {0}")]
    Display(String),

    #[error("Event loop error: {0}")]
    EventLoop(String),

    #[error("UI update error: {0}")]
    UiUpdate(String),

    #[error("Invalid image data: expected {expected} bytes, got {actual}")]
    InvalidImageData {
        expected: usize,
        actual: usize,
    },

    #[error("Image creation error: {0}")]
    ImageCreation(String),

    #[error("Callback setup error: {0}")]
    CallbackSetup(String),

    #[error("Thread synchronization error: {0}")]
    ThreadSync(String),

    #[error("Other Slint error: {0}")]
    Other(String),
}

// Implement Send and Sync for SlintBridge
unsafe impl Send for SlintBridge {}
unsafe impl Sync for SlintBridge {}