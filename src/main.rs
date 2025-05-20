// main.rs - Entry point for the application

use clap::Parser;
use eframe::egui;
use egui::{FontId, IconData, TextStyle};

mod app;
mod shared_memory; // This contains your existing SharedMemoryReader implementation
mod ui;

use app::EchoViewer;

// Keep your existing Args struct here
#[derive(Parser, Debug)]
#[command(name = "Medical Echography Viewer")]
#[command(about = "Displays echography frames from shared memory in real-time")]
struct Args {
    /// Name of the shared memory region
    #[arg(short, long, default_value = "ultrasound_frames")]
    shm_name: String,

    /// Format of the frames (rgb, bgr, yuv)
    #[arg(short, long, default_value = "bgra")]
    format: String,

    /// Width of the window
    #[arg(short, long, default_value_t = 1024)]
    width: usize,

    /// Height of the window
    #[arg(short, long, default_value_t = 768)]
    height: usize,

    /// Skip to latest frame rather than processing sequentially
    #[arg(short, long, default_value_t = true)] // Default changed to true for medical use
    catch_up: bool,

    /// Dump first few frames to files for debugging
    #[arg(long, default_value_t = false)]
    dump_frames: bool,

    /// Enable verbose debug output
    #[arg(short, long, default_value_t = false)]
    verbose: bool,

    /// Reconnection delay in milliseconds
    #[arg(long, default_value_t = 500)] // Reduced delay for medical use
    reconnect_delay: u64,

    /// CPU core to pin the application to (-1 for no pinning)
    #[arg(long, default_value_t = -1)]
    cpu_core: i32,

    /// Enable high priority thread scheduling
    #[arg(long, default_value_t = true)]
    high_priority: bool,
}

// Main entry point for the application
fn main() -> Result<(), eframe::Error> {
    // Parse command line arguments
    let args = Args::parse();

    // Apply CPU pinning if requested
    if args.cpu_core >= 0 {
        unsafe {
            let mut cpu_set: libc::cpu_set_t = std::mem::zeroed();
            libc::CPU_ZERO(&mut cpu_set);
            libc::CPU_SET(args.cpu_core as usize, &mut cpu_set);

            libc::pthread_setaffinity_np(
                libc::pthread_self(),
                std::mem::size_of::<libc::cpu_set_t>(),
                &cpu_set
            );

            println!("Application pinned to CPU core {}", args.cpu_core);
        }
    }

    // Set high priority for UI thread if requested
    if args.high_priority {
        unsafe {
            let mut sched_param: libc::sched_param = std::mem::zeroed();
            sched_param.sched_priority = 90; // High priority
            let result = libc::pthread_setschedparam(
                libc::pthread_self(),
                libc::SCHED_RR,
                &sched_param
            );

            if result == 0 {
                println!("Thread priority set to high (SCHED_RR, 90)");

                // Set I/O priority to real-time
                let io_result = libc::syscall(libc::SYS_ioprio_set, 1, 0, 4 << 13);
                if io_result == 0 {
                    println!("I/O priority set to real-time");
                }
            } else {
                println!("Failed to set thread priority, error: {}", result);
            }
        }
    }

    // Create eframe options with professional-grade settings
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([args.width as f32, args.height as f32])
            .with_min_inner_size([800.0, 600.0])
            .with_resizable(true)
            .with_decorations(true)
            .with_transparent(false)
            .with_icon(create_app_icon()),
        vsync: false, // Disable VSync for minimal latency
        hardware_acceleration: eframe::HardwareAcceleration::Required,
        // renderer field removed as it may not be available in this version
        ..Default::default()
    };

    // Run the application
    eframe::run_native(
        "Medical Echography Viewer",
        native_options,
        Box::new(|cc| {
            // Set custom fonts if needed
            setup_custom_fonts(&cc.egui_ctx);

            // Apply a default GUI scaling that's good for medical displays
            cc.egui_ctx.set_pixels_per_point(1.2);

            Ok(Box::new(EchoViewer::new(args)))
        })
    )
}

// Create application icon 
fn create_app_icon() -> IconData {
    // A simple medical-themed icon
    let width = 32;
    let height = 32;
    let mut rgba = Vec::with_capacity(width * height * 4);

    // Icon color (medical blue)
    let icon_color = [48, 107, 185, 255]; // RGBA
    let bg_color = [30, 40, 60, 255]; // Dark blue background
    let accent_color = [66, 185, 196, 255]; // Accent color

    for y in 0..height {
        for x in 0..width {
            // Calculate distance from center for circular icon
            let cx = x as f32 - width as f32 / 2.0;
            let cy = y as f32 - height as f32 / 2.0;
            let dist = (cx * cx + cy * cy).sqrt();
            let radius = width as f32 / 2.0;

            if dist < radius - 2.0 {
                // Main circle
                rgba.extend_from_slice(&bg_color);
            } else if dist < radius {
                // Border
                rgba.extend_from_slice(&icon_color);
            } else {
                // Transparent background
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }

    // Draw "waves" in the center (stylized ultrasound icon)
    for y in 0..height {
        for x in 0..width {
            let cx = x as f32 - width as f32 / 2.0;
            let cy = y as f32 - height as f32 / 2.0;
            let dist = (cx * cx + cy * cy).sqrt();

            // Create wave patterns
            for r in [12.0, 8.0, 4.0] {
                if (dist - r).abs() < 0.8 {
                    let idx = (y * width + x) * 4;
                    if idx < rgba.len() - 3 {
                        rgba[idx..idx+4].copy_from_slice(&accent_color);
                    }
                }
            }

            // Center dot
            if dist < 1.5 {
                let idx = (y * width + x) * 4;
                if idx < rgba.len() - 3 {
                    rgba[idx..idx+4].copy_from_slice(&[255, 255, 255, 255]);
                }
            }
        }
    }

    IconData {
        rgba,
        width: width as u32,
        height: height as u32,
    }
}

// Setup custom fonts for the application
fn setup_custom_fonts(ctx: &egui::Context) {
    // Just use the default fonts that come with egui
    // This is the safest approach and should work on any system

    // Set slightly larger default sizes for the default fonts
    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (TextStyle::Heading, FontId::new(22.0, egui::FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(16.0, egui::FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(14.0, egui::FontFamily::Monospace)),
        (TextStyle::Button, FontId::new(16.0, egui::FontFamily::Proportional)),
        (TextStyle::Small, FontId::new(12.0, egui::FontFamily::Proportional)),
    ].into();

    ctx.set_style(style);
}