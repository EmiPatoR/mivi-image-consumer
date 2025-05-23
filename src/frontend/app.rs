// src/frontend/app.rs - Main Application Frontend for Medical Frame Viewer

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::{mpsc, broadcast};
use tracing::{info, error, warn, debug};

use crate::backend::{
    MedicalFrameBackend, BackendCommand, BackendEvent, BackendConfig
};
use crate::frontend::{
    SlintBridge, ImageConverter, UiState, FrontendError
};

/// Internal UI command to avoid sending Slint types across threads
#[derive(Debug)]
pub enum UiCommand {
    UpdateFrame {
        frame_data: Arc<[u8]>,
        width: u32,
        height: u32,
        frame_id: u64,
        sequence_number: u64,
        resolution: String,
        format: String,
    },
    UpdateConnectionStatus(String, bool),
    UpdateStatistics(f64, f64, u64),
    ClearFrame,
    ShowNotification(String, bool),
}

/// Main application frontend that coordinates between Slint UI and backend
pub struct MedicalFrameApp {
    // Backend communication
    backend: Arc<MedicalFrameBackend>,
    command_sender: mpsc::UnboundedSender<BackendCommand>,

    // UI components
    slint_bridge: Arc<SlintBridge>,
    ui_state: Arc<tokio::sync::RwLock<UiState>>,
    image_converter: Arc<ImageConverter>,

    // Application state
    is_running: Arc<AtomicBool>,
    settings_path: std::path::PathBuf,

    // Internal UI communication
    ui_command_tx: mpsc::UnboundedSender<UiCommand>,
    ui_command_rx: Option<mpsc::UnboundedReceiver<UiCommand>>,
}

impl MedicalFrameApp {
    /// Create a new medical frame application
    pub async fn new(backend_config: BackendConfig) -> Result<Self, FrontendError> {
        info!("🏥 Initializing MiVi Medical Frame Application");

        // Create backend
        let backend = Arc::new(MedicalFrameBackend::new(backend_config.clone()));
        let command_sender = backend.get_command_sender();

        // Create UI components
        let slint_bridge = Arc::new(SlintBridge::new()
            .map_err(|e| FrontendError::Slint(e.to_string()))?);

        // Initialize UI state
        let mut ui_state = UiState::new();
        ui_state.shm_name = backend_config.shm_name.clone();
        ui_state.format = backend_config.format.clone();
        ui_state.catch_up_mode = backend_config.catch_up;
        ui_state.verbose_logging = backend_config.verbose;
        ui_state.reconnect_delay_ms = backend_config.reconnect_delay.as_millis() as u64;

        let ui_state = Arc::new(tokio::sync::RwLock::new(ui_state));
        let image_converter = Arc::new(ImageConverter::new());

        // Settings path
        let settings_path = Self::get_settings_path();

        // Create UI command channel
        let (ui_command_tx, ui_command_rx) = mpsc::unbounded_channel();

        let app = Self {
            backend,
            command_sender,
            slint_bridge,
            ui_state,
            image_converter,
            is_running: Arc::new(AtomicBool::new(false)),
            settings_path,
            ui_command_tx,
            ui_command_rx: Some(ui_command_rx),
        };

        // Load saved settings
        app.load_settings().await?;

        info!("✅ MiVi Medical Frame Application initialized");
        Ok(app)
    }

    /// Run the application
    pub async fn run(&mut self) -> Result<(), FrontendError> {
        info!("🚀 Starting MiVi Medical Frame Application");

        // Mark as running
        self.is_running.store(true, std::sync::atomic::Ordering::Relaxed);

        // Start the backend
        self.backend.start().await
            .map_err(|e| FrontendError::Backend(e.to_string()))?;

        // Setup UI event handlers
        self.setup_ui_handlers().await?;

        // Start background event processing task
        let event_task = self.start_event_processing().await;

        // Start periodic tasks
        let periodic_task = self.start_periodic_tasks().await;

        // Take the UI command receiver
        let mut ui_command_rx = self.ui_command_rx.take()
            .ok_or(FrontendError::Other("Application already started".to_string()))?;

        // Start UI command processing in main thread
        let slint_bridge_for_ui = Arc::clone(&self.slint_bridge);
        let image_converter_for_ui = Arc::clone(&self.image_converter);
        let is_running_for_ui = Arc::clone(&self.is_running);

        let ui_task = tokio::spawn(async move {
            while let Some(cmd) = ui_command_rx.recv().await {
                if !is_running_for_ui.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }

                if let Err(e) = Self::handle_ui_command(cmd, &slint_bridge_for_ui, &image_converter_for_ui).await {
                    error!("Failed to handle UI command: {}", e);
                }
            }
        });

        // Update initial UI state
        self.update_ui_from_state().await?;

        // Run the Slint UI (this will block until the UI is closed)
        info!("🎨 Starting Slint UI event loop");
        let ui_result = self.slint_bridge.run().await;

        // Mark as not running
        self.is_running.store(false, std::sync::atomic::Ordering::Relaxed);

        // Cancel background tasks
        event_task.abort();
        periodic_task.abort();
        ui_task.abort();

        // Save settings before exit
        if let Err(e) = self.save_settings().await {
            warn!("Failed to save settings: {}", e);
        }

        info!("✅ MiVi Medical Frame Application stopped");
        ui_result.map_err(|e| FrontendError::Slint(e.to_string()))
    }

    /// Handle UI commands on the main thread
    async fn handle_ui_command(
        command: UiCommand,
        slint_bridge: &Arc<SlintBridge>,
        image_converter: &Arc<ImageConverter>,
    ) -> Result<(), FrontendError> {
        match command {
            UiCommand::UpdateFrame { frame_data, width, height, frame_id, sequence_number, resolution, format } => {
                // Convert frame data to Slint image on main thread
                match image_converter.create_slint_image_from_rgba(&frame_data, width, height) {
                    Ok(slint_image) => {
                        slint_bridge.update_frame(
                            slint_image,
                            &resolution,
                            &format,
                            frame_id as i32,
                            sequence_number as i32,
                        ).await.map_err(|e| FrontendError::Ui(e.to_string()))?;
                    }
                    Err(e) => {
                        error!("Failed to convert frame to Slint image: {}", e);
                        // Show error image
                        match image_converter.create_error_image(width, height, &e.to_string()).await {
                            Ok(error_image) => {
                                slint_bridge.update_frame(
                                    error_image,
                                    &resolution,
                                    "Error",
                                    frame_id as i32,
                                    sequence_number as i32,
                                ).await.map_err(|e| FrontendError::Ui(e.to_string()))?;
                            }
                            Err(ie) => {
                                error!("Failed to create error image: {}", ie);
                            }
                        }
                    }
                }
            }
            UiCommand::UpdateConnectionStatus(status, connected) => {
                slint_bridge.update_connection_status(&status, connected).await
                    .map_err(|e| FrontendError::Ui(e.to_string()))?;
            }
            UiCommand::UpdateStatistics(fps, latency, total_frames) => {
                slint_bridge.update_statistics(fps as f32, latency as f32, total_frames as i32).await
                    .map_err(|e| FrontendError::Ui(e.to_string()))?;
            }
            UiCommand::ClearFrame => {
                slint_bridge.clear_frame().await
                    .map_err(|e| FrontendError::Ui(e.to_string()))?;
            }
            UiCommand::ShowNotification(message, is_error) => {
                slint_bridge.show_notification(&message, is_error).await
                    .map_err(|e| FrontendError::Ui(e.to_string()))?;
            }
        }
        Ok(())
    }

    /// Setup UI event handlers
    async fn setup_ui_handlers(&self) -> Result<(), FrontendError> {
        info!("⚙️ Setting up UI event handlers");

        // Reconnect button handler
        {
            let command_sender = self.command_sender.clone();
            let ui_state = Arc::clone(&self.ui_state);

            self.slint_bridge.on_reconnect_clicked(move || {
                let command_sender = command_sender.clone();
                let ui_state = Arc::clone(&ui_state);

                tokio::spawn(async move {
                    info!("🔄 Reconnect button clicked");

                    let (shm_name, config) = {
                        let mut state = ui_state.write().await;
                        state.mark_connection_attempt();
                        let config = state.get_backend_config();
                        (state.shm_name.clone(), config)
                    };

                    if let Err(e) = command_sender.send(BackendCommand::Connect { shm_name, config }) {
                        error!("Failed to send connect command: {}", e);
                    }
                });
            }).await.map_err(|e| FrontendError::Ui(e.to_string()))?;
        }

        // Catch-up mode toggle handler
        {
            let command_sender = self.command_sender.clone();
            let ui_state = Arc::clone(&self.ui_state);

            self.slint_bridge.on_toggle_catch_up(move |enabled| {
                let command_sender = command_sender.clone();
                let ui_state = Arc::clone(&ui_state);

                tokio::spawn(async move {
                    info!("⚙️ Catch-up mode toggled: {}", enabled);

                    // Update UI state
                    {
                        let mut state = ui_state.write().await;
                        state.catch_up_mode = enabled;
                    }

                    if let Err(e) = command_sender.send(BackendCommand::SetCatchUpMode(enabled)) {
                        error!("Failed to send catch-up mode command: {}", e);
                    }
                });
            }).await.map_err(|e| FrontendError::Ui(e.to_string()))?;
        }

        // Settings button handler
        {
            let ui_state = Arc::clone(&self.ui_state);

            self.slint_bridge.on_settings_clicked(move || {
                let ui_state = Arc::clone(&ui_state);

                tokio::spawn(async move {
                    info!("⚙️ Settings button clicked");

                    // For now, just log current settings
                    // In a full implementation, you'd open a settings dialog
                    let state = ui_state.read().await;
                    info!("Current settings:");
                    info!("  SHM Name: {}", state.shm_name);
                    info!("  Format: {}", state.format);
                    info!("  Catch-up: {}", state.catch_up_mode);
                    info!("  Verbose: {}", state.verbose_logging);
                    info!("  Auto-reconnect: {}", state.auto_reconnect);
                });
            }).await.map_err(|e| FrontendError::Ui(e.to_string()))?;
        }

        // About button handler
        {
            self.slint_bridge.on_about_clicked(move || {
                info!("ℹ️ About button clicked");

                // Show about information
                info!("MiVi - Medical Imaging Virtual Intelligence");
                info!("Version: 0.2.0");
                info!("Professional real-time DICOM frame viewer");
                info!("Built with Rust and Slint UI framework");
            }).await.map_err(|e| FrontendError::Ui(e.to_string()))?;
        }

        info!("✅ UI event handlers setup complete");
        Ok(())
    }

    /// Start event processing from backend
    async fn start_event_processing(&self) -> tokio::task::JoinHandle<()> {
        let mut event_receiver = self.backend.get_event_receiver();
        let ui_state = Arc::clone(&self.ui_state);
        let ui_command_tx = self.ui_command_tx.clone();
        let is_running = Arc::clone(&self.is_running);

        tokio::spawn(async move {
            info!("🔄 Starting backend event processing loop");

            while is_running.load(std::sync::atomic::Ordering::Relaxed) {
                match event_receiver.recv().await {
                    Ok(event) => {
                        if let Err(e) = Self::handle_backend_event(
                            event,
                            &ui_state,
                            &ui_command_tx,
                        ).await {
                            error!("Error handling backend event: {}", e);
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        info!("Backend event channel closed");
                        break;
                    }
                    Err(broadcast::error::RecvError::Lagged(count)) => {
                        warn!("Backend event receiver lagged by {} events", count);
                        continue;
                    }
                }
            }

            info!("🔄 Backend event processing loop stopped");
        })
    }

    /// Handle a single backend event
    async fn handle_backend_event(
        event: BackendEvent,
        ui_state: &Arc<tokio::sync::RwLock<UiState>>,
        ui_command_tx: &mpsc::UnboundedSender<UiCommand>,
    ) -> Result<(), FrontendError> {
        match event {
            BackendEvent::Connected => {
                info!("✅ Backend connected");

                // Update UI state
                {
                    let mut state = ui_state.write().await;
                    state.update_connection_status("Connected".to_string(), true);
                }

                // Send UI command
                let _ = ui_command_tx.send(UiCommand::UpdateConnectionStatus("Connected".to_string(), true));
            }

            BackendEvent::Disconnected => {
                info!("🔌 Backend disconnected");

                // Update UI state
                {
                    let mut state = ui_state.write().await;
                    state.update_connection_status("Disconnected".to_string(), false);
                }

                // Send UI commands
                let _ = ui_command_tx.send(UiCommand::UpdateConnectionStatus("Disconnected".to_string(), false));
                let _ = ui_command_tx.send(UiCommand::ClearFrame);
            }

            BackendEvent::ConnectionError(error) => {
                error!("❌ Backend connection error: {}", error);

                // Update UI state
                {
                    let mut state = ui_state.write().await;
                    state.update_connection_status(format!("Error: {}", error), false);
                }

                // Send UI command
                let _ = ui_command_tx.send(UiCommand::UpdateConnectionStatus(format!("Error: {}", error), false));
            }

            BackendEvent::ConnectionLost => {
                warn!("⚠️ Backend connection lost");

                // Update UI state
                {
                    let mut state = ui_state.write().await;
                    state.update_connection_status("Connection Lost - Attempting reconnection...".to_string(), false);
                }

                // Send UI command
                let _ = ui_command_tx.send(UiCommand::UpdateConnectionStatus("Connection Lost - Attempting reconnection...".to_string(), false));
            }

            BackendEvent::NewFrame(processed_frame) => {
                // Update UI state
                {
                    let mut state = ui_state.write().await;
                    state.update_frame_info(
                        processed_frame.header.frame_id,
                        processed_frame.header.sequence_number,
                        processed_frame.resolution_string(),
                        processed_frame.format_string(),
                    );
                }

                // Send UI command with raw frame data (avoid sending Slint Image across threads)
                let _ = ui_command_tx.send(UiCommand::UpdateFrame {
                    frame_data: processed_frame.rgb_data.clone(),
                    width: processed_frame.header.width,
                    height: processed_frame.header.height,
                    frame_id: processed_frame.header.frame_id,
                    sequence_number: processed_frame.header.sequence_number,
                    resolution: processed_frame.resolution_string(),
                    format: processed_frame.format_string(),
                });

                debug!("📺 Frame processed: {} {}x{}", 
                       processed_frame.header.frame_id,
                       processed_frame.header.width,
                       processed_frame.header.height);
            }

            BackendEvent::StatisticsUpdate(stats) => {
                // Update UI state
                {
                    let mut state = ui_state.write().await;
                    state.update_performance(
                        stats.current_fps,
                        stats.average_latency_ms,
                        stats.total_frames_received,
                        stats.frames_dropped,
                    );
                }

                // Send UI command
                let _ = ui_command_tx.send(UiCommand::UpdateStatistics(
                    stats.current_fps,
                    stats.average_latency_ms,
                    stats.total_frames_received,
                ));

                if stats.current_fps > 0.0 {
                    debug!("📊 Stats updated: {:.1} FPS, {:.1}ms latency", 
                           stats.current_fps, stats.average_latency_ms);
                }
            }

            BackendEvent::SettingsChanged => {
                info!("⚙️ Backend settings changed");
                // Handle settings changes if needed
            }
        }

        Ok(())
    }

    /// Start periodic tasks
    async fn start_periodic_tasks(&self) -> tokio::task::JoinHandle<()> {
        let ui_state = Arc::clone(&self.ui_state);
        let is_running = Arc::clone(&self.is_running);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));

            while is_running.load(std::sync::atomic::Ordering::Relaxed) {
                interval.tick().await;

                // Perform periodic tasks
                {
                    let state = ui_state.read().await;

                    // Log session statistics periodically
                    if state.session_stats.frames_received % 300 == 0 && state.session_stats.frames_received > 0 {
                        info!("📊 Session stats: {} frames, {:.1} fps avg, {:.1}ms latency avg, {:.1}% uptime",
                              state.session_stats.frames_received,
                              state.session_fps(),
                              state.session_stats.average_latency,
                              state.connection_uptime());
                    }
                }
            }
        })
    }

    /// Update UI from current state
    async fn update_ui_from_state(&self) -> Result<(), FrontendError> {
        let state = self.ui_state.read().await;

        // Update connection status
        self.slint_bridge.update_connection_status(&state.connection_status, state.is_connected).await
            .map_err(|e| FrontendError::Ui(e.to_string()))?;

        // Update configuration
        self.slint_bridge.update_config(&state.shm_name, &state.format).await
            .map_err(|e| FrontendError::Ui(e.to_string()))?;

        // Update catch-up mode
        self.slint_bridge.set_catch_up_mode(state.catch_up_mode).await
            .map_err(|e| FrontendError::Ui(e.to_string()))?;

        // Update statistics
        self.slint_bridge.update_statistics(state.fps, state.latency_ms, state.total_frames).await
            .map_err(|e| FrontendError::Ui(e.to_string()))?;

        Ok(())
    }

    /// Load settings from file
    async fn load_settings(&self) -> Result<(), FrontendError> {
        if !self.settings_path.exists() {
            info!("📁 No settings file found, using defaults");
            return Ok(());
        }

        match tokio::fs::read_to_string(&self.settings_path).await {
            Ok(content) => {
                let mut state = self.ui_state.write().await;
                if let Err(e) = state.from_json(&content) {
                    warn!("Failed to parse settings file: {}", e);
                } else {
                    info!("📁 Settings loaded from {:?}", self.settings_path);
                }
            }
            Err(e) => {
                warn!("Failed to read settings file: {}", e);
            }
        }

        Ok(())
    }

    /// Save settings to file
    async fn save_settings(&self) -> Result<(), FrontendError> {
        let state = self.ui_state.read().await;

        match state.to_json() {
            Ok(json) => {
                // Create settings directory if it doesn't exist
                if let Some(parent) = self.settings_path.parent() {
                    if let Err(e) = tokio::fs::create_dir_all(parent).await {
                        return Err(FrontendError::Other(format!("Failed to create settings directory: {}", e)));
                    }
                }

                if let Err(e) = tokio::fs::write(&self.settings_path, json).await {
                    return Err(FrontendError::Other(format!("Failed to write settings file: {}", e)));
                }

                info!("📁 Settings saved to {:?}", self.settings_path);
            }
            Err(e) => {
                return Err(FrontendError::Other(format!("Failed to serialize settings: {}", e)));
            }
        }

        Ok(())
    }

    /// Get settings file path
    fn get_settings_path() -> std::path::PathBuf {
        if let Some(config_dir) = dirs::config_dir() {
            config_dir.join("mivi").join("settings.json")
        } else {
            std::path::PathBuf::from("mivi_settings.json")
        }
    }

    /// Send command to backend
    pub async fn send_command(&self, command: BackendCommand) -> Result<(), FrontendError> {
        self.command_sender.send(command)
            .map_err(|e| FrontendError::Communication(e.to_string()))
    }

    /// Get current UI state
    pub async fn get_ui_state(&self) -> UiState {
        self.ui_state.read().await.clone()
    }

    /// Check if application is running
    pub fn is_running(&self) -> bool {
        self.is_running.load(std::sync::atomic::Ordering::Relaxed)
    }
}