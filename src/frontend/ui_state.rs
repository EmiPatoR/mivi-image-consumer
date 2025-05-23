// src/frontend/ui_state.rs - UI State Management for Medical Frame Viewer

use std::time::Instant;
use serde::{Deserialize, Serialize};

use crate::backend::{BackendConfig, types::ConnectionConfig};

/// UI state for the medical frame viewer application
#[derive(Debug, Clone)]
pub struct UiState {
    // Connection state
    pub is_connected: bool,
    pub connection_status: String,
    pub shm_name: String,
    pub last_connection_attempt: Option<Instant>,
    
    // Frame display state
    pub has_frame: bool,
    pub current_frame_id: u64,
    pub frame_id: i32,
    pub sequence_number: i32,
    pub resolution: String,
    pub frame_format: String,
    pub last_frame_time: Instant,
    
    // Performance metrics
    pub fps: f32,
    pub latency_ms: f32,
    pub total_frames: i32,
    pub dropped_frames: i32,
    
    // Configuration
    pub catch_up_mode: bool,
    pub format: String,
    pub verbose_logging: bool,
    pub reconnect_delay_ms: u64,
    
    // UI preferences
    pub window_title: String,
    pub show_debug_info: bool,
    pub auto_reconnect: bool,
    pub notification_enabled: bool,
    
    // Medical context
    pub device_info: Option<DeviceInfo>,
    pub patient_info: Option<PatientInfo>,
    pub study_info: Option<StudyInfo>,
    
    // Statistics
    pub session_stats: SessionStatistics,
}

impl UiState {
    /// Create a new UI state with default values
    pub fn new() -> Self {
        Self {
            is_connected: false,
            connection_status: "Disconnected - Waiting for medical device".to_string(),
            shm_name: "ultrasound_frames".to_string(),
            last_connection_attempt: None,
            
            has_frame: false,
            current_frame_id: 0,
            frame_id: 0,
            sequence_number: 0,
            resolution: "0x0".to_string(),
            frame_format: "Unknown".to_string(),
            last_frame_time: Instant::now(),
            
            fps: 0.0,
            latency_ms: 0.0,
            total_frames: 0,
            dropped_frames: 0,
            
            catch_up_mode: false,
            format: "YUV".to_string(),
            verbose_logging: false,
            reconnect_delay_ms: 1000,
            
            window_title: "MiVi - Medical Imaging Virtual Intelligence".to_string(),
            show_debug_info: false,
            auto_reconnect: true,
            notification_enabled: true,
            
            device_info: None,
            patient_info: None,
            study_info: None,
            
            session_stats: SessionStatistics::new(),
        }
    }
    
    /// Update connection status
    pub fn update_connection_status(&mut self, status: String, connected: bool) {
        self.connection_status = status;
        self.is_connected = connected;
        
        if !connected {
            self.has_frame = false;
            self.current_frame_id = 0;
            self.frame_id = 0;
            self.sequence_number = 0;
        }
        
        // Update statistics
        if connected {
            self.session_stats.successful_connections += 1;
            self.session_stats.last_connected = Some(Instant::now());
        } else {
            self.session_stats.disconnections += 1;
        }
    }
    
    /// Update frame information
    pub fn update_frame_info(&mut self, frame_id: u64, sequence: u64, resolution: String, format: String) {
        self.has_frame = true;
        self.current_frame_id = frame_id;
        self.frame_id = frame_id as i32;
        self.sequence_number = sequence as i32;
        self.resolution = resolution;
        self.frame_format = format;
        self.last_frame_time = Instant::now();
        
        // Update statistics
        self.session_stats.frames_received += 1;
        self.session_stats.last_frame_time = Some(Instant::now());
    }
    
    /// Update performance metrics
    pub fn update_performance(&mut self, fps: f64, latency: f64, total: u64, dropped: u64) {
        self.fps = fps as f32;
        self.latency_ms = latency as f32;
        self.total_frames = total as i32;
        self.dropped_frames = dropped as i32;
        
        // Update statistics
        self.session_stats.update_performance(fps, latency);
    }
    
    /// Get backend configuration from UI state
    pub fn get_backend_config(&self) -> BackendConfig {
        BackendConfig {
            shm_name: self.shm_name.clone(),
            format: self.format.clone(),
            width: 1024, // Default width
            height: 768, // Default height
            catch_up: self.catch_up_mode,
            verbose: self.verbose_logging,
            reconnect_delay: std::time::Duration::from_millis(self.reconnect_delay_ms),
        }
    }
    
    /// Get connection configuration
    pub fn get_connection_config(&self) -> ConnectionConfig {
        ConnectionConfig {
            reconnect_delay: std::time::Duration::from_millis(self.reconnect_delay_ms),
            max_reconnect_attempts: if self.auto_reconnect { 10 } else { 1 },
            frame_timeout: std::time::Duration::from_secs(5),
            buffer_size: 1024 * 1024 * 50, // 50MB
            verbose_logging: self.verbose_logging,
        }
    }
    
    /// Check if reconnection should be attempted
    pub fn should_attempt_reconnection(&self) -> bool {
        if !self.auto_reconnect || self.is_connected {
            return false;
        }
        
        if let Some(last_attempt) = self.last_connection_attempt {
            last_attempt.elapsed().as_millis() >= self.reconnect_delay_ms as u128
        } else {
            true
        }
    }
    
    /// Mark connection attempt
    pub fn mark_connection_attempt(&mut self) {
        self.last_connection_attempt = Some(Instant::now());
        self.session_stats.connection_attempts += 1;
    }
    
    /// Get session duration
    pub fn session_duration(&self) -> std::time::Duration {
        self.session_stats.session_start.elapsed()
    }
    
    /// Get frames per second over session
    pub fn session_fps(&self) -> f64 {
        let duration = self.session_duration().as_secs_f64();
        if duration > 0.0 {
            self.session_stats.frames_received as f64 / duration
        } else {
            0.0
        }
    }
    
    /// Get connection uptime percentage
    pub fn connection_uptime(&self) -> f64 {
        let total_time = self.session_duration().as_secs_f64();
        if total_time > 0.0 {
            (self.session_stats.connected_time.as_secs_f64() / total_time) * 100.0
        } else {
            0.0
        }
    }
    
    /// Check if connection is stable
    pub fn is_connection_stable(&self) -> bool {
        self.is_connected && 
        self.last_frame_time.elapsed().as_secs() < 5 &&
        self.fps > 1.0 &&
        self.latency_ms < 100.0
    }
    
    /// Get status summary for display
    pub fn get_status_summary(&self) -> String {
        if self.is_connected {
            if self.has_frame {
                format!("Connected - {} @ {:.1} FPS, {:.1}ms latency", 
                        self.resolution, self.fps, self.latency_ms)
            } else {
                "Connected - Waiting for frames".to_string()
            }
        } else {
            self.connection_status.clone()
        }
    }
    
    /// Export state to JSON for saving preferences
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        let serializable_state = SerializableUiState {
            shm_name: self.shm_name.clone(),
            catch_up_mode: self.catch_up_mode,
            format: self.format.clone(),
            verbose_logging: self.verbose_logging,
            reconnect_delay_ms: self.reconnect_delay_ms,
            show_debug_info: self.show_debug_info,
            auto_reconnect: self.auto_reconnect,
            notification_enabled: self.notification_enabled,
        };
        
        serde_json::to_string_pretty(&serializable_state)
    }
    
    /// Load state from JSON
    pub fn from_json(&mut self, json: &str) -> Result<(), serde_json::Error> {
        let serializable_state: SerializableUiState = serde_json::from_str(json)?;
        
        self.shm_name = serializable_state.shm_name;
        self.catch_up_mode = serializable_state.catch_up_mode;
        self.format = serializable_state.format;
        self.verbose_logging = serializable_state.verbose_logging;
        self.reconnect_delay_ms = serializable_state.reconnect_delay_ms;
        self.show_debug_info = serializable_state.show_debug_info;
        self.auto_reconnect = serializable_state.auto_reconnect;
        self.notification_enabled = serializable_state.notification_enabled;
        
        Ok(())
    }
}

impl Default for UiState {
    fn default() -> Self {
        Self::new()
    }
}

/// Device information for medical context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub manufacturer: String,
    pub model: String,
    pub serial_number: String,
    pub software_version: String,
    pub device_type: String,
    pub calibration_date: Option<String>,
}

/// Patient information for medical context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatientInfo {
    pub patient_id: String,
    pub patient_name: String,
    pub birth_date: String,
    pub sex: String,
    pub age: Option<u32>,
}

/// Study information for medical context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudyInfo {
    pub study_id: String,
    pub study_description: String,
    pub modality: String,
    pub body_part: String,
    pub study_date: String,
    pub referring_physician: Option<String>,
    pub performing_physician: Option<String>,
}

/// Session statistics for monitoring
#[derive(Debug, Clone)]
pub struct SessionStatistics {
    pub session_start: Instant,
    pub connection_attempts: u64,
    pub successful_connections: u64,
    pub disconnections: u64,
    pub frames_received: u64,
    pub last_connected: Option<Instant>,
    pub last_frame_time: Option<Instant>,
    pub connected_time: std::time::Duration,
    pub peak_fps: f64,
    pub average_latency: f64,
    pub latency_samples: Vec<f64>,
}

impl SessionStatistics {
    /// Create new session statistics
    pub fn new() -> Self {
        Self {
            session_start: Instant::now(),
            connection_attempts: 0,
            successful_connections: 0,
            disconnections: 0,
            frames_received: 0,
            last_connected: None,
            last_frame_time: None,
            connected_time: std::time::Duration::ZERO,
            peak_fps: 0.0,
            average_latency: 0.0,
            latency_samples: Vec::new(),
        }
    }
    
    /// Update performance statistics
    pub fn update_performance(&mut self, fps: f64, latency: f64) {
        if fps > self.peak_fps {
            self.peak_fps = fps;
        }
        
        // Update latency samples (keep last 100)
        self.latency_samples.push(latency);
        if self.latency_samples.len() > 100 {
            self.latency_samples.remove(0);
        }
        
        // Calculate average latency
        if !self.latency_samples.is_empty() {
            self.average_latency = self.latency_samples.iter().sum::<f64>() / self.latency_samples.len() as f64;
        }
    }
    
    /// Get connection success rate
    pub fn connection_success_rate(&self) -> f64 {
        if self.connection_attempts > 0 {
            (self.successful_connections as f64 / self.connection_attempts as f64) * 100.0
        } else {
            0.0
        }
    }
    
    /// Get average frames per connection
    pub fn frames_per_connection(&self) -> f64 {
        if self.successful_connections > 0 {
            self.frames_received as f64 / self.successful_connections as f64
        } else {
            0.0
        }
    }
}

impl Default for SessionStatistics {
    fn default() -> Self {
        Self::new()
    }
}

/// Serializable subset of UI state for saving preferences
#[derive(Debug, Serialize, Deserialize)]
struct SerializableUiState {
    pub shm_name: String,
    pub catch_up_mode: bool,
    pub format: String,
    pub verbose_logging: bool,
    pub reconnect_delay_ms: u64,
    pub show_debug_info: bool,
    pub auto_reconnect: bool,
    pub notification_enabled: bool,
}
