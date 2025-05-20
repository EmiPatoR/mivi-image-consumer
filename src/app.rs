// app.rs - Main application state and update logic

use eframe::egui;
use egui::*;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use crate::{ui, Args};
use crate::shared_memory::SharedMemoryReader;
use crate::ui::animations::AnimationState;
use crate::ui::theme::{Theme, UiColors};
use crate::ui::tools::{Tool, Measurement, Annotation};

// Re-exports from UI modules
pub use crate::ui::theme::PatientInfo;

// Structure to hold the application state
pub struct EchoViewer {
    // Core application state
    pub shm_reader: Arc<Mutex<SharedMemoryReader>>,
    pub image_texture_id: Option<egui::TextureId>,
    pub frame_data: Vec<Color32>,
    pub frame_width: usize,
    pub frame_height: usize,
    pub connection_status: String,
    pub fps: f64,
    pub latency_ms: f64,
    pub format: String,
    pub total_frames: u64,
    pub last_frame_time: Instant,
    pub frames_received: u64,
    pub last_fps_update: Instant,
    pub catch_up: bool,
    pub last_connection_attempt: Instant,
    pub reconnect_delay: Duration,
    pub frame_header: Option<crate::shared_memory::FrameHeader>,
    pub verbose: bool,
    pub texture_allocation_size: (usize, usize),
    pub gpu_buffer: Vec<u8>,
    pub process_time_us: u64,
    pub texture_time_us: u64,

    // UI state
    pub show_info_panel: bool,
    pub show_tools_panel: bool,
    pub brightness: f32,
    pub contrast: f32,
    pub zoom_level: f32,
    pub region_of_interest: Option<Rect>,
    pub roi_active: bool,
    pub roi_start: Option<Pos2>,
    pub roi_end: Option<Pos2>,
    pub selected_tool: Tool,
    pub measurements: Vec<Measurement>,
    pub patient_info: PatientInfo,
    pub theme: Theme,
    pub colors: UiColors,
    pub show_grid: bool,
    pub show_rulers: bool,
    pub annotation_text: String,
    pub annotations: Vec<Annotation>,
    pub animation: AnimationState,
    pub show_hud: bool,
    pub start_time: Instant,
    pub elapsed_time: f32,
    pub drag_offset: Vec2,
    pub panel_alpha: f32,
    pub show_logo: bool,
    pub show_patient_details: bool,
    pub hovered_button: Option<usize>,
    pub animation_settings: Option<ui::animations::AnimationSettings>,
    pub is_capturing: Option<bool>,
}

impl EchoViewer {
    pub fn new(args: Args) -> Self {
        // Try to connect to shared memory
        let shm_reader = match SharedMemoryReader::new(&args.shm_name, args.verbose) {
            Ok(reader) => {
                Arc::new(Mutex::new(reader))
            },
            Err(e) => {
                println!("Failed to connect to shared memory: {}", e);
                Arc::new(Mutex::new(SharedMemoryReader {
                    mmap: None,
                    shm_name: args.shm_name.clone(),
                    control_block_size: std::mem::size_of::<crate::shared_memory::ControlBlock>(),
                    metadata_area_size: 4096,
                    data_offset: 0,
                    max_frames: 7,
                    frame_slot_size: 0,
                    last_processed_index: 0,
                    verbose: args.verbose,
                    connected: false,
                    last_connection_attempt: Instant::now(),
                    last_frame_time: Instant::now(),
                    no_frames_timeout: Duration::from_secs(2),
                }))
            }
        };

        Self {
            // Initialize shared memory reader and frame processing
            shm_reader,
            image_texture_id: None,
            frame_data: vec![Color32::BLACK; args.width * args.height],
            frame_width: 0,
            frame_height: 0,
            connection_status: "Disconnected - Waiting for producer".to_string(),
            fps: 0.0,
            latency_ms: 0.0,
            format: "Unknown".to_string(),
            total_frames: 0,
            last_frame_time: Instant::now(),
            frames_received: 0,
            last_fps_update: Instant::now(),
            catch_up: args.catch_up,
            last_connection_attempt: Instant::now() - Duration::from_secs(10), // Try immediately
            reconnect_delay: Duration::from_millis(args.reconnect_delay),
            frame_header: None,
            verbose: args.verbose,
            texture_allocation_size: (0, 0),
            gpu_buffer: Vec::new(),
            process_time_us: 0,
            texture_time_us: 0,

            // Initialize UI state
            show_info_panel: true,
            show_tools_panel: true,
            brightness: 0.0,
            contrast: 0.0,
            zoom_level: 1.0,
            region_of_interest: None,
            roi_active: false,
            roi_start: None,
            roi_end: None,
            selected_tool: Tool::View,
            measurements: Vec::new(),
            patient_info: PatientInfo::default(),
            theme: Theme::MedicalBlue,
            colors: UiColors::default(),
            show_grid: false,
            show_rulers: true,
            annotation_text: String::new(),
            annotations: Vec::new(),
            animation: AnimationState::default(),
            show_hud: true,
            start_time: Instant::now(),
            elapsed_time: 0.0,
            drag_offset: Vec2::ZERO,
            panel_alpha: 0.0,
            show_logo: true,
            show_patient_details: true,
            hovered_button: None,
            animation_settings: Some(ui::animations::AnimationSettings::default()),
            is_capturing: Some(false),
        }
    }

    pub fn try_connect(&mut self) {
        let mut reader = self.shm_reader.lock().unwrap();
        self.last_connection_attempt = Instant::now();

        if let Err(e) = reader.try_connect() {
            self.connection_status = format!("Disconnected - {}", e);
            if self.verbose {
                println!("Connection attempt failed: {}", e);
            }
        } else {
            self.connection_status = "Connected".to_string();
            if self.verbose {
                println!("Successfully connected to shared memory");
            }
        }
    }

    pub fn check_connection(&mut self) {
        // Check if we need to attempt reconnection
        if !self.shm_reader.lock().unwrap().is_connected() {
            if self.last_connection_attempt.elapsed() >= self.reconnect_delay {
                self.try_connect();
            }
            return;
        }

        // Check connection health
        let mut reader = self.shm_reader.lock().unwrap();
        if !reader.check_connection_health() {
            // Try to reopen the connection
            if let Err(e) = reader.reopen() {
                self.connection_status = format!("Disconnected - {}", e);
                if self.verbose {
                    println!("Failed to reopen connection: {}", e);
                }
            }
        }
    }

    pub fn update_frame(&mut self) {
        // Start frame processing timer
        let process_start = Instant::now();

        // Try to get a new frame with minimal latency
        let mut reader = self.shm_reader.lock().unwrap();

        // Your existing frame update logic that uses your optimized shm_reader
        match reader.get_next_frame(self.catch_up) {
            Ok(Some((header, data))) => {
                // Successfully got a frame
                self.frame_header = Some(header);
                self.frame_width = header.width as usize;
                self.frame_height = header.height as usize;

                // Calculate latency (producer timestamp to now)
                let now = Instant::now();
                let current_time_ns = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as u64;

                // Calculate latency from producer's timestamp
                let latency_ns = if current_time_ns > header.timestamp {
                    current_time_ns - header.timestamp
                } else {
                    0 // Handle clock misalignment
                };

                self.latency_ms = latency_ns as f64 / 1_000_000.0; // ns to ms

                // Call the appropriate format conversion based on header format
                // Note: The actual implementation would call your SIMD optimized functions
                self.frame_data = crate::shared_memory::convert_frame_to_rgb(
                    data,
                    self.frame_width,
                    self.frame_height,
                    header.bytes_per_pixel as usize,
                    header.format_code,
                    &self.format
                );

                // Update format string
                self.format = crate::shared_memory::format_code_to_string(header.format_code).to_string();

                // Update FPS tracking
                self.frames_received += 1;
                self.last_frame_time = now;

                // Update FPS counter every 500ms for more stable readings
                if self.last_fps_update.elapsed() >= Duration::from_millis(500) {
                    self.fps = self.frames_received as f64 / self.last_fps_update.elapsed().as_secs_f64();
                    self.frames_received = 0;
                    self.last_fps_update = now;

                    // Update total frames count
                    if let Ok((total_written, _, _)) = reader.get_stats() {
                        self.total_frames = total_written;
                    }
                }

                // Update connection status
                self.connection_status = "Connected".to_string();

                // Record frame processing time
                self.process_time_us = process_start.elapsed().as_micros() as u64;
            },
            Ok(None) => {
                // No new frames, but still connected
            },
            Err(e) => {
                // Error reading frame - likely disconnected
                self.connection_status = format!("Connection error: {}", e);
                if self.verbose {
                    println!("Error reading frame: {}", e);
                }
            }
        }
    }

    // Optimized method to update or create texture with minimal allocations
    pub fn update_or_create_texture(&mut self, ctx: &egui::Context) -> Option<egui::TextureId> {
        if self.frame_width == 0 || self.frame_height == 0 || self.frame_data.is_empty() {
            return None;
        }

        // Start texture creation timer
        let texture_start = Instant::now();

        // Reuse the GPU buffer if possible to avoid allocation
        let buffer_size = self.frame_width * self.frame_height * 4;
        if self.gpu_buffer.len() != buffer_size {
            self.gpu_buffer = vec![0u8; buffer_size];
        }

        // Fast path: direct memory copy from frame_data to gpu_buffer
        let mut idx = 0;
        for color in &self.frame_data {
            self.gpu_buffer[idx] = color.r();
            self.gpu_buffer[idx + 1] = color.g();
            self.gpu_buffer[idx + 2] = color.b();
            self.gpu_buffer[idx + 3] = 255; // Force full alpha
            idx += 4;
        }

        // Create or update texture
        let texture_handle = ctx.load_texture(
            "frame_image",
            egui::ColorImage::from_rgba_unmultiplied(
                [self.frame_width, self.frame_height],
                &self.gpu_buffer
            ),
            egui::TextureOptions::LINEAR
        );

        // Record texture processing time
        self.texture_time_us = texture_start.elapsed().as_micros() as u64;

        Some(texture_handle.id())
    }
}

impl eframe::App for EchoViewer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Update time delta between frames for animations
        let now = Instant::now();
        let dt = now.duration_since(self.animation.last_update).as_secs_f32();
        self.animation.last_update = now;

        // Update animations
        crate::ui::animations::update_animations(self, dt);

        // Configure styles if first time or on theme change
        crate::ui::theme::configure_styles(self, ctx);

        // Request a repaint for the next frame
        ctx.request_repaint();

        // Check connection and update frame
        self.check_connection();
        self.update_frame();

        // Draw UI panels
        crate::ui::panels::top_panel::draw(self, ctx);

        if self.show_tools_panel {
            crate::ui::panels::tools_panel::draw(self, ctx);
        }

        if self.show_info_panel {
            crate::ui::panels::info_panel::draw(self, ctx);
        }

        crate::ui::panels::central_panel::draw(self, ctx);
        crate::ui::panels::bottom_panel::draw(self, ctx);
    }
}