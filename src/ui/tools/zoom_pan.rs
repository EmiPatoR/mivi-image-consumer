// ui/tools/zoom_pan.rs - Zoom and pan tools implementation

use crate::app::EchoViewer;
use eframe::egui::*;
use egui::epaint::CornerRadiusF32;

// Zoom tool implementation with animations
pub fn handle_zoom_tool(
    app: &mut EchoViewer,
    ui: &mut Ui,
    image_response: &Response,
    cursor_pos: Pos2,
) {
    // Zoom with mouse wheel
    let wheel_delta = ui.input(|i| i.raw_scroll_delta.y);
    if wheel_delta != 0.0 {
        let zoom_delta = wheel_delta * 0.001;
        app.animation.target_zoom = (app.animation.target_zoom + zoom_delta).clamp(0.5, 4.0);
    }

    // Drag to pan
    if ui.input(|i| i.pointer.primary_down()) {
        // Only allow panning when zoomed in
        if app.animation.target_zoom > 1.0 {
            let delta = ui.input(|i| i.pointer.delta());
            app.drag_offset += delta;

            // Update image position based on drag
            // This would need to be implemented in the actual rendering logic
        }
    }

    // Draw zoom level indicator at cursor
    if ui.input(|i| i.pointer.hover_pos()).is_some() {
        let hover_pos = ui.input(|i| i.pointer.hover_pos()).unwrap();
        let zoom_text = format!("{:.1}×", app.animation.zoom_anim);

        // Only draw if zoomed
        if app.animation.zoom_anim != 1.0 {
            let text_size = ui
                .fonts(|f| {
                    f.layout_no_wrap(
                        zoom_text.clone(),
                        FontId::proportional(11.0),
                        Color32::WHITE,
                    )
                })
                .rect
                .size();

            let text_rect = Rect::from_center_size(
                hover_pos + Vec2::new(0.0, -20.0),
                text_size + Vec2::new(8.0, 6.0),
            );

            // Glass background
            ui.painter().rect_filled(
                text_rect,
                CornerRadiusF32::same(4.0),
                Color32::from_rgba_premultiplied(20, 30, 50, 180),
            );

            // Glass highlight
            ui.painter().rect_stroke(
                text_rect,
                CornerRadiusF32::same(4.0),
                Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 255, 255, 30)),
                StrokeKind::Middle,
            );

            // Text
            ui.painter().text(
                text_rect.center(),
                Align2::CENTER_CENTER,
                zoom_text,
                FontId::proportional(11.0),
                Color32::WHITE,
            );
        }
    }
}

// Pan tool implementation
pub fn handle_pan_tool(
    app: &mut EchoViewer,
    ui: &mut Ui,
    image_response: &Response,
    cursor_pos: Pos2,
) {
    // Similar to zoom tool panning
    if ui.input(|i| i.pointer.primary_down()) {
        let delta = ui.input(|i| i.pointer.delta());
        app.drag_offset += delta;

        // Draw panning indicator
        let arrow_size = 20.0;
        let arrow_color = app.colors.accent;

        // Draw a movement arrow
        if delta.x != 0.0 || delta.y != 0.0 {
            let arrow_dir = delta.normalized();
            let arrow_start = cursor_pos - arrow_dir * arrow_size * 0.5;
            let arrow_end = cursor_pos + arrow_dir * arrow_size * 0.5;

            // Main line
            ui.painter()
                .line_segment([arrow_start, arrow_end], Stroke::new(2.0, arrow_color));

            // Arrow head
            let head_size = 6.0;
            let perpendicular = Vec2::new(-arrow_dir.y, arrow_dir.x) * head_size;

            ui.painter().line_segment(
                [arrow_end, arrow_end - arrow_dir * head_size + perpendicular],
                Stroke::new(2.0, arrow_color),
            );

            ui.painter().line_segment(
                [arrow_end, arrow_end - arrow_dir * head_size - perpendicular],
                Stroke::new(2.0, arrow_color),
            );
        }
    }
}
