// ui/panels/info_panel.rs - Right information panel implementation

use eframe::egui;
use egui::*;
use std::time::Instant;
use crate::app::EchoViewer;
use crate::ui::widgets;

// Draw the right info panel with frame information and measurements
pub fn draw(app: &mut EchoViewer, ctx: &egui::Context) {
    egui::SidePanel::right("info_panel")
        .resizable(true)
        .default_width(250.0)
        .width_range(200.0..=400.0)
        .show(ctx, |ui| {
            // Animated info panel
            let panel_alpha = (app.panel_alpha * 255.0) as u8;

            // Draw panel header
            widgets::panel_header(ui, "Frame Information");
            ui.add_space(8.0);

            // Frame information with a professional layout
            egui::Grid::new("frame_info_grid")
                .num_columns(2)
                .spacing([10.0, 6.0])
                .striped(true)
                .show(ui, |ui| {
                    if let Some(header) = app.frame_header {
                        // Frame data with fade-in animation
                        let info_pairs = [
                            ("Resolution:", format!("{}×{}", header.width, header.height)),
                            ("Format:", app.format.clone()),
                            ("Frame ID:", format!("{}", header.frame_id)),
                            ("Sequence:", format!("{}", header.sequence_number)),
                            ("FPS:", format!("{:.1}", app.fps)),
                            ("Latency:", format!("{:.2} ms", app.latency_ms)),
                            ("Process Time:", format!("{:.2} ms", app.process_time_us as f64 / 1000.0)),
                            ("Texture Time:", format!("{:.2} ms", app.texture_time_us as f64 / 1000.0)),
                            ("Total Frames:", format!("{}", app.total_frames)),
                        ];

                        for (i, (label, value)) in info_pairs.iter().enumerate() {
                            // Animation delay based on index
                            let delay_factor = i as f32 * 0.1;
                            let appear_progress = (app.panel_alpha - delay_factor).max(0.0) / 0.9;
                            let alpha = (appear_progress * 255.0) as u8;

                            ui.label(RichText::new(*label).strong().color(Color32::from_rgba_premultiplied(
                                app.colors.text.r(),
                                app.colors.text.g(),
                                app.colors.text.b(),
                                alpha
                            )));

                            ui.label(RichText::new(value).color(Color32::from_rgba_premultiplied(
                                app.colors.text.r(),
                                app.colors.text.g(),
                                app.colors.text.b(),
                                alpha
                            )));
                            ui.end_row();
                        }
                    } else {
                        ui.label(RichText::new("No frame data available").color(
                            Color32::from_rgba_premultiplied(
                                app.colors.text.r(),
                                app.colors.text.g(),
                                app.colors.text.b(),
                                panel_alpha
                            )
                        ));
                        ui.end_row();
                    }
                });

            ui.add_space(20.0);

            // Measurements section with animations
            widgets::panel_header(ui, "Measurements");
            ui.add_space(8.0);

            if app.measurements.is_empty() {
                ui.label(RichText::new("No measurements recorded").color(
                    Color32::from_rgba_premultiplied(
                        app.colors.text_secondary.r(),
                        app.colors.text_secondary.g(),
                        app.colors.text_secondary.b(),
                        panel_alpha
                    )
                ));
            } else {
                egui::Grid::new("measurements_grid")
                    .num_columns(3)
                    .spacing([10.0, 6.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(RichText::new("Label").strong().color(
                            Color32::from_rgba_premultiplied(
                                app.colors.text.r(),
                                app.colors.text.g(),
                                app.colors.text.b(),
                                panel_alpha
                            )
                        ));

                        ui.label(RichText::new("Length").strong().color(
                            Color32::from_rgba_premultiplied(
                                app.colors.text.r(),
                                app.colors.text.g(),
                                app.colors.text.b(),
                                panel_alpha
                            )
                        ));

                        ui.label(RichText::new("Action").strong().color(
                            Color32::from_rgba_premultiplied(
                                app.colors.text.r(),
                                app.colors.text.g(),
                                app.colors.text.b(),
                                panel_alpha
                            )
                        ));
                        ui.end_row();

                        for (i, measurement) in app.measurements.iter().enumerate() {
                            // Calculate animation progress for each measurement
                            let creation_duration = Instant::now().duration_since(measurement.creation_time).as_secs_f32();
                            let appear_progress = (creation_duration * 3.0).min(1.0);

                            // Color based on animation
                            let color = if creation_duration < 0.5 {
                                crate::ui::theme::lerp_color(app.colors.accent, app.colors.text, creation_duration * 2.0)
                            } else {
                                app.colors.text
                            };

                            // Apply the color with panel fade-in
                            let display_color = Color32::from_rgba_premultiplied(
                                color.r(),
                                color.g(),
                                color.b(),
                                panel_alpha
                            );

                            ui.label(RichText::new(&measurement.label).color(display_color));

                            // Calculate pixel distance
                            let dx = measurement.end.x - measurement.start.x;
                            let dy = measurement.end.y - measurement.start.y;
                            let distance = (dx * dx + dy * dy).sqrt();
                            ui.label(RichText::new(format!("{:.1} px", distance)).color(display_color));

                            if ui.button("🗑").clicked() {
                                app.measurements.remove(i);
                                break;
                            }
                            ui.end_row();
                        }
                    });
            }

            ui.add_space(20.0);

            // Annotations section with animations
            widgets::panel_header(ui, "Annotations");
            ui.add_space(8.0);

            if app.annotations.is_empty() {
                ui.label(RichText::new("No annotations added").color(
                    Color32::from_rgba_premultiplied(
                        app.colors.text_secondary.r(),
                        app.colors.text_secondary.g(),
                        app.colors.text_secondary.b(),
                        panel_alpha
                    )
                ));
            } else {
                egui::Grid::new("annotations_grid")
                    .num_columns(3)
                    .spacing([10.0, 6.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(RichText::new("Text").strong().color(
                            Color32::from_rgba_premultiplied(
                                app.colors.text.r(),
                                app.colors.text.g(),
                                app.colors.text.b(),
                                panel_alpha
                            )
                        ));

                        ui.label(RichText::new("Position").strong().color(
                            Color32::from_rgba_premultiplied(
                                app.colors.text.r(),
                                app.colors.text.g(),
                                app.colors.text.b(),
                                panel_alpha
                            )
                        ));

                        ui.label(RichText::new("Action").strong().color(
                            Color32::from_rgba_premultiplied(
                                app.colors.text.r(),
                                app.colors.text.g(),
                                app.colors.text.b(),
                                panel_alpha
                            )
                        ));
                        ui.end_row();

                        for (i, annotation) in app.annotations.iter().enumerate() {
                            // Calculate animation progress based on creation time
                            let creation_duration = Instant::now().duration_since(annotation.creation_time).as_secs_f32();
                            let appear_progress = (creation_duration * 3.0).min(1.0);

                            // Color based on animation
                            let color = if creation_duration < 0.5 {
                                crate::ui::theme::lerp_color(app.colors.accent, app.colors.text, creation_duration * 2.0)
                            } else {
                                app.colors.text
                            };

                            // Apply the color with panel fade-in
                            let display_color = Color32::from_rgba_premultiplied(
                                color.r(),
                                color.g(),
                                color.b(),
                                panel_alpha
                            );

                            let text = if annotation.text.len() > 15 {
                                format!("{}...", &annotation.text[0..12])
                            } else {
                                annotation.text.clone()
                            };

                            ui.label(RichText::new(text).color(display_color));
                            ui.label(RichText::new(format!("({:.0},{:.0})",
                                                           annotation.position.x,
                                                           annotation.position.y))
                                .color(display_color));

                            if ui.button("🗑").clicked() {
                                app.annotations.remove(i);
                                break;
                            }
                            ui.end_row();
                        }
                    });
            }

            ui.add_space(ui.available_height() - 30.0);

            // Help text at bottom with animation
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                // Fade in from bottom
                let help_alpha = ((app.panel_alpha - 0.5) * 2.0).max(0.0);
                let help_color = Color32::from_rgba_premultiplied(
                    app.colors.text_secondary.r(),
                    app.colors.text_secondary.g(),
                    app.colors.text_secondary.b(),
                    (help_alpha * 255.0) as u8
                );

                ui.label(RichText::new("Use mouse wheel to zoom").size(10.0).color(help_color));
                ui.label(RichText::new("Drag to pan when zoomed").size(10.0).color(help_color));
            });
        });
}