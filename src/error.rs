// src/error.rs - Comprehensive Error Handling for MiVi Medical Frame Viewer

use std::fmt;

/// Main error type for the MiVi Medical Frame Viewer application
#[derive(Debug, thiserror::Error)]
pub enum MiViError {
    /// Backend-related errors
    #[error("Backend error: {0}")]
    Backend(#[from] crate::backend::BackendError),
    
    /// Frontend-related errors  
    #[error("Frontend error: {0}")]
    Frontend(#[from] crate::frontend::FrontendError),
    
    /// Shared memory errors
    #[error("Shared memory error: {0}")]
    SharedMemory(#[from] crate::backend::shared_memory::SharedMemoryError),
    
    /// Frame processing errors
    #[error("Frame processing error: {0}")]
    FrameProcessing(#[from] crate::backend::frame_processor::ProcessingError),
    
    /// Image conversion errors
    #[error("Image conversion error: {0}")]
    ImageConversion(#[from] crate::frontend::image_converter::ImageConversionError),
    
    /// Slint UI errors
    #[error("UI error: {0}")]
    Ui(#[from] crate::frontend::slint_bridge::SlintBridgeError),
    
    /// Configuration errors
    #[error("Configuration error: {0}")]
    Configuration(String),
    
    /// Application lifecycle errors
    #[error("Application error: {0}")]
    Application(String),
    
    /// Medical device communication errors
    #[error("Medical device error: {0}")]
    MedicalDevice(String),
    
    /// DICOM-related errors
    #[error("DICOM error: {0}")]
    Dicom(String),
    
    /// File system errors
    #[error("File system error: {0}")]
    FileSystem(#[from] std::io::Error),
    
    /// JSON parsing errors
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    
    /// Network-related errors
    #[error("Network error: {0}")]
    Network(String),
    
    /// Permission/security errors
    #[error("Permission error: {0}")]
    Permission(String),
    
    /// Resource exhaustion errors
    #[error("Resource error: {0}")]
    Resource(String),
    
    /// Threading/concurrency errors
    #[error("Concurrency error: {0}")]
    Concurrency(String),
    
    /// Validation errors
    #[error("Validation error: {0}")]
    Validation(String),
    
    /// External dependency errors
    #[error("External dependency error: {0}")]
    ExternalDependency(String),
    
    /// Unrecoverable system errors
    #[error("System error: {0}")]
    System(String),
    
    /// Generic errors with context
    #[error("Error in {context}: {source}")]
    WithContext {
        context: String,
        source: Box<MiViError>,
    },
    
    /// Multiple errors that occurred together
    #[error("Multiple errors occurred: {}", format_error_list(.0))]
    Multiple(Vec<MiViError>),
    
    /// Timeout errors
    #[error("Timeout error: {0}")]
    Timeout(String),
    
    /// Cancellation errors
    #[error("Operation cancelled: {0}")]
    Cancelled(String),
    
    /// Compatibility errors
    #[error("Compatibility error: {0}")]
    Compatibility(String),
    
    /// Unknown/unexpected errors
    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl MiViError {
    /// Create a new configuration error
    pub fn config(msg: impl Into<String>) -> Self {
        MiViError::Configuration(msg.into())
    }
    
    /// Create a new application error
    pub fn app(msg: impl Into<String>) -> Self {
        MiViError::Application(msg.into())
    }
    
    /// Create a new medical device error
    pub fn device(msg: impl Into<String>) -> Self {
        MiViError::MedicalDevice(msg.into())
    }
    
    /// Create a new DICOM error
    pub fn dicom(msg: impl Into<String>) -> Self {
        MiViError::Dicom(msg.into())
    }
    
    /// Create a new network error
    pub fn network(msg: impl Into<String>) -> Self {
        MiViError::Network(msg.into())
    }
    
    /// Create a new permission error
    pub fn permission(msg: impl Into<String>) -> Self {
        MiViError::Permission(msg.into())
    }
    
    /// Create a new resource error
    pub fn resource(msg: impl Into<String>) -> Self {
        MiViError::Resource(msg.into())
    }
    
    /// Create a new validation error
    pub fn validation(msg: impl Into<String>) -> Self {
        MiViError::Validation(msg.into())
    }
    
    /// Create a new timeout error
    pub fn timeout(msg: impl Into<String>) -> Self {
        MiViError::Timeout(msg.into())
    }
    
    /// Add context to an error
    pub fn with_context(self, context: impl Into<String>) -> Self {
        MiViError::WithContext {
            context: context.into(),
            source: Box::new(self),
        }
    }
    
    /// Check if this error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            // These errors are typically recoverable
            MiViError::SharedMemory(_) => true,
            MiViError::Network(_) => true,
            MiViError::Timeout(_) => true,
            MiViError::MedicalDevice(_) => true,
            MiViError::ImageConversion(_) => true,
            MiViError::FrameProcessing(_) => true,
            
            // These errors are typically not recoverable
            MiViError::Configuration(_) => false,
            MiViError::Permission(_) => false,
            MiViError::System(_) => false,
            MiViError::ExternalDependency(_) => false,
            MiViError::Compatibility(_) => false,
            
            // Context and multiple errors depend on their contents
            MiViError::WithContext { source, .. } => source.is_recoverable(),
            MiViError::Multiple(errors) => errors.iter().any(|e| e.is_recoverable()),
            
            // Default to not recoverable for safety
            _ => false,
        }
    }
    
    /// Get error severity level
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical errors that require immediate attention
            MiViError::System(_) |
            MiViError::Permission(_) |
            MiViError::Resource(_) => ErrorSeverity::Critical,
            
            // High severity errors that significantly impact functionality
            MiViError::Backend(_) |
            MiViError::SharedMemory(_) |
            MiViError::Configuration(_) |
            MiViError::ExternalDependency(_) => ErrorSeverity::High,
            
            // Medium severity errors that partially impact functionality
            MiViError::MedicalDevice(_) |
            MiViError::Network(_) |
            MiViError::FrameProcessing(_) |
            MiViError::ImageConversion(_) => ErrorSeverity::Medium,
            
            // Low severity errors that have minimal impact
            MiViError::Ui(_) |
            MiViError::Validation(_) |
            MiViError::Timeout(_) |
            MiViError::Cancelled(_) => ErrorSeverity::Low,
            
            // Context errors inherit from source
            MiViError::WithContext { source, .. } => source.severity(),
            
            // Multiple errors take the highest severity
            MiViError::Multiple(errors) => {
                errors.iter()
                    .map(|e| e.severity())
                    .max()
                    .unwrap_or(ErrorSeverity::Low)
            }
            
            // Default to medium severity
            _ => ErrorSeverity::Medium,
        }
    }
    
    /// Get suggested user action for this error
    pub fn suggested_action(&self) -> &'static str {
        match self {
            MiViError::SharedMemory(_) => "Check if the medical device is running and accessible",
            MiViError::MedicalDevice(_) => "Verify medical device connection and configuration",
            MiViError::Network(_) => "Check network connectivity and firewall settings",
            MiViError::Permission(_) => "Run with appropriate permissions or contact system administrator",
            MiViError::Configuration(_) => "Check configuration file and command line arguments",
            MiViError::Resource(_) => "Free up system resources (memory, disk space) and try again",
            MiViError::Timeout(_) => "Try again or increase timeout settings",
            MiViError::Compatibility(_) => "Update software or check system requirements",
            MiViError::Validation(_) => "Correct the input data and try again",
            MiViError::FileSystem(_) => "Check file permissions and disk space",
            MiViError::ExternalDependency(_) => "Install required dependencies or update software",
            MiViError::System(_) => "Contact system administrator or restart the application",
            MiViError::WithContext { source, .. } => source.suggested_action(),
            MiViError::Multiple(_) => "Address the individual errors listed above",
            _ => "Try restarting the application or contact technical support",
        }
    }
    
    /// Get error category for logging and monitoring
    pub fn category(&self) -> ErrorCategory {
        match self {
            MiViError::Backend(_) |
            MiViError::SharedMemory(_) |
            MiViError::FrameProcessing(_) => ErrorCategory::Backend,
            
            MiViError::Frontend(_) |
            MiViError::Ui(_) |
            MiViError::ImageConversion(_) => ErrorCategory::Frontend,
            
            MiViError::MedicalDevice(_) |
            MiViError::Dicom(_) => ErrorCategory::MedicalDevice,
            
            MiViError::Configuration(_) |
            MiViError::Validation(_) => ErrorCategory::Configuration,
            
            MiViError::Network(_) |
            MiViError::Timeout(_) => ErrorCategory::Network,
            
            MiViError::FileSystem(_) |
            MiViError::Permission(_) => ErrorCategory::System,
            
            MiViError::WithContext { source, .. } => source.category(),
            
            _ => ErrorCategory::Other,
        }
    }
    
    /// Convert to a user-friendly message
    pub fn user_message(&self) -> String {
        match self {
            MiViError::SharedMemory(_) => {
                "Cannot connect to medical device. Please ensure the device is powered on and properly configured.".to_string()
            }
            MiViError::MedicalDevice(_) => {
                "Medical device communication error. Please check device connection and settings.".to_string()
            }
            MiViError::Network(_) => {
                "Network connection error. Please check your network settings and try again.".to_string()
            }
            MiViError::Permission(_) => {
                "Permission denied. Please run the application with appropriate privileges.".to_string()
            }
            MiViError::Configuration(_) => {
                "Configuration error. Please check your settings and try again.".to_string()
            }
            MiViError::Resource(_) => {
                "System resources unavailable. Please free up memory or disk space and try again.".to_string()
            }
            MiViError::Timeout(_) => {
                "Operation timed out. Please try again or check your connection.".to_string()
            }
            MiViError::Compatibility(_) => {
                "Compatibility issue detected. Please update your software or check system requirements.".to_string()
            }
            MiViError::WithContext { context, source } => {
                format!("{}: {}", context, source.user_message())
            }
            MiViError::Multiple(errors) => {
                if errors.len() == 1 {
                    errors[0].user_message()
                } else {
                    format!("Multiple errors occurred. Please address the following issues: {}", 
                            errors.iter().map(|e| e.user_message()).collect::<Vec<_>>().join("; "))
                }
            }
            _ => {
                format!("An error occurred: {}", self)
            }
        }
    }
    
    /// Get error code for external systems
    pub fn error_code(&self) -> u32 {
        match self {
            MiViError::Backend(_) => 1000,
            MiViError::Frontend(_) => 2000,
            MiViError::SharedMemory(_) => 3000,
            MiViError::FrameProcessing(_) => 3100,
            MiViError::ImageConversion(_) => 3200,
            MiViError::Ui(_) => 4000,
            MiViError::Configuration(_) => 5000,
            MiViError::Application(_) => 5100,
            MiViError::MedicalDevice(_) => 6000,
            MiViError::Dicom(_) => 6100,
            MiViError::FileSystem(_) => 7000,
            MiViError::Json(_) => 7100,
            MiViError::Network(_) => 8000,
            MiViError::Permission(_) => 9000,
            MiViError::Resource(_) => 9100,
            MiViError::Concurrency(_) => 9200,
            MiViError::Validation(_) => 9300,
            MiViError::ExternalDependency(_) => 9400,
            MiViError::System(_) => 9500,
            MiViError::Timeout(_) => 9600,
            MiViError::Cancelled(_) => 9700,
            MiViError::Compatibility(_) => 9800,
            MiViError::Unknown(_) => 9999,
            MiViError::WithContext { source, .. } => source.error_code(),
            MiViError::Multiple(_) => 10000,
        }
    }
}

/// Error severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ErrorSeverity {
    /// Low severity - minimal impact on functionality
    Low,
    /// Medium severity - partial impact on functionality  
    Medium,
    /// High severity - significant impact on functionality
    High,
    /// Critical severity - application cannot continue
    Critical,
}

impl fmt::Display for ErrorSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorSeverity::Low => write!(f, "LOW"),
            ErrorSeverity::Medium => write!(f, "MEDIUM"),
            ErrorSeverity::High => write!(f, "HIGH"),
            ErrorSeverity::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// Error categories for classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    /// Backend processing errors
    Backend,
    /// Frontend/UI errors
    Frontend,
    /// Medical device communication errors
    MedicalDevice,
    /// Configuration and validation errors
    Configuration,
    /// Network and connectivity errors
    Network,
    /// System-level errors
    System,
    /// Other/unclassified errors
    Other,
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorCategory::Backend => write!(f, "BACKEND"),
            ErrorCategory::Frontend => write!(f, "FRONTEND"),
            ErrorCategory::MedicalDevice => write!(f, "MEDICAL_DEVICE"),
            ErrorCategory::Configuration => write!(f, "CONFIGURATION"),
            ErrorCategory::Network => write!(f, "NETWORK"),
            ErrorCategory::System => write!(f, "SYSTEM"),
            ErrorCategory::Other => write!(f, "OTHER"),
        }
    }
}

/// Helper function to format error lists
fn format_error_list(errors: &[MiViError]) -> String {
    if errors.is_empty() {
        return "No errors".to_string();
    }
    
    if errors.len() == 1 {
        return errors[0].to_string();
    }
    
    let mut result = String::new();
    for (i, error) in errors.iter().enumerate() {
        if i > 0 {
            result.push_str(", ");
        }
        result.push_str(&format!("({})", error));
    }
    result
}

/// Result type alias for MiVi operations
pub type MiViResult<T> = Result<T, MiViError>;

/// Extension trait for Results to add context
pub trait ResultExt<T> {
    /// Add context to an error
    fn with_context(self, context: impl Into<String>) -> MiViResult<T>;
    
    /// Add context using a closure (for lazy evaluation)
    fn with_context_lazy<F>(self, f: F) -> MiViResult<T>
    where
        F: FnOnce() -> String;
}

impl<T, E> ResultExt<T> for Result<T, E>
where
    E: Into<MiViError>,
{
    fn with_context(self, context: impl Into<String>) -> MiViResult<T> {
        self.map_err(|e| e.into().with_context(context))
    }
    
    fn with_context_lazy<F>(self, f: F) -> MiViResult<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| e.into().with_context(f()))
    }
}

/// Error reporter for structured error handling
pub struct ErrorReporter {
    enable_logging: bool,
    enable_telemetry: bool,
}

impl ErrorReporter {
    /// Create a new error reporter
    pub fn new(enable_logging: bool, enable_telemetry: bool) -> Self {
        Self {
            enable_logging,
            enable_telemetry,
        }
    }
    
    /// Report an error
    pub fn report(&self, error: &MiViError) {
        if self.enable_logging {
            self.log_error(error);
        }
        
        if self.enable_telemetry {
            self.send_telemetry(error);
        }
    }
    
    /// Log error to console/file
    fn log_error(&self, error: &MiViError) {
        use tracing::{error, warn, info};
        
        let severity = error.severity();
        let category = error.category();
        let code = error.error_code();
        
        match severity {
            ErrorSeverity::Critical => {
                error!(
                    error_code = code,
                    category = %category,
                    severity = %severity,
                    "Critical error: {} | Action: {}",
                    error,
                    error.suggested_action()
                );
            }
            ErrorSeverity::High => {
                error!(
                    error_code = code,
                    category = %category,
                    severity = %severity,
                    "High severity error: {} | Action: {}",
                    error,
                    error.suggested_action()
                );
            }
            ErrorSeverity::Medium => {
                warn!(
                    error_code = code,
                    category = %category,
                    severity = %severity,
                    "Medium severity error: {} | Action: {}",
                    error,
                    error.suggested_action()
                );
            }
            ErrorSeverity::Low => {
                info!(
                    error_code = code,
                    category = %category,
                    severity = %severity,
                    "Low severity error: {} | Action: {}",
                    error,
                    error.suggested_action()
                );
            }
        }
    }
    
    /// Send error telemetry (placeholder for external telemetry systems)
    fn send_telemetry(&self, error: &MiViError) {
        // In a real implementation, this would send error data to an external
        // telemetry system like Sentry, DataDog, etc.
        
        let _telemetry_data = ErrorTelemetryData {
            error_code: error.error_code(),
            severity: error.severity(),
            category: error.category(),
            message: error.to_string(),
            user_message: error.user_message(),
            suggested_action: error.suggested_action().to_string(),
            is_recoverable: error.is_recoverable(),
            timestamp: std::time::SystemTime::now(),
        };
        
        // Send telemetry_data to external system
        // telemetry_client.send(telemetry_data);
    }
}

/// Telemetry data structure for error reporting
#[derive(Debug)]
struct ErrorTelemetryData {
    error_code: u32,
    severity: ErrorSeverity,
    category: ErrorCategory,
    message: String,
    user_message: String,
    suggested_action: String,
    is_recoverable: bool,
    timestamp: std::time::SystemTime,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_creation() {
        let error = MiViError::config("Test configuration error");
        assert!(matches!(error, MiViError::Configuration(_)));
        
        let error = MiViError::device("Test device error");
        assert!(matches!(error, MiViError::MedicalDevice(_)));
    }
    
    #[test]
    fn test_error_with_context() {
        let base_error = MiViError::network("Connection failed");
        let contextual_error = base_error.with_context("During startup");
        
        assert!(matches!(contextual_error, MiViError::WithContext { .. }));
        assert!(contextual_error.to_string().contains("During startup"));
    }
    
    #[test]
    fn test_error_recoverability() {
        let network_error = MiViError::network("Test");
        assert!(network_error.is_recoverable());
        
        let config_error = MiViError::config("Test");
        assert!(!config_error.is_recoverable());
    }
    
    #[test]
    fn test_error_categories() {
        let backend_error = MiViError::resource("Test");
        assert_eq!(backend_error.category(), ErrorCategory::System);
        
        let device_error = MiViError::device("Test");
        assert_eq!(device_error.category(), ErrorCategory::MedicalDevice);
    }
    
    #[test]
    fn test_result_ext() {
        let result: Result<(), std::io::Error> = Err(std::io::Error::new(std::io::ErrorKind::NotFound, "File not found"));
        let contextual_result = result.with_context("Reading configuration file");
        
        assert!(contextual_result.is_err());
        assert!(contextual_result.unwrap_err().to_string().contains("Reading configuration file"));
    }
    
    #[test]
    fn test_multiple_errors() {
        let errors = vec![
            MiViError::config("Config error"),
            MiViError::network("Network error"),
        ];
        
        let multiple_error = MiViError::Multiple(errors);
        assert!(multiple_error.to_string().contains("Multiple errors occurred"));
        assert_eq!(multiple_error.severity(), ErrorSeverity::High); // Max of config (High) and network (Medium)
    }
    
    #[test]
    fn test_error_reporter() {
        let reporter = ErrorReporter::new(true, false);
        let error = MiViError::config("Test error");
        
        // This should not panic
        reporter.report(&error);
    }
}
