// ui/panels/central_panel.rs - Main image display implementation

use crate::app::EchoViewer;
use crate::ui::tools;
use crate::ui::widgets::{glass_panel, solid_panel};
use eframe::egui;
use egui::epaint::CornerRadiusF32;
use egui::*;
use std::time::Instant;

// Draw rulers around the image with animation effects
fn draw_rulers(app: &EchoViewer, ui: &egui::Ui, image_rect: Rect) {
    // Fade-in animation
    let alpha = (app.panel_alpha * 200.0) as u8;

    let stroke = Stroke::new(1.0, Color32::from_rgba_premultiplied(200, 200, 200, alpha));
    let text_color = Color32::from_rgba_premultiplied(200, 200, 200, alpha);

    // Semi-transparent ruler background
    let ruler_bg = Color32::from_rgba_premultiplied(20, 25, 35, alpha);

    // Horizontal ruler (top)
    let ruler_height = 20.0;
    let ruler_rect = Rect::from_min_max(
        Pos2::new(image_rect.min.x, image_rect.min.y - ruler_height),
        Pos2::new(image_rect.max.x, image_rect.min.y),
    );

    ui.painter()
        .rect_filled(ruler_rect, CornerRadiusF32::same(0.0), ruler_bg);

    // Ticks every 50 pixels
    let tick_interval = 50.0;
    let mut x = image_rect.min.x;
    while x <= image_rect.max.x {
        let tick_height = if (x - image_rect.min.x) % 100.0 < 1.0 {
            8.0
        } else {
            5.0
        };

        ui.painter().line_segment(
            [
                Pos2::new(x, image_rect.min.y - tick_height),
                Pos2::new(x, image_rect.min.y),
            ],
            stroke,
        );

        // Labels at major ticks with animation
        if (x - image_rect.min.x) % 100.0 < 1.0 {
            let label = format!(
                "{}",
                ((x - image_rect.min.x) / app.animation.zoom_anim) as i32
            );
            ui.painter().text(
                Pos2::new(x, image_rect.min.y - 12.0),
                egui::Align2::CENTER_CENTER,
                label,
                FontId::proportional(10.0),
                text_color,
            );
        }

        x += tick_interval;
    }

    // Vertical ruler (left)
    let ruler_width = 20.0;
    let ruler_rect = Rect::from_min_max(
        Pos2::new(image_rect.min.x - ruler_width, image_rect.min.y),
        Pos2::new(image_rect.min.x, image_rect.max.y),
    );

    ui.painter()
        .rect_filled(ruler_rect, CornerRadiusF32::same(0.0), ruler_bg);

    // Ticks every 50 pixels
    let mut y = image_rect.min.y;
    while y <= image_rect.max.y {
        let tick_width = if (y - image_rect.min.y) % 100.0 < 1.0 {
            8.0
        } else {
            5.0
        };

        ui.painter().line_segment(
            [
                Pos2::new(image_rect.min.x - tick_width, y),
                Pos2::new(image_rect.min.x, y),
            ],
            stroke,
        );

        // Labels at major ticks
        if (y - image_rect.min.y) % 100.0 < 1.0 {
            let label = format!(
                "{}",
                ((y - image_rect.min.y) / app.animation.zoom_anim) as i32
            );
            ui.painter().text(
                Pos2::new(image_rect.min.x - 12.0, y),
                egui::Align2::CENTER_CENTER,
                label,
                FontId::proportional(10.0),
                text_color,
            );
        }

        y += tick_interval;
    }
}

// Draw animated grid
fn draw_animated_grid(app: &EchoViewer, ui: &egui::Ui, image_rect: Rect) {
    // Fade-in animation
    let base_alpha = (app.panel_alpha * 120.0) as u8;

    // Grid size
    let grid_size = 50.0;

    // Vertical lines
    let mut x = image_rect.min.x + grid_size;
    while x < image_rect.max.x {
        // Calculate distance from center for alpha variation
        let center_dist = (x - image_rect.center().x).abs() / (image_rect.width() / 2.0);
        let alpha = (base_alpha as f32 * (1.0 - center_dist * 0.3)) as u8;

        let stroke = Stroke::new(
            if (x - image_rect.min.x) % 100.0 < 1.0 {
                1.0
            } else {
                0.5
            },
            Color32::from_rgba_premultiplied(150, 150, 150, alpha),
        );

        ui.painter().line_segment(
            [
                Pos2::new(x, image_rect.min.y),
                Pos2::new(x, image_rect.max.y),
            ],
            stroke,
        );
        x += grid_size;
    }

    // Horizontal lines
    let mut y = image_rect.min.y + grid_size;
    while y < image_rect.max.y {
        // Calculate distance from center for alpha variation
        let center_dist = (y - image_rect.center().y).abs() / (image_rect.height() / 2.0);
        let alpha = (base_alpha as f32 * (1.0 - center_dist * 0.3)) as u8;

        let stroke = Stroke::new(
            if (y - image_rect.min.y) % 100.0 < 1.0 {
                1.0
            } else {
                0.5
            },
            Color32::from_rgba_premultiplied(150, 150, 150, alpha),
        );

        ui.painter().line_segment(
            [
                Pos2::new(image_rect.min.x, y),
                Pos2::new(image_rect.max.x, y),
            ],
            stroke,
        );
        y += grid_size;
    }

    // Center crosshair for reference
    let center = image_rect.center();
    let crosshair_size = 20.0;
    let crosshair_color = Color32::from_rgba_premultiplied(
        app.colors.accent.r(),
        app.colors.accent.g(),
        app.colors.accent.b(),
        base_alpha,
    );

    ui.painter().line_segment(
        [
            Pos2::new(center.x - crosshair_size, center.y),
            Pos2::new(center.x + crosshair_size, center.y),
        ],
        Stroke::new(1.0, crosshair_color),
    );

    ui.painter().line_segment(
        [
            Pos2::new(center.x, center.y - crosshair_size),
            Pos2::new(center.x, center.y + crosshair_size),
        ],
        Stroke::new(1.0, crosshair_color),
    );

    // Small circle at center
    ui.painter()
        .circle_stroke(center, 5.0, Stroke::new(1.0, crosshair_color));
}

// Draw the HUD overlaying the image
fn draw_hud(app: &EchoViewer, ui: &egui::Ui, image_rect: Rect) {
    if app.show_hud {
        let pos = Pos2::new(image_rect.max.x - 10.0, image_rect.min.y + 10.0);

        if let Some(header) = app.frame_header {
            let infos = [
                format!("FPS: {:.1}", app.fps),
                format!("Frame: {}", header.sequence_number),
                format!("{}×{}", header.width, header.height),
                format!("Latency: {:.1}ms", app.latency_ms),
            ];

            // Draw HUD with glassmorphism effect
            for (i, info) in infos.iter().enumerate() {
                let text_size = ui
                    .fonts(|f| {
                        f.layout_no_wrap(info.clone(), FontId::proportional(12.0), Color32::WHITE)
                    })
                    .rect
                    .size();

                let y_offset = i as f32 * (text_size.y + 8.0);
                let text_rect = Rect::from_min_max(
                    Pos2::new(pos.x - text_size.x - 14.0, pos.y + y_offset),
                    Pos2::new(pos.x, pos.y + y_offset + text_size.y + 8.0),
                );

                // Glass background with animation
                let animation_offset = (i as f32 * 0.1).min(0.3);
                let alpha = ((app.panel_alpha - animation_offset) / 0.7 * 180.0) as u8;

                // Use glass_panel function instead of direct drawing
                glass_panel(ui, text_rect, 5.0, alpha);

                // Text with shadow
                ui.painter().text(
                    Pos2::new(text_rect.center().x, text_rect.center().y + 1.0),
                    Align2::CENTER_CENTER,
                    info,
                    FontId::proportional(12.0),
                    Color32::from_rgba_premultiplied(0, 0, 0, alpha),
                );

                ui.painter().text(
                    text_rect.center(),
                    Align2::CENTER_CENTER,
                    info,
                    FontId::proportional(12.0),
                    Color32::from_rgba_premultiplied(255, 255, 255, alpha),
                );
            }
        }
    }
}

// Draw the central panel with the image and tools
pub fn draw(app: &mut EchoViewer, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        // If we're not connected, show an animated message
        if !app.shm_reader.lock().unwrap().is_connected() {
            ui.centered_and_justified(|ui| {
                // Professional-looking "no connection" message with animations
                let text_color = Color32::from_rgba_premultiplied(
                    app.colors.text.r(),
                    app.colors.text.g(),
                    app.colors.text.b(),
                    ((app.animation.startup_progress * 0.6 + 0.4) * 255.0) as u8,
                );

                let accent_color = Color32::from_rgba_premultiplied(
                    app.colors.accent.r(),
                    app.colors.accent.g(),
                    app.colors.accent.b(),
                    ((app.animation.startup_progress * 0.6 + 0.4) * 255.0) as u8,
                );

                ui.vertical_centered(|ui| {
                    ui.add_space(50.0);

                    // Animated icon with pulsing
                    let icon_size = 36.0 + app.animation.pulse_value * 4.0;
                    let icon_rect = Rect::from_center_size(
                        ui.next_widget_position() + Vec2::new(0.0, icon_size / 2.0),
                        Vec2::new(icon_size, icon_size),
                    );

                    // Icon background glow
                    ui.painter().circle_filled(
                        icon_rect.center(),
                        icon_size * 0.6,
                        Color32::from_rgba_premultiplied(
                            app.colors.accent.r(),
                            app.colors.accent.g(),
                            app.colors.accent.b(),
                            (app.animation.pulse_value * 50.0) as u8,
                        ),
                    );

                    // Rotating icon
                    let rotation_angle = app.elapsed_time * 1.5;
                    for i in 0..8 {
                        let angle = rotation_angle + i as f32 * std::f32::consts::PI / 4.0;
                        let distance = icon_size * 0.4;
                        let x = icon_rect.center().x + angle.cos() * distance;
                        let y = icon_rect.center().y + angle.sin() * distance;

                        let point_size = if i % 2 == 0 { 4.0 } else { 3.0 };
                        let alpha = if i % 2 == 0 { 255 } else { 180 };

                        ui.painter().circle_filled(
                            Pos2::new(x, y),
                            point_size,
                            Color32::from_rgba_premultiplied(
                                accent_color.r(),
                                accent_color.g(),
                                accent_color.b(),
                                alpha,
                            ),
                        );
                    }

                    ui.add_space(icon_size + 20.0);

                    // Text with slide-in animation from bottom
                    let slide_in_offset = (1.0 - app.animation.startup_progress) * 20.0;
                    let text_pos = ui.cursor().min + Vec2::new(0.0, slide_in_offset);

                    ui.painter().text(
                        text_pos,
                        Align2::CENTER_TOP,
                        "Waiting for Connection...",
                        FontId::new(24.0, egui::FontFamily::Proportional),
                        text_color,
                    );

                    ui.add_space(30.0);

                    // Subtitle with fade-in animation
                    let subtitle_alpha =
                        ((app.animation.startup_progress - 0.3).max(0.0) / 0.7 * 255.0) as u8;
                    ui.painter().text(
                        ui.cursor().min + Vec2::new(0.0, slide_in_offset * 0.5),
                        Align2::CENTER_TOP,
                        "Attempting to connect to ultrasound device",
                        FontId::new(16.0, egui::FontFamily::Proportional),
                        Color32::from_rgba_premultiplied(
                            text_color.r(),
                            text_color.g(),
                            text_color.b(),
                            subtitle_alpha,
                        ),
                    );

                    ui.add_space(40.0);

                    // Reconnect button with pulse animation
                    if crate::ui::widgets::pulse_button(
                        ui,
                        "Reconnect Now",
                        Vec2::new(150.0, 36.0),
                        app.animation.pulse_value,
                        ui.rect_contains_pointer(ui.min_rect().expand(60.0)),
                    )
                        .clicked()
                    {
                        app.try_connect();
                    }
                });
            });
            return;
        }

        // Update or create texture
        app.image_texture_id = app.update_or_create_texture(ctx);

        if let Some(texture_id) = app.image_texture_id {
            // Calculate available space and size for the image
            let available_size = ui.available_size();
            let image_aspect_ratio = app.frame_width as f32 / app.frame_height as f32;
            let panel_aspect_ratio = available_size.x / available_size.y;

            // Initial sizing without zoom
            let base_display_size = if image_aspect_ratio > panel_aspect_ratio {
                // Width constrained
                Vec2::new(available_size.x, available_size.x / image_aspect_ratio)
            } else {
                // Height constrained
                Vec2::new(available_size.y * image_aspect_ratio, available_size.y)
            };

            // Apply animated zoom
            let display_size = Vec2::new(
                base_display_size.x * app.animation.zoom_anim,
                base_display_size.y * app.animation.zoom_anim,
            );

            // Get the response for interaction
            let image_response = ui
                .centered_and_justified(|ui| ui.image((texture_id, display_size)))
                .inner;

            // Add subtle vignette effect around the image (medical focused)
            let vignette_size = 15.0; // Controls the size of the vignette
            let vignette_opacity = 100; // 0-255
            let vignette_color = Color32::from_rgba_premultiplied(10, 15, 30, vignette_opacity);

            for i in 0..vignette_size as usize {
                let alpha = ((i as f32 / vignette_size) * vignette_opacity as f32) as u8;
                let color = Color32::from_rgba_premultiplied(
                    vignette_color.r(),
                    vignette_color.g(),
                    vignette_color.b(),
                    vignette_opacity - alpha
                );

                ui.painter().rect_stroke(
                    image_response.rect.expand(i as f32),
                    0.0,
                    Stroke::new(1.0, color),
                    StrokeKind::Middle
                );
            }

            // Image container for glow/shadow effects
            let expanded_rect = image_response.rect.expand(2.0);

            // Draw a subtle glow around the image
            ui.painter().rect_stroke(
                expanded_rect,
                0.0,
                Stroke::new(
                    1.0,
                    Color32::from_rgba_premultiplied(
                        app.colors.accent.r(),
                        app.colors.accent.g(),
                        app.colors.accent.b(),
                        (40.0 + app.animation.pulse_value * 20.0) as u8,
                    ),
                ),
                StrokeKind::Middle,
            );

            // Draw rulers if enabled with animation
            if app.show_rulers {
                draw_rulers(app, ui, image_response.rect);
            }

            // Draw grid if enabled with animation
            if app.show_grid {
                draw_animated_grid(app, ui, image_response.rect);
            }

            // Handle interactions based on selected tool
            if image_response.hovered() {
                let pointer_pos = ui.input(|i| i.pointer.hover_pos());

                if let Some(pos) = pointer_pos {
                    // Handle different tools
                    match app.selected_tool {
                        tools::Tool::ROI => {
                            tools::handle_roi_tool(app, ui, &image_response, pos)
                        }
                        tools::Tool::Measure => {
                            tools::handle_measure_tool(app, ui, &image_response, pos)
                        }
                        tools::Tool::Annotate => {
                            tools::handle_annotate_tool(app, ui, &image_response, pos)
                        }
                        tools::Tool::Zoom => {
                            tools::handle_zoom_tool(app, ui, &image_response, pos)
                        }
                        tools::Tool::Pan => {
                            tools::handle_pan_tool(app, ui, &image_response, pos)
                        }
                        // Other tools handled separately
                        _ => {}
                    }
                }
            }

            // Draw existing measurements with animations
            for measurement in &app.measurements {
                // Animation progress based on creation time
                let time_since_creation = Instant::now()
                    .duration_since(measurement.creation_time)
                    .as_secs_f32();
                let progress = (time_since_creation * 4.0).min(1.0);

                // Animate line drawing
                let start = measurement.start;
                let end = Pos2::new(
                    start.x + (measurement.end.x - start.x) * progress,
                    start.y + (measurement.end.y - start.y) * progress,
                );

                // Enhanced line appearance with glow effect
                let stroke_width = 2.0;
                let stroke_color = app.colors.accent;

                // Draw measurement line with glow
                let glow_color = Color32::from_rgba_premultiplied(
                    stroke_color.r(),
                    stroke_color.g(),
                    stroke_color.b(),
                    (80.0 + 40.0 * app.animation.pulse_value) as u8,
                );

                // Glow effect
                ui.painter()
                    .line_segment([start, end], Stroke::new(stroke_width + 2.0, glow_color));

                // Main line
                ui.painter()
                    .line_segment([start, end], Stroke::new(stroke_width, stroke_color));

                // Only draw the label if animation is complete
                if progress >= 1.0 {
                    // Draw measurement label
                    let mid_point =
                        Pos2::new((start.x + end.x) / 2.0, (start.y + end.y) / 2.0 - 15.0);

                    // Add a background for the text with glass effect
                    let text_size = egui::Vec2::new(60.0, 20.0);
                    let text_rect = Rect::from_center_size(mid_point, text_size);

                    // Use glass_panel instead of direct drawing
                    solid_panel(ui, text_rect, 6.0, app.colors.panel_bg);

                    // Calculate distance in pixels
                    let dx = end.x - start.x;
                    let dy = end.y - start.y;
                    let distance = (dx * dx + dy * dy).sqrt();

                    // Text shadow for better readability
                    ui.painter().text(
                        mid_point + Vec2::new(1.0, 1.0),
                        Align2::CENTER_CENTER,
                        format!("{}: {:.1}px", measurement.label, distance),
                        FontId::proportional(12.0),
                        Color32::from_rgba_premultiplied(0, 0, 0, 160),
                    );

                    ui.painter().text(
                        mid_point,
                        Align2::CENTER_CENTER,
                        format!("{}: {:.1}px", measurement.label, distance),
                        FontId::proportional(12.0),
                        Color32::WHITE,
                    );
                }
            }

            // Draw annotations
            tools::annotate::draw_annotations(app, ui);

            // Draw ROI
            tools::roi::draw_roi(app, ui);

            // Draw HUD if enabled
            //draw_hud(app, ui, image_response.rect);
            
        } else {
            // No valid frame yet - show animated waiting message
            ui.centered_and_justified(|ui| {
                let text_color = match app.theme {
                    crate::ui::theme::Theme::Dark
                    | crate::ui::theme::Theme::MedicalBlue
                    | crate::ui::theme::Theme::NightMode => Color32::from_rgb(200, 200, 210),
                    crate::ui::theme::Theme::Light => Color32::from_rgb(80, 80, 100),
                    crate::ui::theme::Theme::HighContrast => Color32::WHITE,
                };

                ui.vertical_centered(|ui| {
                    ui.add_space(50.0);

                    // Animated waiting icon
                    let frames_text = "🎬";
                    let icon_size = 36.0 + app.animation.pulse_value * 4.0;

                    ui.painter().text(
                        ui.next_widget_position() + Vec2::new(0.0, icon_size / 2.0),
                        Align2::CENTER_CENTER,
                        frames_text,
                        FontId::new(icon_size, egui::FontFamily::Proportional),
                        crate::ui::theme::lerp_color(
                            text_color,
                            app.colors.accent,
                            app.animation.pulse_value * 0.5,
                        ),
                    );

                    ui.add_space(50.0);

                    // Text with subtle animation
                    let (title_text, subtitle_text) =
                        if app.connection_status.starts_with("Connected") {
                            (
                                "Waiting for Frames...",
                                "Connected to device, awaiting video stream",
                            )
                        } else {
                            ("No Connection", "Please check the ultrasound device status")
                        };

                    // Animate the text appearance
                    let text_offset = (1.0 - app.animation.startup_progress) * 20.0;
                    let text_alpha = ((app.animation.startup_progress * 0.6 + 0.4) * 255.0) as u8;

                    ui.painter().text(
                        ui.next_widget_position() + Vec2::new(0.0, text_offset),
                        Align2::CENTER_TOP,
                        title_text,
                        FontId::new(24.0, egui::FontFamily::Proportional),
                        Color32::from_rgba_premultiplied(
                            text_color.r(),
                            text_color.g(),
                            text_color.b(),
                            text_alpha,
                        ),
                    );

                    ui.add_space(30.0);

                    // Subtitle with a delay
                    let subtitle_alpha =
                        ((app.animation.startup_progress - 0.3).max(0.0) / 0.7 * 255.0) as u8;

                    ui.painter().text(
                        ui.next_widget_position() + Vec2::new(0.0, text_offset * 0.5),
                        Align2::CENTER_TOP,
                        subtitle_text,
                        FontId::new(16.0, egui::FontFamily::Proportional),
                        Color32::from_rgba_premultiplied(
                            text_color.r(),
                            text_color.g(),
                            text_color.b(),
                            subtitle_alpha,
                        ),
                    );
                });
            });
        }
    });
}