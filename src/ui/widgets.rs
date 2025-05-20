// ui/widgets.rs - Custom UI widgets for the application

use crate::ui::theme::lerp_color;
use eframe::egui::{self, *};
use egui::StrokeKind::{Inside, Middle, Outside};
use egui::epaint::CornerRadiusF32;

// Proper glass panel effect with correct alpha handling
pub fn glass_panel(ui: &Ui, rect: Rect, rounding: f32, alpha: u8) {
    // Base background color - need to use non-premultiplied alpha here
    let bg_color = Color32::from_rgba_unmultiplied(25, 35, 60, alpha);

    // Draw the main panel with correct alpha
    ui.painter().rect_filled(rect, rounding, bg_color);

    // Add inner highlight for glass effect (top half only)
    let highlight_rect = Rect::from_min_max(
        rect.min,
        Pos2::new(rect.max.x, rect.min.y + rect.height() * 0.5),
    );

    // Very subtle highlight - use non-premultiplied alpha
    let highlight_color = Color32::from_rgba_unmultiplied(255, 255, 255, alpha / 4);
    ui.painter()
        .rect_filled(highlight_rect, CornerRadius::same(0), highlight_color);

    // Add subtle border
    ui.painter().rect_stroke(
        rect,
        rounding,
        Stroke::new(
            1.0,
            Color32::from_rgba_unmultiplied(255, 255, 255, alpha / 3),
        ),
        Inside,
    );
}

pub fn solid_panel(ui: &mut Ui, rect: Rect, rounding: f32, color: Color32) {
    // Draw the main panel with solid color
    ui.painter().rect_filled(rect, rounding, color);

    // Add subtle border
    ui.painter().rect_stroke(
        rect,
        rounding,
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 40)),
        Inside,
    );
}

// A nice pulsing button with hover effect
pub fn pulse_button(
    ui: &mut Ui,
    text: &str,
    size: Vec2,
    pulse_value: f32,
    hover: bool,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    if ui.is_rect_visible(rect) {
        let rounding = 6.0;

        // Get button colors from theme
        let inactive_color = ui.style().visuals.widgets.inactive.bg_fill;
        let active_color = ui.style().visuals.widgets.active.bg_fill;
        let hovered_color = ui.style().visuals.widgets.hovered.bg_fill;

        // Base color for the button
        let base_color = if response.is_pointer_button_down_on() {
            active_color
        } else if hover {
            hovered_color
        } else {
            inactive_color
        };

        // Shadow
        let shadow_rect = rect.expand(1.0);
        ui.painter().rect_filled(
            shadow_rect,
            rounding,
            Color32::from_rgba_premultiplied(10, 15, 30, 100),
        );

        // Button background
        ui.painter().rect_filled(rect, rounding, base_color);

        // Pulse effect
        if hover || pulse_value > 0.1 {
            let pulse_color = ui.style().visuals.selection.bg_fill;
            let alpha = (pulse_value * 60.0) as u8;

            ui.painter().rect_stroke(
                rect,
                rounding,
                Stroke::new(
                    1.5,
                    Color32::from_rgba_premultiplied(
                        pulse_color.r(),
                        pulse_color.g(),
                        pulse_color.b(),
                        alpha,
                    ),
                ),
                Inside,
            );
        }

        // Top highlight
        let highlight_rect =
            Rect::from_min_size(rect.min, Vec2::new(rect.width(), rect.height() * 0.3));

        ui.painter().rect_filled(
            highlight_rect,
            CornerRadiusF32 {
                nw: rounding,
                ne: rounding,
                sw: 0.0,
                se: 0.0,
            },
            Color32::from_rgba_premultiplied(255, 255, 255, 30),
        );

        // Text with shadow
        let font = FontId::proportional(14.0);
        let text_color = ui.style().visuals.text_color();

        // Text shadow
        ui.painter().text(
            rect.center() + Vec2::new(1.0, 1.0),
            Align2::CENTER_CENTER,
            text,
            font.clone(),
            Color32::from_rgba_premultiplied(0, 0, 0, 120),
        );

        // Text
        ui.painter()
            .text(rect.center(), Align2::CENTER_CENTER, text, font, text_color);
    }

    response
}

// A fancy slider that looks medical-grade
pub fn medical_slider(ui: &mut Ui, value: &mut f32, range: std::ops::RangeInclusive<f32>,
                      text: &str, anim_value: f32) -> Response {
    ui.horizontal(|ui| {
        // Label with theme color
        let text_color = ui.style().visuals.text_color();

        ui.label(RichText::new(text).color(text_color));

        ui.add_space(5.0);
        ui.add(egui::Slider::new(value, range)
            .show_value(true)
            .trailing_fill(true)
            .handle_shape(egui::style::HandleShape::Circle)
            .text(""))
    }).inner
}

// A professional looking tool button
pub fn tool_button(
    ui: &mut Ui,
    text: &str,
    icon: &str,
    selected: bool,
    hover: bool,
    animation_progress: f32,
) -> Response {
    let height = 36.0;
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::click());

    if ui.is_rect_visible(rect) {
        let rounding = 6.0;

        // Get colors from the current theme
        let inactive_color = ui.style().visuals.widgets.inactive.bg_fill;
        let active_color = ui.style().visuals.widgets.active.bg_fill;
        let hovered_color = ui.style().visuals.widgets.hovered.bg_fill;

        // Determine top and bottom colors based on state
        let (top_color, bottom_color) = if selected {
            (
                active_color.linear_multiply(1.1),
                active_color.linear_multiply(0.9),
            )
        } else if hover {
            (
                hovered_color.linear_multiply(1.1),
                hovered_color.linear_multiply(0.9),
            )
        } else {
            (
                inactive_color.linear_multiply(1.1),
                inactive_color.linear_multiply(0.9),
            )
        };

        // Create a mesh for the gradient
        let mut mesh = Mesh::default();

        // Add the four corners
        mesh.colored_vertex(rect.left_top(), top_color);
        mesh.colored_vertex(rect.right_top(), top_color);
        mesh.colored_vertex(rect.left_bottom(), bottom_color);
        mesh.colored_vertex(rect.right_bottom(), bottom_color);

        // Add indices to form two triangles
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(1, 3, 2);

        // Paint the mesh
        ui.painter().add(Shape::mesh(mesh));

        // Selection animation - left bar that grows when selected
        if selected || animation_progress > 0.0 {
            let progress = if selected {
                animation_progress
            } else {
                1.0 - animation_progress
            };
            let indicator_width = 4.0;
            let indicator_height = height * progress;

            ui.painter().rect_filled(
                Rect::from_min_size(
                    Pos2::new(rect.min.x, rect.min.y + (height - indicator_height) / 2.0),
                    Vec2::new(indicator_width, indicator_height),
                ),
                CornerRadiusF32 {
                    nw: 2.0,
                    ne: 0.0,
                    sw: 2.0,
                    se: 0.0,
                },
                ui.style().visuals.selection.bg_fill, // Use theme selection color
            );
        }

        // Icon and text colors based on theme
        let text_color = if selected {
            ui.style().visuals.selection.stroke.color
        } else {
            ui.style().visuals.text_color()
        };

        // Icon and text
        ui.painter().text(
            Pos2::new(rect.min.x + 24.0, rect.center().y),
            Align2::LEFT_CENTER,
            icon,
            FontId::proportional(18.0),
            text_color,
        );

        ui.painter().text(
            Pos2::new(rect.min.x + 50.0, rect.center().y),
            Align2::LEFT_CENTER,
            text,
            FontId::proportional(14.0),
            text_color,
        );

        // Top highlight for 3D effect
        if selected {
            let highlight_rect =
                Rect::from_min_size(rect.min, Vec2::new(rect.width(), rect.height() * 0.3));

            ui.painter().rect_filled(
                highlight_rect,
                CornerRadiusF32 {
                    nw: rounding,
                    ne: rounding,
                    sw: 0.0,
                    se: 0.0,
                },
                Color32::from_rgba_premultiplied(255, 255, 255, 20),
            );
        }
    }

    response
}

// Professional looking panel header
pub fn panel_header(ui: &mut Ui, title: &str) {
    let header_height = 28.0;
    let rect = Rect::from_min_size(
        ui.cursor().min,
        Vec2::new(ui.available_width(), header_height),
    );

    // Get colors from theme
    let top_color = ui
        .style()
        .visuals
        .widgets
        .noninteractive
        .bg_fill
        .linear_multiply(1.1);
    let bottom_color = ui
        .style()
        .visuals
        .widgets
        .noninteractive
        .bg_fill
        .linear_multiply(0.9);

    // Use a proper mesh for smooth gradient
    let mut mesh = Mesh::default();

    // Add the four corners
    mesh.colored_vertex(rect.left_top(), top_color);
    mesh.colored_vertex(rect.right_top(), top_color);
    mesh.colored_vertex(rect.left_bottom(), bottom_color);
    mesh.colored_vertex(rect.right_bottom(), bottom_color);

    // Add indices to form two triangles
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);

    // Paint the mesh
    ui.painter().add(Shape::mesh(mesh));

    // Add title text with slight emboss effect
    ui.painter().text(
        Pos2::new(rect.min.x + 10.0, rect.center().y + 1.0),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(15.0),
        Color32::from_rgba_premultiplied(0, 0, 0, 100),
    );

    ui.painter().text(
        Pos2::new(rect.min.x + 10.0, rect.center().y),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(15.0),
        ui.style().visuals.text_color(),
    );

    // Bottom highlight
    ui.painter().line_segment(
        [
            Pos2::new(rect.min.x, rect.max.y - 1.0),
            Pos2::new(rect.max.x, rect.max.y - 1.0),
        ],
        Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 255, 255, 20)),
    );

    // Bottom shadow
    ui.painter().line_segment(
        [
            Pos2::new(rect.min.x, rect.max.y),
            Pos2::new(rect.max.x, rect.max.y),
        ],
        Stroke::new(1.0, Color32::from_rgba_premultiplied(0, 0, 0, 80)),
    );

    ui.add_space(header_height);
}
