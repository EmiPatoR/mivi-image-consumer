// src/backend/mod.rs - Backend Module for Medical Frame Streaming

pub mod shared_memory;
pub mod frame_processor;
pub mod connection_manager;
pub mod types;

pub use shared_memory::SharedMemoryReader;
pub use frame_processor::FrameProcessor;
pub use connection_manager::ConnectionManager;
pub use types::*;

use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{info, warn, error, debug};

/// Backend service that manages all frame streaming operations
pub struct MedicalFrameBackend {
    connection_manager: Arc<ConnectionManager>,
    frame_processor: Arc<FrameProcessor>,
    
    // Communication channels
    command_tx: mpsc::UnboundedSender<BackendCommand>,
    command_rx: Arc<RwLock<Option<mpsc::UnboundedReceiver<BackendCommand>>>>,
    
    // Event broadcasting
    event_tx: broadcast::Sender<BackendEvent>,
    
    // State management
    current_state: Arc<RwLock<BackendState>>,
}

impl MedicalFrameBackend {
    /// Create a new backend service
    pub fn new(config: BackendConfig) -> Self {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (event_tx, _) = broadcast::channel(1000);
        
        let connection_manager = Arc::new(ConnectionManager::new(config.clone()));
        let frame_processor = Arc::new(FrameProcessor::new());
        
        let current_state = Arc::new(RwLock::new(BackendState::default()));
        
        Self {
            connection_manager,
            frame_processor,
            command_tx,
            command_rx: Arc::new(RwLock::new(Some(command_rx))),
            event_tx,
            current_state,
        }
    }
    
    /// Get a command sender for frontend communication
    pub fn get_command_sender(&self) -> mpsc::UnboundedSender<BackendCommand> {
        self.command_tx.clone()
    }
    
    /// Get an event receiver for frontend communication
    pub fn get_event_receiver(&self) -> broadcast::Receiver<BackendEvent> {
        self.event_tx.subscribe()
    }
    
    /// Get current backend state
    pub async fn get_state(&self) -> BackendState {
        self.current_state.read().await.clone()
    }
    
    /// Start the backend service
    pub async fn start(&self) -> Result<(), BackendError> {
        info!("🚀 Starting MiVi Medical Frame Backend");
        
        // Take the command receiver
        let mut command_rx = {
            let mut rx_guard = self.command_rx.write().await;
            rx_guard.take().ok_or(BackendError::AlreadyStarted)?
        };
        
        // Clone necessary components for the async task
        let connection_manager = Arc::clone(&self.connection_manager);
        let frame_processor = Arc::clone(&self.frame_processor);
        let event_tx = self.event_tx.clone();
        let current_state = Arc::clone(&self.current_state);
        
        // Start the main backend loop
        tokio::spawn(async move {
            let mut frame_timer = tokio::time::interval(std::time::Duration::from_millis(16)); // ~60 FPS
            let mut stats_timer = tokio::time::interval(std::time::Duration::from_secs(1));
            
            loop {
                tokio::select! {
                    // Handle commands from frontend
                    Some(command) = command_rx.recv() => {
                        if let Err(e) = Self::handle_command(
                            command,
                            &connection_manager,
                            &frame_processor,
                            &event_tx,
                            &current_state,
                        ).await {
                            error!("Command handling error: {}", e);
                        }
                    }
                    
                    // Process frames at regular intervals
                    _ = frame_timer.tick() => {
                        if let Err(e) = Self::process_frame_cycle(
                            &connection_manager,
                            &frame_processor,
                            &event_tx,
                            &current_state,
                        ).await {
                            debug!("Frame processing: {}", e);
                        }
                    }
                    
                    // Update statistics
                    _ = stats_timer.tick() => {
                        Self::update_statistics(&event_tx, &current_state).await;
                    }
                }
            }
        });
        
        info!("✅ MiVi Medical Frame Backend started successfully");
        Ok(())
    }
    
    /// Handle commands from frontend
    async fn handle_command(
        command: BackendCommand,
        connection_manager: &Arc<ConnectionManager>,
        _frame_processor: &Arc<FrameProcessor>,
        event_tx: &broadcast::Sender<BackendEvent>,
        current_state: &Arc<RwLock<BackendState>>,
    ) -> Result<(), BackendError> {
        match command {
            BackendCommand::Connect { shm_name, config } => {
                info!("🔌 Connecting to shared memory: {}", shm_name);
                
                match connection_manager.connect(&shm_name, config).await {
                    Ok(_) => {
                        let mut state = current_state.write().await;
                        state.connection_status = ConnectionStatus::Connected;
                        state.shm_name = shm_name;
                        
                        let _ = event_tx.send(BackendEvent::Connected);
                        info!("✅ Connected to shared memory");
                    }
                    Err(e) => {
                        let mut state = current_state.write().await;
                        state.connection_status = ConnectionStatus::Error(e.to_string());
                        
                        let _ = event_tx.send(BackendEvent::ConnectionError(e.to_string()));
                        warn!("❌ Connection failed: {}", e);
                    }
                }
            }
            
            BackendCommand::Disconnect => {
                info!("🔌 Disconnecting from shared memory");
                
                connection_manager.disconnect().await;
                
                let mut state = current_state.write().await;
                state.connection_status = ConnectionStatus::Disconnected;
                state.current_frame = None;
                
                let _ = event_tx.send(BackendEvent::Disconnected);
                info!("✅ Disconnected from shared memory");
            }
            
            BackendCommand::SetCatchUpMode(enabled) => {
                info!("⚙️ Setting catch-up mode: {}", enabled);
                
                let mut state = current_state.write().await;
                state.catch_up_mode = enabled;
                
                let _ = event_tx.send(BackendEvent::SettingsChanged);
            }
            
            BackendCommand::UpdateConfig(config) => {
                info!("⚙️ Updating configuration");
                
                connection_manager.update_config(config).await?;
                let _ = event_tx.send(BackendEvent::SettingsChanged);
            }
        }
        
        Ok(())
    }
    
    /// Process a single frame cycle
    async fn process_frame_cycle(
        connection_manager: &Arc<ConnectionManager>,
        frame_processor: &Arc<FrameProcessor>,
        event_tx: &broadcast::Sender<BackendEvent>,
        current_state: &Arc<RwLock<BackendState>>,
    ) -> Result<(), BackendError> {
        // Check if we're connected
        if !connection_manager.is_connected().await {
            return Err(BackendError::NotConnected);
        }
        
        // Get the current catch-up mode
        let catch_up_mode = {
            let state = current_state.read().await;
            state.catch_up_mode
        };
        
        // Try to get a new frame
        match connection_manager.get_next_frame(catch_up_mode).await {
            Ok(Some(raw_frame)) => {
                // Process the frame (zero-copy)
                let processed_frame = frame_processor.process_frame(raw_frame).await?;
                
                // Update state
                {
                    let mut state = current_state.write().await;
                    state.current_frame = Some(processed_frame.clone());
                    state.frame_stats.update_frame_received();
                }
                
                // Notify frontend (zero-copy)
                let _ = event_tx.send(BackendEvent::NewFrame(processed_frame));
            }
            Ok(None) => {
                // No new frame available
            }
            Err(e) => {
                warn!("Frame processing error: {}", e);
                
                // Check if we should attempt reconnection
                if matches!(e, BackendError::ConnectionLost) {
                    let mut state = current_state.write().await;
                    state.connection_status = ConnectionStatus::Reconnecting;
                    
                    let _ = event_tx.send(BackendEvent::ConnectionLost);
                }
                
                return Err(e);
            }
        }
        
        Ok(())
    }
    
    /// Update statistics and send to frontend
    async fn update_statistics(
        event_tx: &broadcast::Sender<BackendEvent>,
        current_state: &Arc<RwLock<BackendState>>,
    ) {
        let stats = {
            let mut state = current_state.write().await;
            state.frame_stats.calculate_fps();
            state.frame_stats.clone()
        };
        
        let _ = event_tx.send(BackendEvent::StatisticsUpdate(stats));
    }
}

/// Backend configuration
#[derive(Debug, Clone)]
pub struct BackendConfig {
    pub shm_name: String,
    pub format: String,
    pub width: usize,
    pub height: usize,
    pub catch_up: bool,
    pub verbose: bool,
    pub reconnect_delay: std::time::Duration,
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            shm_name: "ultrasound_frames".to_string(),
            format: "yuv".to_string(),
            width: 1024,
            height: 768,
            catch_up: false,
            verbose: false,
            reconnect_delay: std::time::Duration::from_secs(1),
        }
    }
}

/// Backend state
#[derive(Debug, Clone)]
pub struct BackendState {
    pub connection_status: ConnectionStatus,
    pub shm_name: String,
    pub current_frame: Option<ProcessedFrame>,
    pub frame_stats: FrameStatistics,
    pub catch_up_mode: bool,
}

impl Default for BackendState {
    fn default() -> Self {
        Self {
            connection_status: ConnectionStatus::Disconnected,
            shm_name: String::new(),
            current_frame: None,
            frame_stats: FrameStatistics::default(),
            catch_up_mode: false,
        }
    }
}

/// Commands that can be sent to the backend
#[derive(Debug)]
pub enum BackendCommand {
    Connect { shm_name: String, config: BackendConfig },
    Disconnect,
    SetCatchUpMode(bool),
    UpdateConfig(BackendConfig),
}

/// Events emitted by the backend
#[derive(Debug, Clone)]
pub enum BackendEvent {
    Connected,
    Disconnected,
    ConnectionError(String),
    ConnectionLost,
    NewFrame(ProcessedFrame),
    StatisticsUpdate(FrameStatistics),
    SettingsChanged,
}

/// Connection status
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Error(String),
}

impl ConnectionStatus {
    pub fn is_connected(&self) -> bool {
        matches!(self, ConnectionStatus::Connected)
    }
    
    pub fn to_string(&self) -> String {
        match self {
            ConnectionStatus::Disconnected => "Disconnected".to_string(),
            ConnectionStatus::Connecting => "Connecting...".to_string(),
            ConnectionStatus::Connected => "Connected".to_string(),
            ConnectionStatus::Reconnecting => "Reconnecting...".to_string(),
            ConnectionStatus::Error(e) => format!("Error: {}", e),
        }
    }
}

/// Backend errors
#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error("Backend already started")]
    AlreadyStarted,
    
    #[error("Not connected to shared memory")]
    NotConnected,
    
    #[error("Connection lost")]
    ConnectionLost,
    
    #[error("Shared memory error: {0}")]
    SharedMemory(#[from] shared_memory::SharedMemoryError),
    
    #[error("Frame processing error: {0}")]
    FrameProcessing(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Other error: {0}")]
    Other(String),
}
