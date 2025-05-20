// ui/panels/top_panel.rs - Top application bar implementation

use eframe::egui;
use egui::*;
use crate::app::EchoViewer;
use crate::ui::theme::lerp_color;
use crate::ui::widgets::{self, glass_panel};

// Draw the top panel with header and patient info
pub fn draw(app: &mut EchoViewer, ctx: &egui::Context) {
    egui::TopBottomPanel::top("header_panel")
        .height_range(48.0..=48.0)
        .show(ctx, |ui| {
            // Apply a nice gradient to the top panel - IMPROVED GRADIENT IMPLEMENTATION
            let header_rect = ui.max_rect();
            let top_color = app.colors.primary;
            let bottom_color = app.colors.panel_bg;

            // Draw smoother gradient background - improve with fewer steps
            let steps = 20; // Reduced steps for smoother appearance
            for i in 0..steps {
                let t = i as f32 / (steps as f32 - 1.0);
                let color = crate::ui::theme::lerp_color(top_color, bottom_color, t);
                let y_start = header_rect.min.y + (header_rect.height() * (i as f32 / steps as f32));
                let y_end = header_rect.min.y + (header_rect.height() * ((i + 1) as f32 / steps as f32));
                let height = y_end - y_start;

                ui.painter().rect_filled(
                    Rect::from_min_max(
                        Pos2::new(header_rect.min.x, y_start),
                        Pos2::new(header_rect.max.x, y_end)
                    ),
                    CornerRadius::same(0),
                    color
                );
            }

            // Logo glow based on pulse animation
            if app.show_logo {
                let logo_rect = Rect::from_min_size(
                    Pos2::new(header_rect.min.x + 10.0, header_rect.min.y + 8.0),
                    Vec2::new(32.0, 32.0)
                );

                let glow_size = 4.0 + app.animation.pulse_value * 2.0;
                let glow_color = Color32::from_rgba_premultiplied(
                    app.colors.accent.r(),
                    app.colors.accent.g(),
                    app.colors.accent.b(),
                    (100.0 * app.animation.pulse_value) as u8
                );

                // Draw glow
                ui.painter().circle_filled(
                    logo_rect.center(),
                    logo_rect.width() / 2.0 + glow_size,
                    glow_color
                );

                // Draw logo background
                ui.painter().circle_filled(
                    logo_rect.center(),
                    logo_rect.width() / 2.0,
                    crate::ui::theme::lerp_color(app.colors.secondary, app.colors.accent, 0.3)
                );

                // Draw stylized ultrasound "waves" icon
                let center = logo_rect.center();
                let radius = logo_rect.width() / 2.0 - 4.0;

                // Draw wave arcs
                for i in 0..3 {
                    let r = radius - i as f32 * 4.0;
                    if r > 0.0 {
                        ui.painter().circle_stroke(
                            center,
                            r,
                            Stroke::new(1.5, Color32::from_rgba_premultiplied(255, 255, 255, 200))
                        );
                    }
                }

                // Draw a small circle at center
                ui.painter().circle_filled(
                    center,
                    2.0,
                    Color32::WHITE
                );
            }

            // Use a layout that makes better use of the available space
            ui.horizontal(|ui| {
                // Logo/Application name with animation
                ui.add_space(48.0); // Space for logo

                let title_color = crate::ui::theme::lerp_color(
                    app.colors.text,
                    app.colors.accent,
                    app.animation.pulse_value * 0.1
                );

                ui.label(
                    RichText::new("MiVi Echography Viewer")
                        .heading()
                        .color(title_color)
                        .strong()
                );

                // Spacer to help position the patient info
                ui.add_space(20.0);

                // Patient information with smooth reveal animation - use a fixed width
                if app.show_patient_details {
                    // Create a dedicated patient card
                    let card_rect = Rect::from_min_size(
                        Pos2::new(ui.cursor().min.x, header_rect.min.y + 6.0),
                        Vec2::new(280.0, 36.0)
                    );

                    // Draw card background with glass effect - with proper alpha!
                    glass_panel(ui, card_rect, 8.0, 180);

                    // Add subtle patient icon
                    ui.painter().text(
                        Pos2::new(card_rect.min.x + 20.0, card_rect.center().y),
                        Align2::LEFT_CENTER,
                        "👤",
                        FontId::proportional(14.0),
                        Color32::from_rgba_premultiplied(180, 190, 210, 200)
                    );

                    let alpha = (app.panel_alpha * 255.0) as u8;
                    let patient_info_width = 280.0; // Fixed width for patient info

                    ui.allocate_ui(Vec2::new(patient_info_width, ui.available_height()), |ui| {
                        ui.horizontal(|ui| {
                            // First column - labels
                            ui.vertical(|ui| {
                                ui.label(RichText::new("Patient:").strong().size(12.0)
                                    .color(Color32::from_rgba_premultiplied(
                                        app.colors.text.r(), app.colors.text.g(), app.colors.text.b(), alpha
                                    )));
                                ui.label(RichText::new("ID:").strong().size(12.0)
                                    .color(Color32::from_rgba_premultiplied(
                                        app.colors.text.r(), app.colors.text.g(), app.colors.text.b(), alpha
                                    )));
                            });

                            // First column - values
                            ui.vertical(|ui| {
                                ui.label(RichText::new(&app.patient_info.name).size(12.0)
                                    .color(Color32::from_rgba_premultiplied(
                                        app.colors.text.r(), app.colors.text.g(), app.colors.text.b(), alpha
                                    )));
                                ui.label(RichText::new(&app.patient_info.id).size(12.0)
                                    .color(Color32::from_rgba_premultiplied(
                                        app.colors.text.r(), app.colors.text.g(), app.colors.text.b(), alpha
                                    )));
                            });

                            ui.add_space(20.0);

                            // Second column - labels
                            ui.vertical(|ui| {
                                ui.label(RichText::new("DOB:").strong().size(12.0)
                                    .color(Color32::from_rgba_premultiplied(
                                        app.colors.text.r(), app.colors.text.g(), app.colors.text.b(), alpha
                                    )));
                                ui.label(RichText::new("Study:").strong().size(12.0)
                                    .color(Color32::from_rgba_premultiplied(
                                        app.colors.text.r(), app.colors.text.g(), app.colors.text.b(), alpha
                                    )));
                            });

                            // Second column - values
                            ui.vertical(|ui| {
                                ui.label(RichText::new(&app.patient_info.dob).size(12.0)
                                    .color(Color32::from_rgba_premultiplied(
                                        app.colors.text.r(), app.colors.text.g(), app.colors.text.b(), alpha
                                    )));
                                ui.label(RichText::new(&app.patient_info.study_date).size(12.0)
                                    .color(Color32::from_rgba_premultiplied(
                                        app.colors.text.r(), app.colors.text.g(), app.colors.text.b(), alpha
                                    )));
                            });
                        });
                    });
                }

                // Expand to push right-aligned controls to the right edge
                ui.add_space(ui.available_width() - 320.0); // Reserve space for right controls

                // Right-aligned controls
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(8.0);

                    // Theme selection with animation
                    let theme_button_text = match app.theme {
                        crate::ui::theme::Theme::MedicalBlue => "🧪 Medical",
                        crate::ui::theme::Theme::Dark => "🌙 Dark",
                        crate::ui::theme::Theme::Light => "☀️ Light",
                        crate::ui::theme::Theme::NightMode => "🌃 Night",
                        crate::ui::theme::Theme::HighContrast => "🔍 High Contrast",
                    };

                    // Determine if this is our hovered button
                    let is_theme_hovered = app.hovered_button == Some(0);

                    // If hovered, update our tracked state for animation
                    if ui.rect_contains_pointer(ui.min_rect().expand(20.0)) {
                        app.hovered_button = Some(0);
                    } else if app.hovered_button == Some(0) {
                        app.hovered_button = None;
                    }

                    // Animated theme button
                    if crate::ui::widgets::pulse_button(
                        ui,
                        theme_button_text,
                        Vec2::new(110.0, 32.0),
                        if is_theme_hovered { app.animation.pulse_value } else { 0.0 },
                        is_theme_hovered
                    ).clicked() {
                        // Cycle through themes
                        app.theme = match app.theme {
                            crate::ui::theme::Theme::MedicalBlue => crate::ui::theme::Theme::Dark,
                            crate::ui::theme::Theme::Dark => crate::ui::theme::Theme::Light,
                            crate::ui::theme::Theme::Light => crate::ui::theme::Theme::NightMode,
                            crate::ui::theme::Theme::NightMode => crate::ui::theme::Theme::HighContrast,
                            crate::ui::theme::Theme::HighContrast => crate::ui::theme::Theme::MedicalBlue,
                        };

                        // Update colors for the new theme
                        crate::ui::theme::update_theme_colors(app);

                        // Force a complete redraw/update when changing themes
                        ctx.request_repaint();
                    }

                    ui.add_space(10.0);

                    // Connection status with a professional animated look
                    let (status_text, status_color) = if app.connection_status.starts_with("Connected") {
                        ("Connected", app.colors.success)
                    } else {
                        ("Disconnected", app.colors.error)
                    };

                    ui.horizontal(|ui| {
                        ui.label("Status:");

                        // Status indicator with animation
                        let indicator_size = 8.0 + if status_text == "Connected" {
                            app.animation.pulse_value * 2.0
                        } else {
                            (app.animation.reconnect_pulse).sin() * 2.0
                        };

                        let indicator_rect = Rect::from_center_size(
                            Pos2::new(ui.cursor().min.x + 6.0, ui.cursor().min.y + 9.0),
                            Vec2::new(indicator_size, indicator_size)
                        );

                        // Glow effect
                        ui.painter().circle_filled(
                            indicator_rect.center(),
                            indicator_size + 2.0,
                            Color32::from_rgba_premultiplied(
                                status_color.r(),
                                status_color.g(),
                                status_color.b(),
                                80
                            )
                        );

                        ui.painter().circle_filled(
                            indicator_rect.center(),
                            indicator_size,
                            status_color
                        );

                        ui.add_space(15.0);
                        ui.label(RichText::new(status_text).color(status_color).strong());
                    });

                    ui.add_space(10.0);

                    // Reconnect button with icon and animation
                    if crate::ui::widgets::pulse_button(
                        ui,
                        "🔄 Reconnect",
                        Vec2::new(100.0, 32.0),
                        if !app.connection_status.starts_with("Connected") { app.animation.pulse_value } else { 0.0 },
                        ui.rect_contains_pointer(ui.min_rect().expand(20.0))
                    ).clicked() {
                        app.try_connect();
                    }
                });
            });

            // Bottom highlight
            ui.painter().line_segment(
                [Pos2::new(header_rect.min.x, header_rect.max.y),
                    Pos2::new(header_rect.max.x, header_rect.max.y)],
                Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 255, 255, 25))
            );
        });
}