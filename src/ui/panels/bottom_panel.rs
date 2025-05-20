// ui/panels/bottom_panel.rs - Bottom control bar implementation

use crate::app::EchoViewer;
use eframe::egui;
use egui::epaint::CornerRadiusF32;
use egui::StrokeKind::Inside;
use egui::*;

// Draw the bottom panel with controls and status
pub fn draw(app: &mut EchoViewer, ctx: &egui::Context) {
    egui::TopBottomPanel::bottom("bottom_panel")
        .height_range(40.0..=40.0)
        .show(ctx, |ui| {
            // Panel background with gradient - IMPROVED IMPLEMENTATION
            let rect = ui.max_rect();
            let top_color = app.colors.panel_bg;
            let bottom_color = app.colors.primary;

            // Draw gradient background using fewer steps for smoother appearance
            let steps = 20; // Reduced steps
            for i in 0..steps {
                let t = 1.0 - i as f32 / (steps as f32 - 1.0); // Reversed gradient
                let color = crate::ui::theme::lerp_color(top_color, bottom_color, t);

                let y_start = rect.min.y + (rect.height() * (i as f32 / steps as f32));
                let y_end = rect.min.y + (rect.height() * ((i + 1) as f32 / steps as f32));

                ui.painter().rect_filled(
                    Rect::from_min_max(
                        Pos2::new(rect.min.x, y_start),
                        Pos2::new(rect.max.x, y_end)
                    ),
                    CornerRadiusF32::same(0.),
                    color
                );
            }

            // Top shadow for 3D effect
            ui.painter().line_segment(
                [Pos2::new(rect.min.x, rect.min.y),
                    Pos2::new(rect.max.x, rect.min.y)],
                Stroke::new(1.0, Color32::from_rgba_premultiplied(0, 0, 0, 40))
            );

            // Main content
            ui.horizontal(|ui| {
                ui.add_space(8.0);

                // Zoom controls with animation
                ui.label(RichText::new("Zoom:").color(app.colors.text));

                if ui.add(egui::Button::new("-").corner_radius(20.0)).clicked() {
                    app.animation.target_zoom = (app.animation.target_zoom - 0.1).max(0.5);
                }

                // Zoom level indicator with animation
                let zoom_text = format!("{:.1}×", app.animation.zoom_anim);

                // Draw glass-effect panel
                let zoom_label_size = ui.fonts(|f| f.layout_no_wrap(
                    zoom_text.clone(),
                    FontId::proportional(14.0),
                    Color32::WHITE
                )).rect.size();

                let zoom_rect = Rect::from_min_size(
                    ui.cursor().min - Vec2::new(0.0, 2.0),
                    zoom_label_size + Vec2::new(10.0, 4.0)
                );

                // Glass background
                ui.painter().rect_filled(
                    zoom_rect,
                    CornerRadiusF32::same(4.0),
                    Color32::from_rgba_premultiplied(40, 60, 90, 180)
                );

                // Glass top highlight
                ui.painter().rect_stroke(Rect::from_min_max(
                        zoom_rect.min,
                        Pos2::new(zoom_rect.max.x, zoom_rect.min.y + zoom_rect.height() * 0.5)
                    ), CornerRadiusF32 {
                        nw: 4.0,
                        ne: 4.0,
                        sw: 0.0,
                        se: 0.0,
                    }, Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 255, 255, 40)), Inside);

                // Text with shadow
                ui.painter().text(
                    zoom_rect.center() + Vec2::new(0.0, 1.0),
                    Align2::CENTER_CENTER,
                    &zoom_text,
                    FontId::proportional(14.0),
                    Color32::from_rgba_premultiplied(0, 0, 0, 100)
                );

                ui.painter().text(
                    zoom_rect.center(),
                    Align2::CENTER_CENTER,
                    &zoom_text,
                    FontId::proportional(14.0),
                    Color32::WHITE
                );

                // Adjust spacing based on text width
                ui.add_space(zoom_label_size.x + 14.0);

                if ui.add(egui::Button::new("+").corner_radius(20.0)).clicked() {
                    app.animation.target_zoom = (app.animation.target_zoom + 0.1).min(4.0);
                }

                ui.separator();

                // Frame information with animated highlight
                if let Some(header) = app.frame_header {
                    // Frame counter with subtle animation
                    let frame_text = format!("Frame: {}", header.frame_id);
                    let frame_color = if header.frame_id % 30 < 5 {
                        // Briefly highlight every 30 frames
                        crate::ui::theme::lerp_color(app.colors.text, app.colors.accent, 0.3)
                    } else {
                        app.colors.text
                    };

                    ui.label(RichText::new(frame_text).color(frame_color));
                    ui.separator();

                    // FPS with color coding
                    let fps_color = if app.fps >= 59.0 {
                        app.colors.success // Green for 60+ FPS
                    } else if app.fps >= 29.0 {
                        app.colors.warning // Yellow for 30-59 FPS
                    } else {
                        app.colors.error // Red for <30 FPS
                    };

                    ui.label(RichText::new(format!("FPS: {:.1}", app.fps)).color(fps_color));
                    ui.separator();

                    // Latency with color coding
                    let latency_color = if app.latency_ms <= 16.0 {
                        app.colors.success // Green for <16ms (60+ FPS)
                    } else if app.latency_ms <= 33.0 {
                        app.colors.warning // Yellow for 16-33ms (30-60 FPS)
                    } else {
                        app.colors.error // Red for >33ms (<30 FPS)
                    };

                    ui.label(RichText::new(format!("Latency: {:.1} ms", app.latency_ms)).color(latency_color));
                }

                // Right-aligned controls
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(8.0);

                    // Low latency toggle with enhanced style
                    let old_catchup = app.catch_up;

                    // Style the checkbox as a toggle
                    let toggle_size = Vec2::new(40.0, 20.0);
                    let (toggle_rect, toggle_response) = ui.allocate_exact_size(toggle_size, Sense::click());

                    if toggle_response.clicked() {
                        app.catch_up = !app.catch_up;
                    }

                    if ui.is_rect_visible(toggle_rect) {
                        // Draw track
                        let corner = toggle_size.y / 2.0;
                        let track_color = if app.catch_up {
                            crate::ui::theme::lerp_color(app.colors.button_active, app.colors.accent, app.animation.pulse_value * 0.3)
                        } else {
                            Color32::from_rgba_premultiplied(60, 70, 90, 180)
                        };

                        ui.painter().rect_filled(
                            toggle_rect,
                            CornerRadiusF32::same(corner),
                            track_color
                        );

                        // Draw handle
                        let circle_size = toggle_size.y * 0.8;
                        let mut circle_x = if app.catch_up {
                            toggle_rect.right() - circle_size - 2.0
                        } else {
                            toggle_rect.left() + 2.0
                        };

                        // Small animation when toggled
                        if old_catchup != app.catch_up {
                            // Just animate with a slight offset
                            circle_x += if app.catch_up { -3.0 } else { 3.0 };
                        }

                        let circle_pos = Pos2::new(
                            circle_x,
                            toggle_rect.center().y
                        );

                        // Glow for the handle
                        if app.catch_up {
                            ui.painter().circle_filled(
                                circle_pos,
                                circle_size * 0.7,
                                Color32::from_rgba_premultiplied(
                                    app.colors.accent.r(),
                                    app.colors.accent.g(),
                                    app.colors.accent.b(),
                                    (100.0 * app.animation.pulse_value) as u8
                                )
                            );
                        }

                        // Handle
                        ui.painter().circle_filled(
                            circle_pos,
                            circle_size / 2.0,
                            if app.catch_up { app.colors.accent } else { Color32::WHITE }
                        );

                        // Label
                        ui.label("Low Latency Mode");
                    }

                    ui.separator();

                    // Mode indicator with glass effect
                    let mode_rect = Rect::from_min_size(
                        ui.cursor().min - Vec2::new(90.0, 0.0),
                        Vec2::new(85.0, 26.0)
                    );

                    // Glass background
                    ui.painter().rect_filled(
                        mode_rect,
                        CornerRadiusF32::same(6.0),
                        Color32::from_rgba_premultiplied(40, 60, 90, 180)
                    );

                    // Glass highlight
                    ui.painter().rect_stroke(Rect::from_min_max(
                            mode_rect.min,
                            Pos2::new(mode_rect.max.x, mode_rect.min.y + mode_rect.height() * 0.5)
                        ), CornerRadiusF32 {
                            nw: 6.0,
                            ne: 6.0,
                            sw: 0.0,
                            se: 0.0,
                        }, Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 255, 255, 40)),Inside);

                    // Mode text with shadow
                    ui.painter().text(
                        mode_rect.center() + Vec2::new(0.0, 1.0),
                        Align2::CENTER_CENTER,
                        "Mode: B-Mode",
                        FontId::proportional(13.0),
                        Color32::from_rgba_premultiplied(0, 0, 0, 120)
                    );

                    ui.painter().text(
                        mode_rect.center(),
                        Align2::CENTER_CENTER,
                        "Mode: B-Mode",
                        FontId::proportional(13.0),
                        Color32::WHITE
                    );

                    ui.add_space(100.0);

                    ui.separator();

                    // Depth indicator
                    let depth_rect = Rect::from_min_size(
                        ui.cursor().min - Vec2::new(80.0, 0.0),
                        Vec2::new(75.0, 26.0)
                    );

                    // Glass background
                    ui.painter().rect_filled(
                        depth_rect,
                        CornerRadiusF32::same(6.0),
                        Color32::from_rgba_premultiplied(40, 60, 90, 180)
                    );

                    // Glass highlight
                    ui.painter().rect_stroke(Rect::from_min_max(
                            depth_rect.min,
                            Pos2::new(depth_rect.max.x, depth_rect.min.y + depth_rect.height() * 0.5)
                        ), CornerRadiusF32 {
                            nw: 6.0,
                            ne: 6.0,
                            sw: 0.0,
                            se: 0.0,
                        }, Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 255, 255, 40)), Inside);

                    // Depth text with shadow
                    ui.painter().text(
                        depth_rect.center() + Vec2::new(0.0, 1.0),
                        Align2::CENTER_CENTER,
                        "Depth: 10 cm",
                        FontId::proportional(13.0),
                        Color32::from_rgba_premultiplied(0, 0, 0, 120)
                    );

                    ui.painter().text(
                        depth_rect.center(),
                        Align2::CENTER_CENTER,
                        "Depth: 10 cm",
                        FontId::proportional(13.0),
                        Color32::WHITE
                    );

                    ui.add_space(90.0);
                });
            });
        });
}