// ui/tools/mod.rs - Tool implementations module

use eframe::egui::*;
use std::time::Instant;

// Tool enum - used to track the currently selected tool
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tool {
    View,
    Zoom,
    Pan,
    ROI,
    Measure,
    Annotate,
}

// Data for measurements
pub struct Measurement {
    pub start: Pos2,
    pub end: Pos2,
    pub label: String,
    pub creation_time: Instant,
    pub animated_progress: f32,
}

// Data for annotations
pub struct Annotation {
    pub position: Pos2,
    pub text: String,
    pub creation_time: Instant,
    pub animated_progress: f32,
}

// Import tool implementations
pub mod measure;
pub mod roi;
pub mod annotate;
pub mod zoom_pan;

// Re-export tool functions for use elsewhere
pub use measure::handle_measure_tool;
pub use roi::handle_roi_tool;
pub use annotate::handle_annotate_tool;
pub use zoom_pan::{handle_zoom_tool, handle_pan_tool};