// ui/tools/annotate.rs - Annotation tool implementation

use crate::app::EchoViewer;
use crate::ui::tools::Annotation;
use eframe::egui::*;
use eframe::epaint::StrokeKind::Middle;
use egui::epaint::CornerRadiusF32;
use std::time::Instant;

// Annotation tool implementation with animations
pub fn handle_annotate_tool(
    app: &mut EchoViewer,
    ui: &mut Ui,
    image_response: &Response,
    cursor_pos: Pos2,
) {
    // Annotation tool implementation with animations
    if ui.input(|i| i.pointer.primary_clicked()) {
        if !app.annotation_text.is_empty() {
            app.annotations.push(Annotation {
                position: cursor_pos,
                text: app.annotation_text.clone(),
                creation_time: Instant::now(),
                animated_progress: 0.0,
            });

            // Clear the text input after adding
            app.annotation_text.clear();
        } else {
            // If no text is entered, show a small animated popup
            let text_pos = cursor_pos + egui::vec2(10.0, 10.0);
            let text = "Enter annotation text in sidebar";

            // Glass-effect popup
            let text_size = ui
                .fonts(|f| {
                    f.layout_no_wrap(
                        text.parse().unwrap(),
                        FontId::proportional(12.0),
                        Color32::WHITE,
                    )
                })
                .rect
                .size();

            let text_rect = Rect::from_min_size(text_pos, text_size + egui::vec2(12.0, 8.0));

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
                Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 255, 255, 30)),
                StrokeKind::Middle,
            );

            // Text shadow
            ui.painter().text(
                text_pos + egui::vec2(7.0, 5.0),
                egui::Align2::LEFT_TOP,
                text,
                FontId::proportional(12.0),
                Color32::from_rgba_premultiplied(0, 0, 0, 180),
            );

            // Text
            ui.painter().text(
                text_pos + egui::vec2(6.0, 4.0),
                egui::Align2::LEFT_TOP,
                text,
                FontId::proportional(12.0),
                Color32::WHITE,
            );

            // Draw a pointer to the annotation position
            ui.painter().line_segment(
                [cursor_pos, text_pos - Vec2::new(5.0, 0.0)],
                Stroke::new(1.5, app.colors.accent),
            );

            // Circle at cursor position
            ui.painter()
                .circle_filled(cursor_pos, 4.0, app.colors.accent);
        }
    }

    // Show preview of annotation
    if !app.annotation_text.is_empty() && ui.input(|i| i.pointer.hover_pos()).is_some() {
        let hover_pos = ui.input(|i| i.pointer.hover_pos()).unwrap();

        // Measure text
        let text_size = ui
            .fonts(|f| {
                f.layout_no_wrap(
                    app.annotation_text.clone(),
                    FontId::proportional(12.0),
                    Color32::WHITE,
                )
            })
            .rect
            .size();

        // Show preview of annotation at cursor
        let text_rect = Rect::from_min_size(
            hover_pos + Vec2::new(15.0, 0.0),
            text_size + Vec2::new(14.0, 8.0),
        );

        // Glass background with pulsing animation
        let alpha = (160.0 + app.animation.pulse_value * 40.0) as u8;

        ui.painter().rect_filled(
            text_rect,
            CornerRadiusF32::same(6.0),
            Color32::from_rgba_premultiplied(40, 60, 120, alpha),
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
            text_rect.min + Vec2::new(8.0, 5.0),
            Align2::LEFT_TOP,
            &app.annotation_text,
            FontId::proportional(12.0),
            Color32::from_rgba_premultiplied(0, 0, 0, alpha),
        );

        // Actual text
        ui.painter().text(
            text_rect.min + Vec2::new(7.0, 4.0),
            Align2::LEFT_TOP,
            &app.annotation_text,
            FontId::proportional(12.0),
            Color32::from_rgba_premultiplied(255, 255, 255, alpha),
        );

        // Connecting line
        ui.painter().line_segment(
            [
                hover_pos,
                text_rect.min + Vec2::new(-5.0, text_rect.height() / 2.0),
            ],
            Stroke::new(
                1.5,
                Color32::from_rgba_premultiplied(
                    app.colors.accent.r(),
                    app.colors.accent.g(),
                    app.colors.accent.b(),
                    alpha,
                ),
            ),
        );

        // Circle at cursor
        let circle_size = 3.0 + app.animation.pulse_value * 2.0;
        ui.painter().circle_filled(
            hover_pos,
            circle_size,
            Color32::from_rgba_premultiplied(
                app.colors.accent.r(),
                app.colors.accent.g(),
                app.colors.accent.b(),
                alpha,
            ),
        );
    }
}

// Draw existing annotations with animations
pub fn draw_annotations(app: &EchoViewer, ui: &Ui) {
    let now = Instant::now();

    for annotation in &app.annotations {
        // Calculate animation progress
        let time_since_creation = now.duration_since(annotation.creation_time).as_secs_f32();
        let progress = (time_since_creation * 3.0).min(1.0);

        if progress > 0.0 {
            // Measure text dimensions
            let text_size = ui
                .fonts(|f| {
                    f.layout_no_wrap(
                        annotation.text.clone(),
                        FontId::proportional(12.0),
                        Color32::WHITE,
                    )
                })
                .rect
                .size();

            // Animate size growth
            let size_scale = progress;
            let animated_size = Vec2::new(
                text_size.x * size_scale + 10.0,
                text_size.y * size_scale + 6.0,
            );

            let text_rect = Rect::from_min_size(annotation.position, animated_size);

            // Background with glass effect
            ui.painter().rect_filled(
                text_rect,
                CornerRadiusF32::same(5.0),
                Color32::from_rgba_premultiplied(40, 60, 120, (220.0 * progress) as u8),
            );

            // Glass highlight
            ui.painter().rect_stroke(
                Rect::from_min_max(
                    text_rect.min,
                    text_rect.min + Vec2::new(text_rect.width(), text_rect.height() * 0.3),
                ),
                CornerRadiusF32 {
                    nw: 5.0,
                    ne: 5.0,
                    sw: 0.0,
                    se: 0.0,
                },
                Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 255, 255, 40)),
                Middle,
            );

            // Draw connecting line from position to text for context
            let connector_start = annotation.position + Vec2::new(-5.0, text_rect.height() / 2.0);
            let connector_end = connector_start + Vec2::new(-10.0, 0.0);

            ui.painter().line_segment(
                [connector_start, connector_end],
                Stroke::new(
                    1.0,
                    Color32::from_rgba_premultiplied(
                        app.colors.accent.r(),
                        app.colors.accent.g(),
                        app.colors.accent.b(),
                        (200.0 * progress) as u8,
                    ),
                ),
            );

            // Circle at the end
            ui.painter()
                .circle_filled(connector_end, 3.0 * progress, app.colors.accent);

            // Draw text with fade-in
            let text_alpha = (255.0 * progress) as u8;

            // Text shadow
            ui.painter().text(
                annotation.position + Vec2::new(6.0, 4.0),
                Align2::LEFT_TOP,
                &annotation.text,
                FontId::proportional(12.0),
                Color32::from_rgba_premultiplied(0, 0, 0, (140.0 * progress) as u8),
            );

            // Main text
            ui.painter().text(
                annotation.position + Vec2::new(5.0, 3.0),
                Align2::LEFT_TOP,
                &annotation.text,
                FontId::proportional(12.0),
                Color32::from_rgba_premultiplied(255, 255, 255, text_alpha),
            );
        }
    }
}
