// src/frontend/mod.rs - Frontend Module for Medical Frame Viewer

pub mod app;
pub mod slint_bridge;
pub mod image_converter;
pub mod ui_state;

pub use app::MedicalFrameApp;
pub use slint_bridge::SlintBridge;
pub use image_converter::ImageConverter;
pub use ui_state::UiState;

use std::sync::Arc;
use tokio::sync::{mpsc, broadcast};
use tracing::{info, error};

use crate::backend::{
    MedicalFrameBackend, BackendCommand, BackendEvent, BackendConfig
};
use crate::frontend::image_converter::ImageConversionError;
use crate::frontend::slint_bridge::SlintBridgeError;

/// Frontend command for internal communication
#[derive(Debug, Clone)]
pub enum FrontendCommand {
    /// Update UI with new frame data (raw data, not Slint Image)
    UpdateFrame {
        frame_data: Arc<[u8]>,
        width: u32,
        height: u32,
        frame_id: u64,
        sequence_number: u64,
        resolution: String,
        format: String,
    },
    /// Update connection status
    UpdateConnectionStatus(String, bool),
    /// Update statistics
    UpdateStatistics(f64, f64, u64),
    /// Clear frame display
    ClearFrame,
}

/// Frontend service that manages the Slint UI and communicates with backend
pub struct MedicalFrameFrontend {
    // Backend communication
    backend: Arc<MedicalFrameBackend>,
    command_sender: mpsc::UnboundedSender<BackendCommand>,

    // UI components
    slint_bridge: Arc<SlintBridge>,
    ui_state: Arc<tokio::sync::RwLock<UiState>>,

    // Image processing
    image_converter: Arc<ImageConverter>,

    // Internal frontend communication
    frontend_command_tx: mpsc::UnboundedSender<FrontendCommand>,
    frontend_command_rx: Option<mpsc::UnboundedReceiver<FrontendCommand>>,
}

impl MedicalFrameFrontend {
    /// Create a new frontend service
    pub fn new(backend_config: BackendConfig) -> Result<Self, FrontendError> {
        info!("🎨 Initializing MiVi Medical Frame Frontend");

        // Create backend
        let backend = Arc::new(MedicalFrameBackend::new(backend_config.clone()));
        let command_sender = backend.get_command_sender();

        // Create UI components
        let slint_bridge = Arc::new(SlintBridge::new()?);
        let ui_state = Arc::new(tokio::sync::RwLock::new(UiState::new()));
        let image_converter = Arc::new(ImageConverter::new());

        // Create internal command channel
        let (frontend_command_tx, frontend_command_rx) = mpsc::unbounded_channel();

        Ok(Self {
            backend,
            command_sender,
            slint_bridge,
            ui_state,
            image_converter,
            frontend_command_tx,
            frontend_command_rx: Some(frontend_command_rx),
        })
    }

    /// Start the frontend service
    pub async fn start(&mut self) -> Result<(), FrontendError> {
        info!("🚀 Starting MiVi Medical Frame Frontend");

        // Start the backend
        self.backend.start().await
            .map_err(|e| FrontendError::Backend(e.to_string()))?;

        // Setup UI event handlers
        self.setup_ui_handlers().await?;

        // Take the frontend command receiver
        let mut frontend_command_rx = self.frontend_command_rx.take()
            .ok_or(FrontendError::Other("Frontend already started".to_string()))?;

        // Start event processing loop in background
        let event_processor = self.start_event_processing().await;

        // Start frontend command processing loop in main thread
        let slint_bridge = Arc::clone(&self.slint_bridge);
        let image_converter = Arc::clone(&self.image_converter);

        tokio::spawn(async move {
            while let Some(cmd) = frontend_command_rx.recv().await {
                if let Err(e) = Self::handle_frontend_command(cmd, &slint_bridge, &image_converter).await {
                    error!("Failed to handle frontend command: {}", e);
                }
            }
        });

        // Run the Slint UI (blocks until UI closes)
        info!("🎨 Starting Slint UI");
        self.slint_bridge.run().await?;

        // Cleanup
        event_processor.abort();
        info!("✅ MiVi Medical Frame Frontend stopped");

        Ok(())
    }

    /// Handle frontend commands (runs on main thread)
    async fn handle_frontend_command(
        command: FrontendCommand,
        slint_bridge: &Arc<SlintBridge>,
        image_converter: &Arc<ImageConverter>,
    ) -> Result<(), FrontendError> {
        match command {
            FrontendCommand::UpdateFrame { frame_data, width, height, frame_id, sequence_number, resolution, format } => {
                // Convert raw data to Slint image on main thread
                match image_converter.create_slint_image_from_rgba(&frame_data, width, height) {
                    Ok(slint_image) => {
                        slint_bridge.update_frame(
                            slint_image,
                            &resolution,
                            &format,
                            frame_id as i32,
                            sequence_number as i32,
                        ).await?;
                    }
                    Err(e) => {
                        error!("Failed to create Slint image: {}", e);
                        // Create error image
                        match image_converter.create_error_image(width, height, &e.to_string()).await {
                            Ok(error_image) => {
                                slint_bridge.update_frame(
                                    error_image,
                                    &resolution,
                                    "Error",
                                    frame_id as i32,
                                    sequence_number as i32,
                                ).await?;
                            }
                            Err(ie) => {
                                error!("Failed to create error image: {}", ie);
                            }
                        }
                    }
                }
            }
            FrontendCommand::UpdateConnectionStatus(status, connected) => {
                slint_bridge.update_connection_status(&status, connected).await?;
            }
            FrontendCommand::UpdateStatistics(fps, latency, total_frames) => {
                slint_bridge.update_statistics(fps as f32, latency as f32, total_frames as i32).await?;
            }
            FrontendCommand::ClearFrame => {
                slint_bridge.clear_frame().await?;
            }
        }
        Ok(())
    }

    /// Setup UI event handlers
    async fn setup_ui_handlers(&self) -> Result<(), FrontendError> {
        let command_sender = self.command_sender.clone();
        let ui_state = Arc::clone(&self.ui_state);

        // Setup reconnect button handler
        {
            let command_sender = command_sender.clone();
            let ui_state = Arc::clone(&ui_state);

            self.slint_bridge.on_reconnect_clicked(move || {
                let command_sender = command_sender.clone();
                let ui_state = Arc::clone(&ui_state);

                tokio::spawn(async move {
                    let state = ui_state.read().await;
                    let config = state.get_backend_config();
                    let shm_name = state.shm_name.clone();

                    let _ = command_sender.send(BackendCommand::Connect {
                        shm_name,
                        config
                    });
                });
            }).await?;
        }

        // Setup catch-up mode toggle
        {
            let command_sender = command_sender.clone();

            self.slint_bridge.on_toggle_catch_up(move |enabled| {
                let command_sender = command_sender.clone();

                tokio::spawn(async move {
                    let _ = command_sender.send(BackendCommand::SetCatchUpMode(enabled));
                });
            }).await?;
        }

        // Setup settings handler
        {
            self.slint_bridge.on_settings_clicked(move || {
                // Open settings dialog (implement as needed)
                info!("⚙️ Settings clicked");
            }).await?;
        }

        // Setup about handler
        {
            self.slint_bridge.on_about_clicked(move || {
                // Show about dialog (implement as needed)
                info!("ℹ️ About clicked");
            }).await?;
        }

        Ok(())
    }

    /// Start event processing from backend (background thread)
    async fn start_event_processing(&mut self) -> tokio::task::JoinHandle<()> {
        let mut event_receiver = self.backend.get_event_receiver();
        let ui_state = Arc::clone(&self.ui_state);
        let frontend_command_tx = self.frontend_command_tx.clone();

        tokio::spawn(async move {
            info!("🔄 Starting backend event processing");

            while let Ok(event) = event_receiver.recv().await {
                match event {
                    BackendEvent::Connected => {
                        info!("✅ Backend connected");

                        // Update UI state
                        {
                            let mut state = ui_state.write().await;
                            state.is_connected = true;
                            state.connection_status = "Connected".to_string();
                        }

                        // Send frontend command
                        let _ = frontend_command_tx.send(FrontendCommand::UpdateConnectionStatus("Connected".to_string(), true));
                    }

                    BackendEvent::Disconnected => {
                        info!("🔌 Backend disconnected");

                        // Update UI state
                        {
                            let mut state = ui_state.write().await;
                            state.is_connected = false;
                            state.connection_status = "Disconnected".to_string();
                            state.has_frame = false;
                        }

                        // Send frontend commands
                        let _ = frontend_command_tx.send(FrontendCommand::UpdateConnectionStatus("Disconnected".to_string(), false));
                        let _ = frontend_command_tx.send(FrontendCommand::ClearFrame);
                    }

                    BackendEvent::ConnectionError(error) => {
                        error!("❌ Backend connection error: {}", error);

                        // Update UI state
                        {
                            let mut state = ui_state.write().await;
                            state.is_connected = false;
                            state.connection_status = format!("Error: {}", error);
                        }

                        // Send frontend command
                        let _ = frontend_command_tx.send(FrontendCommand::UpdateConnectionStatus(format!("Error: {}", error), false));
                    }

                    BackendEvent::ConnectionLost => {
                        info!("⚠️ Backend connection lost, attempting reconnection");

                        // Update UI state
                        {
                            let mut state = ui_state.write().await;
                            state.connection_status = "Reconnecting...".to_string();
                        }

                        // Send frontend command
                        let _ = frontend_command_tx.send(FrontendCommand::UpdateConnectionStatus("Reconnecting...".to_string(), false));
                    }

                    BackendEvent::NewFrame(processed_frame) => {
                        // Update UI state
                        {
                            let mut state = ui_state.write().await;
                            state.has_frame = true;
                            state.frame_id = processed_frame.header.frame_id as i32;
                            state.sequence_number = processed_frame.header.sequence_number as i32;
                            state.resolution = processed_frame.resolution_string();
                            state.frame_format = processed_frame.format_string();
                            state.last_frame_time = std::time::Instant::now();
                        }

                        // Send frontend command with raw data (avoid sending Slint Image across threads)
                        let _ = frontend_command_tx.send(FrontendCommand::UpdateFrame {
                            frame_data: processed_frame.rgb_data.clone(),
                            width: processed_frame.header.width,
                            height: processed_frame.header.height,
                            frame_id: processed_frame.header.frame_id,
                            sequence_number: processed_frame.header.sequence_number,
                            resolution: processed_frame.resolution_string(),
                            format: processed_frame.format_string(),
                        });
                    }

                    BackendEvent::StatisticsUpdate(stats) => {
                        // Update UI state with statistics
                        {
                            let mut state = ui_state.write().await;
                            state.fps = stats.current_fps as f32;
                            state.latency_ms = stats.average_latency_ms as f32;
                            state.total_frames = stats.total_frames_received as i32;
                        }

                        // Send frontend command
                        let _ = frontend_command_tx.send(FrontendCommand::UpdateStatistics(
                            stats.current_fps,
                            stats.average_latency_ms,
                            stats.total_frames_received,
                        ));
                    }

                    BackendEvent::SettingsChanged => {
                        info!("⚙️ Backend settings changed");
                        // Handle settings changes if needed
                    }
                }
            }

            info!("🔄 Backend event processing stopped");
        })
    }

    /// Send a command to the backend
    pub async fn send_command(&self, command: BackendCommand) -> Result<(), FrontendError> {
        self.command_sender.send(command)
            .map_err(|e| FrontendError::Communication(e.to_string()))
    }

    /// Get current UI state
    pub async fn get_ui_state(&self) -> UiState {
        self.ui_state.read().await.clone()
    }

    /// Update UI state
    pub async fn update_ui_state<F>(&self, updater: F) -> Result<(), FrontendError>
    where
        F: FnOnce(&mut UiState),
    {
        let mut state = self.ui_state.write().await;
        updater(&mut state);
        Ok(())
    }
}

// Add method to ImageConverter for creating Slint images from raw RGBA data
impl ImageConverter {
    /// Create Slint image from raw RGBA data (helper method)
    pub fn create_slint_image_from_rgba(&self, rgba_data: &[u8], width: u32, height: u32) -> Result<slint::Image, ImageConversionError> {
        self.create_slint_image_optimized(rgba_data, width, height)
    }
}

/// Frontend errors
#[derive(Debug, thiserror::Error)]
pub enum FrontendError {
    #[error("Backend error: {0}")]
    Backend(String),

    #[error("UI error: {0}")]
    Ui(String),

    #[error("Communication error: {0}")]
    Communication(String),

    #[error("Image conversion error: {0}")]
    ImageConversion(String),

    #[error("Slint error: {0}")]
    Slint(String),

    #[error("Other frontend error: {0}")]
    Other(String),
}

impl From<SlintBridgeError> for FrontendError {
    fn from(err: SlintBridgeError) -> Self {
        FrontendError::Slint(err.to_string())
    }
}

impl From<crate::frontend::image_converter::ImageConversionError> for FrontendError {
    fn from(err: crate::frontend::image_converter::ImageConversionError) -> Self {
        FrontendError::ImageConversion(err.to_string())
    }
}