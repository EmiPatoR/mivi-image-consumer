// ui/widgets.rs - Custom UI widgets for the application

use eframe::egui::{self, *};
use egui::epaint::CornerRadiusF32;
use egui::StrokeKind::Outside;
use crate::ui::theme::lerp_color;

// A nice pulsing button with hover effect
pub fn pulse_button(ui: &mut Ui, text: &str, size: Vec2, pulse_value: f32, hover: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);
        let rounding = 4.0;

        // Base color with pulse animation
        let mut base_color = Color32::from_rgb(44, 63, 102);
        if hover {
            base_color = Color32::from_rgb(58, 117, 195);
        }

        // Glow effect based on pulse value
        let glow_alpha = (pulse_value * 0.5 + 0.5) * 255.0;
        let glow_color = if hover {
            Color32::from_rgba_premultiplied(100, 170, 255, glow_alpha as u8)
        } else {
            Color32::from_rgba_premultiplied(80, 140, 220, glow_alpha as u8)
        };

        // Shadow
        let shadow_rect = rect.expand(1.0);
        ui.painter().rect_filled(
            shadow_rect,
            rounding,
            Color32::from_rgba_premultiplied(10, 15, 30, 100)
        );

        // Button background
        ui.painter().rect_filled(
            rect,
            rounding,
            base_color
        );

        // Glow effect
        if hover || pulse_value > 0.2 {
            ui.painter().rect_stroke(rect, rounding, Stroke::new(1.5, glow_color), Outside);
        }

        // Top highlight
        let highlight_rect = Rect::from_min_size(
            rect.min,
            Vec2::new(rect.width(), rect.height() * 0.2)
        );

        ui.painter().rect_filled(
            highlight_rect,
            CornerRadiusF32 {
                nw: rounding,
                ne: rounding,
                sw: 0.0,
                se: 0.0,
            },
            Color32::from_rgba_premultiplied(255, 255, 255, 30)
        );

        // Bottom shadow
        let shadow_bottom = Rect::from_min_size(
            Pos2::new(rect.min.x, rect.min.y + rect.height() * 0.8),
            Vec2::new(rect.width(), rect.height() * 0.2)
        );

        ui.painter().rect_filled(
            shadow_bottom,
            CornerRadiusF32 {
                nw: 0.0,
                ne: 0.0,
                sw: rounding,
                se: rounding,
            },
            Color32::from_rgba_premultiplied(0, 0, 0, 30)
        );

        // Text with shadow
        let font = FontId::proportional(14.0);
        let text_color = Color32::WHITE;

        // Text shadow
        ui.painter().text(
            rect.center() + Vec2::new(1.0, 1.0),
            Align2::CENTER_CENTER,
            text,
            font.clone(),
            Color32::from_rgba_premultiplied(0, 0, 0, 120)
        );

        // Text
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            text,
            font,
            text_color
        );
    }

    response
}

// A fancy slider that looks medical-grade
pub fn medical_slider(ui: &mut Ui, value: &mut f32, range: std::ops::RangeInclusive<f32>,
                      text: &str, anim_value: f32) -> Response {
    ui.horizontal(|ui| {
        // Label
        ui.label(text);

        ui.add_space(5.0);
        ui.add(egui::Slider::new(value, range)
            .show_value(true)
            .trailing_fill(true)
            .handle_shape(egui::style::HandleShape::Circle)
            .text(""))
    }).inner
}

// A professional looking tool button
pub fn tool_button(ui: &mut Ui, text: &str, icon: &str, selected: bool,
                   hover: bool, animation_progress: f32) -> Response {
    let height = 32.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::click());

    if ui.is_rect_visible(rect) {
        let rounding = 4.0;

        // Background
        let bg_color = if selected {
            Color32::from_rgb(58, 117, 195)
        } else if hover {
            Color32::from_rgb(44, 63, 102)
        } else {
            Color32::from_rgba_premultiplied(40, 50, 80, 180)
        };

        ui.painter().rect_filled(
            rect,
            rounding,
            bg_color
        );

        // Selection animation - left bar that grows when selected
        if selected || animation_progress > 0.0 {
            let progress = if selected { animation_progress } else { 1.0 - animation_progress };
            let indicator_width = 4.0;
            let indicator_height = height * progress;

            ui.painter().rect_filled(
                Rect::from_min_size(
                    Pos2::new(rect.min.x, rect.min.y + (height - indicator_height) / 2.0),
                    Vec2::new(indicator_width, indicator_height)
                ),
                CornerRadiusF32 {
                    nw: 2.0,
                    ne: 0.0,
                    sw: 2.0,
                    se: 0.0,
                },
                Color32::from_rgb(66, 185, 196)
            );
        }

        // Icon and text
        ui.painter().text(
            Pos2::new(rect.min.x + 20.0, rect.center().y),
            Align2::LEFT_CENTER,
            icon,
            FontId::proportional(18.0),
            if selected { Color32::WHITE } else { Color32::from_rgb(200, 210, 220) }
        );

        ui.painter().text(
            Pos2::new(rect.min.x + 45.0, rect.center().y),
            Align2::LEFT_CENTER,
            text,
            FontId::proportional(14.0),
            if selected { Color32::WHITE } else { Color32::from_rgb(200, 210, 220) }
        );

        // Top highlight for 3D effect
        if selected {
            let highlight_rect = Rect::from_min_size(
                rect.min,
                Vec2::new(rect.width(), rect.height() * 0.3)
            );

            ui.painter().rect_filled(
                highlight_rect,
                CornerRadiusF32 {
                    nw: rounding,
                    ne: rounding,
                    sw: 0.0,
                    se: 0.0,
                },
                Color32::from_rgba_premultiplied(255, 255, 255, 20)
            );
        }
    }

    response
}

// Professional looking panel header
pub fn panel_header(ui: &mut Ui, title: &str) {
    let header_height = 28.0;
    let rect = ui.available_rect_before_wrap();//.with_height(header_height);

    // Background with gradient
    let top_color = Color32::from_rgb(48, 60, 90);
    let bottom_color = Color32::from_rgb(35, 45, 70);

    ui.painter().rect_filled(
        rect,
        0.0,
        top_color
    );

    // Subtle gradient
    for i in 0..header_height as usize {
        let t = i as f32 / header_height;
        let color = lerp_color(top_color, bottom_color, t);
        let y = rect.min.y + i as f32;

        ui.painter().line_segment(
            [Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)],
            Stroke::new(1.0, color)
        );
    }

    // Add title text with slight emboss effect
    ui.painter().text(
        Pos2::new(rect.min.x + 10.0, rect.center().y + 1.0),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(15.0),
        Color32::from_rgba_premultiplied(0, 0, 0, 100)
    );

    ui.painter().text(
        Pos2::new(rect.min.x + 10.0, rect.center().y),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(15.0),
        Color32::WHITE
    );

    // Bottom highlight
    ui.painter().line_segment(
        [Pos2::new(rect.min.x, rect.max.y - 1.0),
            Pos2::new(rect.max.x, rect.max.y - 1.0)],
        Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 255, 255, 20))
    );

    // Bottom shadow
    ui.painter().line_segment(
        [Pos2::new(rect.min.x, rect.max.y),
            Pos2::new(rect.max.x, rect.max.y)],
        Stroke::new(1.0, Color32::from_rgba_premultiplied(0, 0, 0, 80))
    );

    ui.add_space(header_height);
}