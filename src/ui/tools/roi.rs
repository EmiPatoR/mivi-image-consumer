// ui/tools/roi.rs - Region of Interest tool implementation

use crate::app::EchoViewer;
use eframe::egui::*;
use egui::epaint::CornerRadiusF32;

// ROI tool implementation with animations
pub fn handle_roi_tool(
    app: &mut EchoViewer,
    ui: &mut Ui,
    image_response: &Response,
    cursor_pos: Pos2,
) {
    // ROI tool implementation with animations
    if ui.input(|i| i.pointer.primary_pressed()) {
        app.roi_active = true;
        app.roi_start = Some(cursor_pos);
        app.roi_end = Some(cursor_pos);
    }

    if app.roi_active {
        if ui.input(|i| i.pointer.primary_down()) {
            app.roi_end = Some(cursor_pos);

            // Update region of interest rectangle
            if let (Some(start), Some(end)) = (app.roi_start, app.roi_end) {
                let min_x = start.x.min(end.x);
                let min_y = start.y.min(end.y);
                let max_x = start.x.max(end.x);
                let max_y = start.y.max(end.y);

                app.region_of_interest = Some(Rect::from_min_max(
                    Pos2::new(min_x, min_y),
                    Pos2::new(max_x, max_y),
                ));
            }
        }

        if ui.input(|i| i.pointer.primary_released()) {
            app.roi_active = false;
            // Keep the ROI rectangle
        }
    }

    // Preview the ROI while drawing with animation
    if app.roi_active {
        if let (Some(start), Some(end)) = (app.roi_start, app.roi_end) {
            let rect = Rect::from_two_pos(start, end);

            // Animated preview
            for i in 0..3 {
                let size = 3.0 - i as f32;
                let alpha = 255 - i * 70;

                ui.painter().rect_stroke(
                    rect.expand(i as f32 * 0.5),
                    CornerRadiusF32::same(0.0),
                    Stroke::new(
                        size,
                        Color32::from_rgba_premultiplied(
                            app.colors.accent.r(),
                            app.colors.accent.g(),
                            app.colors.accent.b(),
                            alpha,
                        ),
                    ),
                    StrokeKind::Middle,
                );
            }

            // Show dimensions while drawing
            let dx = (end.x - start.x).abs();
            let dy = (end.y - start.y).abs();

            let text = format!("{}×{}", dx as i32, dy as i32);
            let text_pos = cursor_pos + Vec2::new(10.0, 10.0);

            // Background
            let text_size = ui
                .fonts(|f| {
                    f.layout_no_wrap(text.clone(), FontId::proportional(12.0), Color32::WHITE)
                })
                .rect
                .size();

            let text_rect = Rect::from_min_size(text_pos, text_size + vec2(8.0, 4.0));

            ui.painter().rect_filled(
                text_rect,
                CornerRadiusF32::same(4.0),
                Color32::from_rgba_premultiplied(0, 0, 0, 200),
            );

            // Text
            ui.painter().text(
                text_pos + vec2(4.0, 2.0),
                egui::Align2::LEFT_TOP,
                text,
                FontId::proportional(12.0),
                Color32::WHITE,
            );
        }
    }
}

// Draw existing ROI with animated effects
pub fn draw_roi(app: &EchoViewer, ui: &Ui) {
    if let Some(roi) = app.region_of_interest {
        // Draw animated ROI rectangle with effects
        let roi_color = app.colors.accent;
        let pulse = (app.animation.pulse_value * 40.0) as u8;

        // Animated border
        ui.painter().rect_stroke(
            roi,
            CornerRadiusF32::same(0.0),
            Stroke::new(
                2.0,
                Color32::from_rgba_premultiplied(
                    roi_color.r(),
                    roi_color.g(),
                    roi_color.b(),
                    200 + pulse,
                ),
            ),
            StrokeKind::Middle,
        );

        // Add subtle fill for more visibility
        ui.painter().rect_filled(
            roi,
            CornerRadiusF32::same(0.0),
            Color32::from_rgba_premultiplied(roi_color.r(), roi_color.g(), roi_color.b(), 15),
        );

        // Corner effects for better visibility
        let corner_size = 10.0;

        // Top-left corner
        ui.painter().line_segment(
            [roi.min, Pos2::new(roi.min.x + corner_size, roi.min.y)],
            Stroke::new(3.0, roi_color),
        );
        ui.painter().line_segment(
            [roi.min, Pos2::new(roi.min.x, roi.min.y + corner_size)],
            Stroke::new(3.0, roi_color),
        );

        // Top-right corner
        ui.painter().line_segment(
            [
                Pos2::new(roi.max.x, roi.min.y),
                Pos2::new(roi.max.x - corner_size, roi.min.y),
            ],
            Stroke::new(3.0, roi_color),
        );
        ui.painter().line_segment(
            [
                Pos2::new(roi.max.x, roi.min.y),
                Pos2::new(roi.max.x, roi.min.y + corner_size),
            ],
            Stroke::new(3.0, roi_color),
        );

        // Bottom-left corner
        ui.painter().line_segment(
            [
                Pos2::new(roi.min.x, roi.max.y),
                Pos2::new(roi.min.x + corner_size, roi.max.y),
            ],
            Stroke::new(3.0, roi_color),
        );
        ui.painter().line_segment(
            [
                Pos2::new(roi.min.x, roi.max.y),
                Pos2::new(roi.min.x, roi.max.y - corner_size),
            ],
            Stroke::new(3.0, roi_color),
        );

        // Bottom-right corner
        ui.painter().line_segment(
            [roi.max, Pos2::new(roi.max.x - corner_size, roi.max.y)],
            Stroke::new(3.0, roi_color),
        );
        ui.painter().line_segment(
            [roi.max, Pos2::new(roi.max.x, roi.max.y - corner_size)],
            Stroke::new(3.0, roi_color),
        );

        // Draw ROI dimensions text
        let text = format!("ROI: {}×{}", roi.width() as i32, roi.height() as i32);
        let text_pos = Pos2::new(roi.min.x, roi.min.y - 25.0);

        // Glass-effect background for text
        let text_size = ui
            .fonts(|f| f.layout_no_wrap(text.clone(), FontId::proportional(12.0), Color32::WHITE))
            .rect
            .size();

        let text_rect = Rect::from_min_size(text_pos, text_size + egui::vec2(14.0, 8.0));

        // Glassmorphism effect
        ui.painter().rect_filled(
            text_rect,
            CornerRadiusF32::same(6.0),
            Color32::from_rgba_premultiplied(20, 30, 50, 220),
        );

        // Glass highlight
        ui.painter().rect_stroke(
            text_rect,
            CornerRadiusF32::same(6.0),
            Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 255, 255, 35)),
            StrokeKind::Middle,
        );

        // Text shadow
        ui.painter().text(
            text_pos + egui::vec2(8.0, 5.0),
            Align2::LEFT_TOP,
            &text,
            FontId::proportional(12.0),
            Color32::from_rgba_premultiplied(0, 0, 0, 180),
        );

        // Main text
        ui.painter().text(
            text_pos + vec2(7.0, 4.0),
            Align2::LEFT_TOP,
            text,
            FontId::proportional(12.0),
            Color32::WHITE,
        );
    }
}
