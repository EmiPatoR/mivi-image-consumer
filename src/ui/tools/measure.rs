// ui/tools/measure.rs - Measurement tool implementation

use crate::app::EchoViewer;
use crate::ui::tools::Measurement;
use eframe::egui::*;
use egui::epaint::CornerRadiusF32;
use std::time::Instant;

// Measurement tool implementation with animations
pub fn handle_measure_tool(
    app: &mut EchoViewer,
    ui: &mut Ui,
    image_response: &Response,
    cursor_pos: Pos2,
) {
    // Static state for measurement in progress
    static mut MEASURING_ACTIVE: bool = false;
    static mut MEASURE_START: Option<Pos2> = None;

    unsafe {
        if ui.input(|i| i.pointer.primary_pressed()) {
            MEASURING_ACTIVE = true;
            MEASURE_START = Some(cursor_pos);
        }

        if MEASURING_ACTIVE {
            if let Some(start) = MEASURE_START {
                // Draw the animated measurement line
                for i in 0..3 {
                    let size = 3.0 - i as f32;
                    let alpha = 255 - i * 70;

                    ui.painter().line_segment(
                        [start, cursor_pos],
                        Stroke::new(
                            size,
                            Color32::from_rgba_premultiplied(
                                app.colors.accent.r(),
                                app.colors.accent.g(),
                                app.colors.accent.b(),
                                alpha,
                            ),
                        ),
                    );
                }

                // Show distance while dragging with glass effect
                let dx = cursor_pos.x - start.x;
                let dy = cursor_pos.y - start.y;
                let distance = (dx * dx + dy * dy).sqrt();

                let mid_point = Pos2::new(
                    (start.x + cursor_pos.x) / 2.0,
                    (start.y + cursor_pos.y) / 2.0 - 15.0,
                );

                // Glassmorphism background
                let text = format!("{:.1} px", distance);
                let text_size = ui
                    .fonts(|f| {
                        f.layout_no_wrap(text.clone(), FontId::proportional(12.0), Color32::WHITE)
                    })
                    .rect
                    .size();

                let text_rect =
                    Rect::from_center_size(mid_point, text_size + egui::vec2(10.0, 6.0));

                // Glass background
                ui.painter().rect_filled(
                    text_rect,
                    CornerRadiusF32::same(6.0),
                    Color32::from_rgba_premultiplied(20, 30, 50, 220),
                );

                // Glass highlight
                ui.painter().rect_stroke(
                    Rect::from_min_max(
                        text_rect.min,
                        Pos2::new(text_rect.max.x, text_rect.min.y + text_rect.height() * 0.4),
                    ),
                    CornerRadiusF32 {
                        nw: 6.0,
                        ne: 6.0,
                        sw: 0.0,
                        se: 0.0,
                    },
                    Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 255, 255, 40)),
                    StrokeKind::Middle,
                );

                // Text shadow
                ui.painter().text(
                    mid_point + Vec2::new(1.0, 1.0),
                    Align2::CENTER_CENTER,
                    &text,
                    FontId::proportional(12.0),
                    Color32::from_rgba_premultiplied(0, 0, 0, 180),
                );

                // Text
                ui.painter().text(
                    mid_point,
                    Align2::CENTER_CENTER,
                    text,
                    FontId::proportional(12.0),
                    Color32::WHITE,
                );

                // Draw endpoints
                ui.painter().circle_filled(start, 4.0, app.colors.accent);

                ui.painter()
                    .circle_filled(cursor_pos, 4.0, app.colors.accent);
            }

            if ui.input(|i| i.pointer.primary_released()) {
                if let Some(start) = MEASURE_START {
                    // Finalize measurement
                    let dx = cursor_pos.x - start.x;
                    let dy = cursor_pos.y - start.y;
                    let distance = (dx * dx + dy * dy).sqrt();

                    // Only add if it's a meaningful measurement (not just a click)
                    if distance > 5.0 {
                        // Generate a default label
                        let label = format!("M{}", app.measurements.len() + 1);

                        app.measurements.push(Measurement {
                            start,
                            end: cursor_pos,
                            label,
                            creation_time: Instant::now(),
                            animated_progress: 0.0,
                        });
                    }
                }

                MEASURING_ACTIVE = false;
                MEASURE_START = None;
            }
        }
    }
}
