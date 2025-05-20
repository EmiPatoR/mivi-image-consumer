// ui/theme.rs - Theme definitions and management

use crate::app::EchoViewer;
use eframe::egui;
use egui::*;

// Theme enumeration
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Theme {
    Light,
    Dark,
    HighContrast,
    MedicalBlue, // Professional medical theme
    NightMode,   // Eye-friendly night mode for low light environments
}

// Patient information structure
pub struct PatientInfo {
    pub id: String,
    pub name: String,
    pub dob: String,
    pub study_date: String,
    pub modality: String,
    pub doctor: String,
    pub hospital: String,
}

impl Default for PatientInfo {
    fn default() -> Self {
        Self {
            id: "ID12345".to_string(),
            name: "[Patient Name]".to_string(),
            dob: "YYYY-MM-DD".to_string(),
            study_date: "2025-05-20".to_string(),
            modality: "Ultrasound".to_string(),
            doctor: "Dr. Sarah Johnson".to_string(),
            hospital: "Central Medical Center".to_string(),
        }
    }
}

// UI colors for the application
pub struct UiColors {
    pub primary: Color32,
    pub secondary: Color32,
    pub accent: Color32,
    pub background: Color32,
    pub panel_bg: Color32,
    pub text: Color32,
    pub text_secondary: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub error: Color32,
    pub button_bg: Color32,
    pub button_active: Color32,
    pub button_hover: Color32,
    pub border_light: Color32,
    pub border_dark: Color32,
    pub shadow: Color32,
}

impl Default for UiColors {
    fn default() -> Self {
        // Default medical theme colors
        Self {
            primary: Color32::from_rgb(28, 39, 65),         // Deeper blue
            secondary: Color32::from_rgb(41, 90, 165),      // Softer blue
            accent: Color32::from_rgb(56, 177, 189),        // Brighter teal accent
            background: Color32::from_rgb(16, 20, 32),      // Darker background for contrast
            panel_bg: Color32::from_rgb(22, 27, 38),        // Slightly lighter than background
            text: Color32::from_rgb(235, 240, 250),         // Softer white for better eye comfort
            text_secondary: Color32::from_rgb(175, 185, 210), // Subtle secondary text
            success: Color32::from_rgb(70, 200, 120),       // Brighter green for better visibility
            warning: Color32::from_rgb(240, 180, 50),       // Warmer yellow
            error: Color32::from_rgb(225, 80, 80),          // Slightly softer red
            button_bg: Color32::from_rgb(38, 54, 91),       // Richer button color
            button_active: Color32::from_rgb(58, 120, 210), // Brighter active state
            button_hover: Color32::from_rgb(48, 100, 180),  // Clear hover state
            border_light: Color32::from_rgb(55, 65, 90),    // Subtle borders
            border_dark: Color32::from_rgb(35, 40, 60),     // Shadow borders
            shadow: Color32::from_rgba_premultiplied(8, 10, 16, 200), // Deeper shadows
        }
    }
}

// Helper function to interpolate between colors
pub fn lerp_color(col_1: Color32, col_2: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let r = lerp(col_1.r() as f32, col_2.r() as f32, t) as u8;
    let g = lerp(col_1.g() as f32, col_2.g() as f32, t) as u8;
    let b = lerp(col_1.b() as f32, col_2.b() as f32, t) as u8;
    let a = lerp(col_1.a() as f32, col_2.a() as f32, t) as u8;
    Color32::from_rgba_premultiplied(r, g, b, a)
}

// Linear interpolation helper
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

// Configure UI styles based on current theme
pub fn configure_styles(app: &mut EchoViewer, ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    // Configure text styles
    style.text_styles = [
        (TextStyle::Heading, FontId::new(20.0, egui::FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(16.0, egui::FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(14.0, egui::FontFamily::Monospace)),
        (TextStyle::Button, FontId::new(16.0, egui::FontFamily::Proportional)),
        (TextStyle::Small, FontId::new(12.0, egui::FontFamily::Proportional)),
    ].into();

    // Update colors based on theme
    update_theme_colors(app);

    // Set colors for a professional medical application
    match app.theme {
        Theme::MedicalBlue => {
            // Modern medical theme with blue tones
            style.visuals.dark_mode = true;
            style.visuals.panel_fill = app.colors.panel_bg;
            style.visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(30, 40, 60);
            style.visuals.widgets.inactive.bg_fill = app.colors.button_bg;
            style.visuals.widgets.active.bg_fill = app.colors.button_active;
            style.visuals.widgets.hovered.bg_fill = app.colors.button_hover;
            style.visuals.window_fill = app.colors.panel_bg;
            style.visuals.window_stroke = Stroke::new(1.0, app.colors.border_light);
        },
        Theme::Dark => {
            // Dark theme
            style.visuals.dark_mode = true;
            style.visuals.panel_fill = Color32::from_rgb(22, 25, 37);
            style.visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(30, 34, 46);
            style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(40, 44, 56);
            style.visuals.widgets.active.bg_fill = Color32::from_rgb(48, 107, 185);
            style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(58, 117, 195);
            style.visuals.window_fill = Color32::from_rgb(22, 25, 37);
            style.visuals.window_stroke = Stroke::new(1.0, Color32::from_rgb(40, 44, 56));
        },
        Theme::Light => {
            // Light theme
            style.visuals.dark_mode = false;
            style.visuals.panel_fill = Color32::from_rgb(240, 244, 248);
            style.visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(230, 236, 242);
            style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(220, 228, 236);
            style.visuals.widgets.active.bg_fill = Color32::from_rgb(70, 130, 210);
            style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(90, 150, 230);
            style.visuals.window_fill = Color32::from_rgb(240, 244, 248);
            style.visuals.window_stroke = Stroke::new(1.0, Color32::from_rgb(200, 210, 220));
        },
        Theme::NightMode => {
            // Night mode for dark environments
            style.visuals.dark_mode = true;
            style.visuals.panel_fill = Color32::from_rgb(12, 15, 27);
            style.visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(18, 22, 35);
            style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(25, 30, 45);
            style.visuals.widgets.active.bg_fill = Color32::from_rgb(40, 80, 140);
            style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(35, 65, 120);
            style.visuals.window_fill = Color32::from_rgb(12, 15, 27);
            style.visuals.window_stroke = Stroke::new(1.0, Color32::from_rgb(30, 35, 50));
        },
        Theme::HighContrast => {
            // High contrast theme
            style.visuals.dark_mode = true;
            style.visuals.panel_fill = Color32::BLACK;
            style.visuals.widgets.noninteractive.bg_fill = Color32::BLACK;
            style.visuals.widgets.inactive.bg_fill = Color32::DARK_GRAY;
            style.visuals.widgets.active.bg_fill = Color32::WHITE;
            style.visuals.widgets.hovered.bg_fill = Color32::LIGHT_GRAY;
            style.visuals.window_fill = Color32::BLACK;
            style.visuals.window_stroke = Stroke::new(2.0, Color32::WHITE);
        }
    }

    // Add button rounding
    style.visuals.widgets.noninteractive.corner_radius = CornerRadius::same(6);
    style.visuals.widgets.inactive.corner_radius = CornerRadius::same(6);
    style.visuals.widgets.active.corner_radius = CornerRadius::same(6);
    style.visuals.widgets.hovered.corner_radius = CornerRadius::same(6);

    // Set window rounding
    style.visuals.window_corner_radius = CornerRadius::same(8);
    style.visuals.popup_shadow.spread = 10;

    // Enhanced shadows
    style.visuals.popup_shadow.color = Color32::from_rgba_premultiplied(0, 0, 0, 180);

    // Apply the style
    ctx.set_style(style);
}

// Update colors based on current theme
pub fn update_theme_colors(app: &mut EchoViewer) {
    match app.theme {
        Theme::MedicalBlue => {
            app.colors = UiColors {
                primary: Color32::from_rgb(28, 39, 65),         // Deeper blue
                secondary: Color32::from_rgb(41, 90, 165),      // Softer blue
                accent: Color32::from_rgb(56, 177, 189),        // Brighter teal accent
                background: Color32::from_rgb(16, 20, 32),      // Darker background for contrast
                panel_bg: Color32::from_rgb(22, 27, 38),        // Slightly lighter than background
                text: Color32::from_rgb(235, 240, 250),         // Softer white for better eye comfort
                text_secondary: Color32::from_rgb(175, 185, 210), // Subtle secondary text
                success: Color32::from_rgb(70, 200, 120),       // Brighter green for better visibility
                warning: Color32::from_rgb(240, 180, 50),       // Warmer yellow
                error: Color32::from_rgb(225, 80, 80),          // Slightly softer red
                button_bg: Color32::from_rgb(38, 54, 91),       // Richer button color
                button_active: Color32::from_rgb(58, 120, 210), // Brighter active state
                button_hover: Color32::from_rgb(48, 100, 180),  // Clear hover state
                border_light: Color32::from_rgb(55, 65, 90),    // Subtle borders
                border_dark: Color32::from_rgb(35, 40, 60),     // Shadow borders
                shadow: Color32::from_rgba_premultiplied(8, 10, 16, 200), // Deeper shadows
            };
        },
        Theme::NightMode => {
            app.colors = UiColors {
                primary: Color32::from_rgb(15, 20, 35),
                secondary: Color32::from_rgb(40, 60, 120),
                accent: Color32::from_rgb(60, 150, 170),
                background: Color32::from_rgb(10, 12, 20),
                panel_bg: Color32::from_rgb(15, 18, 30),
                text: Color32::from_rgb(200, 205, 225),
                text_secondary: Color32::from_rgb(140, 145, 175),
                success: Color32::from_rgb(60, 160, 100),
                warning: Color32::from_rgb(200, 150, 50),
                error: Color32::from_rgb(180, 60, 60),
                button_bg: Color32::from_rgb(30, 40, 70),
                button_active: Color32::from_rgb(50, 90, 170),
                button_hover: Color32::from_rgb(40, 70, 140),
                border_light: Color32::from_rgb(40, 50, 80),
                border_dark: Color32::from_rgb(25, 30, 50),
                shadow: Color32::from_rgba_premultiplied(5, 7, 12, 200),
            };
        },
        Theme::Dark => {
            app.colors = UiColors {
                primary: Color32::from_rgb(30, 30, 40),
                secondary: Color32::from_rgb(50, 90, 160),
                accent: Color32::from_rgb(80, 170, 180),
                background: Color32::from_rgb(22, 25, 37),
                panel_bg: Color32::from_rgb(30, 34, 46),
                text: Color32::from_rgb(220, 225, 235),
                text_secondary: Color32::from_rgb(160, 165, 185),
                success: Color32::from_rgb(80, 210, 130),
                warning: Color32::from_rgb(245, 190, 65),
                error: Color32::from_rgb(230, 90, 90),
                button_bg: Color32::from_rgb(40, 44, 56),
                button_active: Color32::from_rgb(60, 110, 180),
                button_hover: Color32::from_rgb(50, 95, 160),
                border_light: Color32::from_rgb(50, 55, 75),
                border_dark: Color32::from_rgb(35, 38, 55),
                shadow: Color32::from_rgba_premultiplied(10, 12, 18, 200),
            };
        },
        Theme::Light => {
            app.colors = UiColors {
                primary: Color32::from_rgb(230, 235, 245),
                secondary: Color32::from_rgb(70, 130, 210),
                accent: Color32::from_rgb(40, 150, 160),
                background: Color32::from_rgb(240, 244, 248),
                panel_bg: Color32::from_rgb(230, 235, 242),
                text: Color32::from_rgb(40, 45, 70),
                text_secondary: Color32::from_rgb(80, 90, 120),
                success: Color32::from_rgb(40, 170, 90),
                warning: Color32::from_rgb(220, 160, 40),
                error: Color32::from_rgb(200, 60, 60),
                button_bg: Color32::from_rgb(220, 228, 236),
                button_active: Color32::from_rgb(70, 130, 210),
                button_hover: Color32::from_rgb(90, 150, 230),
                border_light: Color32::from_rgb(200, 210, 220),
                border_dark: Color32::from_rgb(180, 190, 210),
                shadow: Color32::from_rgba_premultiplied(100, 110, 140, 100),
            };
        },
        Theme::HighContrast => {
            app.colors = UiColors {
                primary: Color32::BLACK,
                secondary: Color32::WHITE,
                accent: Color32::from_rgb(255, 255, 0),
                background: Color32::BLACK,
                panel_bg: Color32::BLACK,
                text: Color32::WHITE,
                text_secondary: Color32::from_rgb(220, 220, 220),
                success: Color32::from_rgb(0, 255, 0),
                warning: Color32::from_rgb(255, 255, 0),
                error: Color32::from_rgb(255, 0, 0),
                button_bg: Color32::DARK_GRAY,
                button_active: Color32::WHITE,
                button_hover: Color32::LIGHT_GRAY,
                border_light: Color32::WHITE,
                border_dark: Color32::from_rgb(150, 150, 150),
                shadow: Color32::from_rgba_premultiplied(0, 0, 0, 255),
            };
        }
    }
}