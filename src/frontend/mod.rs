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
    MedicalFrameBackend, BackendCommand, BackendEvent, BackendState, BackendConfig
};
use crate::frontend::slint_bridge::SlintBridgeError;

/// Frontend service that manages the Slint UI and communicates with backend
pub struct MedicalFrameFrontend {
    // Backend communication
    backend: Arc<MedicalFrameBackend>,
    command_sender: mpsc::UnboundedSender<BackendCommand>,
    event_receiver: broadcast::Receiver<BackendEvent>,

    // UI components
    slint_bridge: Arc<SlintBridge>,
    ui_state: Arc<tokio::sync::RwLock<UiState>>,

    // Image processing
    image_converter: Arc<ImageConverter>,
}

impl MedicalFrameFrontend {
    /// Create a new frontend service
    pub fn new(backend_config: BackendConfig) -> Result<Self, FrontendError> {
        info!("🎨 Initializing MiVi Medical Frame Frontend");

        // Create backend
        let backend = Arc::new(MedicalFrameBackend::new(backend_config.clone()));
        let command_sender = backend.get_command_sender();
        let event_receiver = backend.get_event_receiver();

        // Create UI components
        let slint_bridge = Arc::new(SlintBridge::new()?);
        let ui_state = Arc::new(tokio::sync::RwLock::new(UiState::new()));
        let image_converter = Arc::new(ImageConverter::new());

        Ok(Self {
            backend,
            command_sender,
            event_receiver,
            slint_bridge,
            ui_state,
            image_converter,
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

        // Start event processing loop
        let event_processor = self.start_event_processing().await;

        // Run the Slint UI
        info!("🎨 Starting Slint UI");
        self.slint_bridge.run().await?;

        // Cleanup
        event_processor.abort();
        info!("✅ MiVi Medical Frame Frontend stopped");

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

    /// Start event processing from backend
    async fn start_event_processing(&mut self) -> tokio::task::JoinHandle<()> {
        let mut event_receiver = self.event_receiver.resubscribe();
        let slint_bridge = Arc::clone(&self.slint_bridge);
        let ui_state = Arc::clone(&self.ui_state);
        let image_converter = Arc::clone(&self.image_converter);

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

                        // Update UI
                        if let Err(e) = slint_bridge.update_connection_status("Connected", true).await {
                            error!("Failed to update UI connection status: {}", e);
                        }
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

                        // Update UI
                        if let Err(e) = slint_bridge.update_connection_status("Disconnected", false).await {
                            error!("Failed to update UI connection status: {}", e);
                        }
                    }

                    BackendEvent::ConnectionError(error) => {
                        error!("❌ Backend connection error: {}", error);

                        // Update UI state
                        {
                            let mut state = ui_state.write().await;
                            state.is_connected = false;
                            state.connection_status = format!("Error: {}", error);
                        }

                        // Update UI
                        if let Err(e) = slint_bridge.update_connection_status(&format!("Error: {}", error), false).await {
                            error!("Failed to update UI connection status: {}", e);
                        }
                    }

                    BackendEvent::ConnectionLost => {
                        info!("⚠️ Backend connection lost, attempting reconnection");

                        // Update UI state
                        {
                            let mut state = ui_state.write().await;
                            state.connection_status = "Reconnecting...".to_string();
                        }

                        // Update UI
                        if let Err(e) = slint_bridge.update_connection_status("Reconnecting...", false).await {
                            error!("Failed to update UI connection status: {}", e);
                        }
                    }

                    BackendEvent::NewFrame(processed_frame) => {
                        // Convert frame to Slint image format
                        match image_converter.convert_to_slint_image(&processed_frame).await {
                            Ok(slint_image) => {
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

                                // Update UI with new frame
                                if let Err(e) = slint_bridge.update_frame(
                                    slint_image,
                                    &processed_frame.resolution_string(),
                                    &processed_frame.format_string(),
                                    processed_frame.header.frame_id as i32,
                                    processed_frame.header.sequence_number as i32,
                                ).await {
                                    error!("Failed to update UI frame: {}", e);
                                }
                            }
                            Err(e) => {
                                error!("Failed to convert frame to Slint image: {}", e);
                            }
                        }
                    }

                    BackendEvent::StatisticsUpdate(stats) => {
                        // Update UI state with statistics
                        {
                            let mut state = ui_state.write().await;
                            state.fps = stats.current_fps as f32;
                            state.latency_ms = stats.average_latency_ms as f32;
                            state.total_frames = stats.total_frames_received as i32;
                        }

                        // Update UI statistics
                        if let Err(e) = slint_bridge.update_statistics(
                            stats.current_fps as f32,
                            stats.average_latency_ms as f32,
                            stats.total_frames_received as i32,
                        ).await {
                            error!("Failed to update UI statistics: {}", e);
                        }
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