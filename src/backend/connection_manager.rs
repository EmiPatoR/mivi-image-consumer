// src/backend/connection_manager.rs - Medical Device Connection Management

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::backend::{
    shared_memory::SharedMemoryError,
    types::RawFrame,
    ConnectionConfig, ConnectionStatus, SharedMemoryReader,
};

/// Connection manager for medical imaging devices
pub struct ConnectionManager {
    // Shared memory reader
    reader: Arc<RwLock<Option<SharedMemoryReader>>>,

    // Connection state
    connection_status: Arc<RwLock<ConnectionStatus>>,
    current_config: Arc<RwLock<Option<ConnectionConfig>>>,

    // Reconnection management
    reconnect_attempts: Arc<RwLock<u32>>,
    last_reconnect_attempt: Arc<RwLock<Option<Instant>>>,

    // Statistics
    connection_stats: Arc<RwLock<ConnectionStatistics>>,

    // Configuration
    base_config: ConnectionConfig,
}

impl ConnectionManager {
    /// Create a new connection manager
    pub fn new(base_config: ConnectionConfig) -> Self {
        Self {
            reader: Arc::new(RwLock::new(None)),
            connection_status: Arc::new(RwLock::new(ConnectionStatus::Disconnected)),
            current_config: Arc::new(RwLock::new(None)),
            reconnect_attempts: Arc::new(RwLock::new(0)),
            last_reconnect_attempt: Arc::new(RwLock::new(None)),
            connection_stats: Arc::new(RwLock::new(ConnectionStatistics::default())),
            base_config,
        }
    }

    /// Connect to shared memory with specified configuration
    pub async fn connect(
        &self,
        shm_name: &str,
        config: ConnectionConfig,
    ) -> Result<(), ConnectionManagerError> {
        info!("🔌 Connecting to medical device: {}", shm_name);

        // Update connection status
        *self.connection_status.write().await = ConnectionStatus::Connecting;

        // Create shared memory reader
        let mut reader = SharedMemoryReader::new(shm_name, config.clone())
            .map_err(|e| ConnectionManagerError::SharedMemory(e))?;

        // Attempt connection
        match reader.connect().await {
            Ok(()) => {
                // Store successful connection
                *self.reader.write().await = Some(reader);
                *self.connection_status.write().await = ConnectionStatus::Connected;
                *self.current_config.write().await = Some(config);
                *self.reconnect_attempts.write().await = 0;

                // Update statistics
                {
                    let mut stats = self.connection_stats.write().await;
                    stats.successful_connections += 1;
                    stats.last_connected = Some(Instant::now());
                    stats.current_session_start = Some(Instant::now());
                }

                info!("✅ Successfully connected to medical device: {}", shm_name);
                Ok(())
            }
            Err(e) => {
                // Connection failed
                *self.connection_status.write().await = ConnectionStatus::Error(e.to_string());

                // Update statistics
                {
                    let mut stats = self.connection_stats.write().await;
                    stats.failed_connections += 1;
                    stats.last_error = Some(e.to_string());
                }

                error!("❌ Failed to connect to medical device {}: {}", shm_name, e);
                Err(ConnectionManagerError::SharedMemory(e))
            }
        }
    }

    /// Disconnect from shared memory
    pub async fn disconnect(&self) {
        info!("🔌 Disconnecting from medical device");

        // Disconnect reader if present
        if let Some(mut reader) = self.reader.write().await.take() {
            reader.disconnect().await;
        }

        // Update status
        *self.connection_status.write().await = ConnectionStatus::Disconnected;
        *self.current_config.write().await = None;

        // Update statistics
        {
            let mut stats = self.connection_stats.write().await;
            if let Some(session_start) = stats.current_session_start {
                stats.total_session_time += session_start.elapsed();
            }
            stats.current_session_start = None;
        }

        info!("✅ Disconnected from medical device");
    }

    /// Check if currently connected
    pub async fn is_connected(&self) -> bool {
        matches!(
            *self.connection_status.read().await,
            ConnectionStatus::Connected
        )
    }

    /// Get current connection status
    pub async fn get_status(&self) -> ConnectionStatus {
        self.connection_status.read().await.clone()
    }

    /// Get next frame from shared memory
    pub async fn get_next_frame(
        &self,
        catch_up: bool,
    ) -> Result<Option<RawFrame>, ConnectionManagerError> {
        // Check if we have an active reader
        let reader_lock = self.reader.read().await;
        let reader = reader_lock
            .as_ref()
            .ok_or(ConnectionManagerError::NotConnected)?;

        // Check connection health
        if !reader.check_connection_health() {
            drop(reader_lock); // Release the read lock

            // Mark as connection lost and attempt reconnection
            *self.connection_status.write().await = ConnectionStatus::Reconnecting;

            // Update statistics
            {
                let mut stats = self.connection_stats.write().await;
                stats.connection_lost_count += 1;
            }

            warn!("⚠️ Connection health check failed, attempting reconnection");

            // Try to reconnect
            if let Err(e) = self.attempt_reconnection().await {
                error!("🔄 Reconnection failed: {}", e);
                return Err(ConnectionManagerError::ConnectionLost);
            }

            // Try to get the frame again with the new connection
            let reader_lock = self.reader.read().await;
            let reader = reader_lock
                .as_ref()
                .ok_or(ConnectionManagerError::NotConnected)?;

            reader
                .get_next_frame(catch_up)
                .await
                .map_err(|e| ConnectionManagerError::SharedMemory(e))
        } else {
            // Connection is healthy, get frame normally
            reader.get_next_frame(catch_up).await.map_err(|e| {
                match e {
                    SharedMemoryError::ConnectionLost => {
                        // Schedule reconnection
                        let connection_status = Arc::clone(&self.connection_status);
                        tokio::spawn(async move {
                            *connection_status.write().await = ConnectionStatus::Reconnecting;
                        });
                        ConnectionManagerError::ConnectionLost
                    }
                    _ => ConnectionManagerError::SharedMemory(e),
                }
            })
        }
    }

    /// Attempt automatic reconnection
    async fn attempt_reconnection(&self) -> Result<(), ConnectionManagerError> {
        let mut attempts = self.reconnect_attempts.write().await;
        let mut last_attempt = self.last_reconnect_attempt.write().await;

        // Check if we should attempt reconnection
        if let Some(last_attempt_time) = *last_attempt {
            if last_attempt_time.elapsed() < self.base_config.reconnect_delay {
                return Err(ConnectionManagerError::ReconnectTooSoon);
            }
        }

        // Check if we've exceeded max attempts
        if *attempts >= self.base_config.max_reconnect_attempts {
            warn!("🔄 Maximum reconnection attempts exceeded: {}", *attempts);
            *self.connection_status.write().await = ConnectionStatus::Error(format!(
                "Max reconnection attempts exceeded: {}",
                *attempts
            ));
            return Err(ConnectionManagerError::MaxReconnectAttemptsExceeded);
        }

        *attempts += 1;
        *last_attempt = Some(Instant::now());

        info!("🔄 Attempting reconnection #{}", *attempts);

        // Get current configuration
        let _config = {
            let config_lock = self.current_config.read().await;
            config_lock
                .as_ref()
                .ok_or(ConnectionManagerError::NoConfiguration)?
                .clone()
        };

        // Force reconnection
        if let Some(mut reader) = self.reader.write().await.take() {
            match reader.force_reconnect().await {
                Ok(()) => {
                    // Successful reconnection
                    *self.reader.write().await = Some(reader);
                    *self.connection_status.write().await = ConnectionStatus::Connected;
                    *attempts = 0; // Reset attempts counter

                    // Update statistics
                    {
                        let mut stats = self.connection_stats.write().await;
                        stats.successful_reconnections += 1;
                    }

                    info!("✅ Successfully reconnected to medical device");
                    Ok(())
                }
                Err(e) => {
                    error!("❌ Reconnection attempt #{} failed: {}", *attempts, e);

                    // Update statistics
                    {
                        let mut stats = self.connection_stats.write().await;
                        stats.failed_reconnections += 1;
                    }

                    if *attempts >= self.base_config.max_reconnect_attempts {
                        *self.connection_status.write().await = ConnectionStatus::Error(format!(
                            "Reconnection failed after {} attempts",
                            *attempts
                        ));
                    }

                    Err(ConnectionManagerError::ReconnectionFailed(e.to_string()))
                }
            }
        } else {
            Err(ConnectionManagerError::NoActiveConnection)
        }
    }

    /// Update connection configuration
    pub async fn update_config(
        &self,
        config: ConnectionConfig,
    ) -> Result<(), ConnectionManagerError> {
        info!("⚙️ Updating connection configuration");

        // If currently connected, disconnect and reconnect with new config
        if self.is_connected().await {
            let shm_name = {
                let reader_lock = self.reader.read().await;
                if let Some(reader) = reader_lock.as_ref() {
                    reader.get_statistics().shm_name.clone()
                } else {
                    return Err(ConnectionManagerError::NotConnected);
                }
            };

            self.disconnect().await;
            self.connect(&shm_name, config).await?;
        } else {
            // Just update the configuration
            *self.current_config.write().await = Some(config);
        }

        info!("✅ Connection configuration updated");
        Ok(())
    }

    /// Get connection statistics
    pub async fn get_statistics(&self) -> ConnectionStatistics {
        let mut stats = self.connection_stats.read().await.clone();

        // Add current session time if connected
        if let Some(session_start) = stats.current_session_start {
            stats.current_session_time = session_start.elapsed();
        }

        // Add reader statistics if available
        if let Some(reader) = self.reader.read().await.as_ref() {
            let reader_stats = reader.get_statistics();
            stats.frames_processed = reader_stats.frames_processed;
            stats.error_count = reader_stats.error_count;
            stats.last_frame_elapsed = reader_stats.last_frame_elapsed;
        }

        stats
    }

    /// Force manual reconnection
    pub async fn force_reconnect(&self) -> Result<(), ConnectionManagerError> {
        info!("🔄 Forcing manual reconnection");

        // Reset attempts counter for manual reconnection
        *self.reconnect_attempts.write().await = 0;

        self.attempt_reconnection().await
    }

    /// Check if automatic reconnection is possible
    pub async fn can_reconnect(&self) -> bool {
        let attempts = *self.reconnect_attempts.read().await;
        let last_attempt = *self.last_reconnect_attempt.read().await;

        // Check attempts limit
        if attempts >= self.base_config.max_reconnect_attempts {
            return false;
        }

        // Check time delay
        if let Some(last_attempt_time) = last_attempt {
            if last_attempt_time.elapsed() < self.base_config.reconnect_delay {
                return false;
            }
        }

        // Must have configuration to reconnect
        self.current_config.read().await.is_some()
    }
}

/// Connection manager errors
#[derive(Debug, thiserror::Error)]
pub enum ConnectionManagerError {
    #[error("Not connected to any medical device")]
    NotConnected,

    #[error("Connection lost to medical device")]
    ConnectionLost,

    #[error("No active connection available")]
    NoActiveConnection,

    #[error("No configuration available for reconnection")]
    NoConfiguration,

    #[error("Reconnection attempted too soon, please wait")]
    ReconnectTooSoon,

    #[error("Maximum reconnection attempts exceeded")]
    MaxReconnectAttemptsExceeded,

    #[error("Reconnection failed: {0}")]
    ReconnectionFailed(String),

    #[error("Shared memory error: {0}")]
    SharedMemory(#[from] SharedMemoryError),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Other connection error: {0}")]
    Other(String),
}

/// Connection statistics and monitoring
#[derive(Debug, Clone, Default)]
pub struct ConnectionStatistics {
    // Connection metrics
    pub successful_connections: u64,
    pub failed_connections: u64,
    pub successful_reconnections: u64,
    pub failed_reconnections: u64,
    pub connection_lost_count: u64,

    // Timing information
    pub last_connected: Option<Instant>,
    pub current_session_start: Option<Instant>,
    pub current_session_time: Duration,
    pub total_session_time: Duration,

    // Frame processing (from reader)
    pub frames_processed: u64,
    pub error_count: u64,
    pub last_frame_elapsed: Duration,

    // Error tracking
    pub last_error: Option<String>,
}

impl ConnectionStatistics {
    /// Get connection uptime percentage
    pub fn uptime_percentage(&self) -> f64 {
        let total_time = self.total_session_time + self.current_session_time;
        if total_time.as_secs() > 0 {
            (self.total_session_time.as_secs_f64() / total_time.as_secs_f64()) * 100.0
        } else {
            0.0
        }
    }

    /// Get average session duration
    pub fn average_session_duration(&self) -> Duration {
        if self.successful_connections > 0 {
            self.total_session_time / self.successful_connections as u32
        } else {
            Duration::ZERO
        }
    }

    /// Get connection reliability score (0-100)
    pub fn reliability_score(&self) -> f64 {
        let total_attempts = self.successful_connections + self.failed_connections;
        if total_attempts > 0 {
            (self.successful_connections as f64 / total_attempts as f64) * 100.0
        } else {
            0.0
        }
    }

    /// Get reconnection success rate
    pub fn reconnection_success_rate(&self) -> f64 {
        let total_reconnection_attempts = self.successful_reconnections + self.failed_reconnections;
        if total_reconnection_attempts > 0 {
            (self.successful_reconnections as f64 / total_reconnection_attempts as f64) * 100.0
        } else {
            0.0
        }
    }

    /// Check if connection is considered stable
    pub fn is_stable(&self) -> bool {
        // Consider stable if:
        // - Connected for at least 30 seconds
        // - Less than 3 reconnections in current session
        // - No errors in last 10 seconds
        self.current_session_time >= Duration::from_secs(30)
            && self.connection_lost_count < 3
            && self.last_frame_elapsed < Duration::from_secs(10)
    }

    /// Get human-readable status summary
    pub fn status_summary(&self) -> String {
        format!(
            "Connections: {}/{} ({:.1}%), Reconnections: {}/{} ({:.1}%), Uptime: {:.1}%, Frames: {}",
            self.successful_connections,
            self.successful_connections + self.failed_connections,
            self.reliability_score(),
            self.successful_reconnections,
            self.successful_reconnections + self.failed_reconnections,
            self.reconnection_success_rate(),
            self.uptime_percentage(),
            self.frames_processed
        )
    }
}