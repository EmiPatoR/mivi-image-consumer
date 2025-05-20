// ui/panels/tools_panel.rs - Left tools panel implementation

use eframe::egui;
use egui::*;
use crate::app::EchoViewer;
use crate::ui::tools::Tool;
use crate::ui::widgets;

// Draw the left tools panel
pub fn draw(app: &mut EchoViewer, ctx: &egui::Context) {
    egui::SidePanel::left("tools_panel")
        .resizable(true)
        .default_width(56.0)  // Slightly wider for better visibility
        .width_range(56.0..=210.0)
        .show(ctx, |ui| {
            // Detect hover over the sidebar for animation
            if ui.rect_contains_pointer(ui.max_rect()) {
                app.animation.sidebar_hover = true;
            } else {
                app.animation.sidebar_hover = false;
            }

            // Draw panel header
            widgets::panel_header(ui, "Tools");

            ui.vertical_centered(|ui| {
                ui.add_space(8.0);

                // Tool selection
                let tool_names = ["View", "Zoom", "Pan", "ROI", "Measure", "Annotate"];
                let tool_icons = ["👁️", "🔍", "✋", "⬚", "📏", "✏️"];

                // Update selected tool index for animations
                let current_tool_idx = match app.selected_tool {
                    Tool::View => 0,
                    Tool::Zoom => 1,
                    Tool::Pan => 2,
                    Tool::ROI => 3,
                    Tool::Measure => 4,
                    Tool::Annotate => 5,
                };

                app.animation.selected_tool_index = current_tool_idx;

                // Tool buttons with animations
                for (i, (name, icon)) in tool_names.iter().zip(tool_icons.iter()).enumerate() {
                    let selected = i == current_tool_idx;

                    // Animated selection
                    let animation_progress = if i == current_tool_idx {
                        app.animation.tool_selection_animation
                    } else if i == app.animation.previous_tool_index {
                        1.0 - app.animation.tool_selection_animation
                    } else {
                        0.0
                    };

                    // Track if this button is hovered for animation
                    let is_hovered = ui.rect_contains_pointer(ui.min_rect().expand(20.0));

                    if widgets::tool_button(
                        ui,
                        name,
                        icon,
                        selected,
                        is_hovered,
                        animation_progress
                    ).clicked() {
                        app.selected_tool = match i {
                            0 => Tool::View,
                            1 => Tool::Zoom,
                            2 => Tool::Pan,
                            3 => Tool::ROI,
                            4 => Tool::Measure,
                            5 => Tool::Annotate,
                            _ => Tool::View,
                        };
                    }
                }

                ui.separator();

                // Display options
                widgets::panel_header(ui, "Display");
                ui.add_space(8.0);

                // Grid option with animation
                let mut grid_changed = false;
                ui.horizontal(|ui| {
                    if ui.checkbox(&mut app.show_grid, "").changed() {
                        grid_changed = true;
                    }

                    // Animated label based on checkbox state
                    let text_color = if app.show_grid {
                        crate::ui::theme::lerp_color(app.colors.text, app.colors.accent,
                                                     if grid_changed { 1.0 } else { 0.5 })
                    } else {
                        app.colors.text
                    };

                    ui.label(RichText::new("Grid").color(text_color));

                    // Add small indicator when enabled
                    if app.show_grid {
                        ui.painter().circle_filled(
                            ui.cursor().min - Vec2::new(16.0, -8.0),
                            3.0,
                            app.colors.accent
                        );
                    }
                });

                // Rulers option with animation
                let mut rulers_changed = false;
                ui.horizontal(|ui| {
                    if ui.checkbox(&mut app.show_rulers, "").changed() {
                        rulers_changed = true;
                    }

                    // Animated label based on checkbox state
                    let text_color = if app.show_rulers {
                        crate::ui::theme::lerp_color(app.colors.text, app.colors.accent,
                                                     if rulers_changed { 1.0 } else { 0.5 })
                    } else {
                        app.colors.text
                    };

                    ui.label(RichText::new("Rulers").color(text_color));

                    // Add small indicator when enabled
                    if app.show_rulers {
                        ui.painter().circle_filled(
                            ui.cursor().min - Vec2::new(16.0, -8.0),
                            3.0,
                            app.colors.accent
                        );
                    }
                });

                ui.separator();

                // Image adjustments with beautiful sliders
                widgets::panel_header(ui, "Adjustments");
                ui.add_space(8.0);

                // Brightness control with animation
                let old_brightness = app.brightness;
                if widgets::medical_slider(
                    ui,
                    &mut app.brightness,
                    -1.0..=1.0,
                    "Brightness:",
                    app.animation.brightness_change_anim
                ).changed() {
                    app.animation.brightness_change_anim = 1.0;
                }

                // Contrast control with animation
                let old_contrast = app.contrast;
                if widgets::medical_slider(
                    ui,
                    &mut app.contrast,
                    -1.0..=1.0,
                    "Contrast:",
                    app.animation.contrast_change_anim
                ).changed() {
                    app.animation.contrast_change_anim = 1.0;
                }

                ui.separator();

                // Bottom part - expand to show more information
                if ui.button("ℹ️ Frame Info").clicked() {
                    app.show_info_panel = !app.show_info_panel;
                }

                // Annotation text input when annotation tool is selected
                if matches!(app.selected_tool, Tool::Annotate) {
                    ui.separator();
                    ui.label("Annotation Text:");
                    ui.text_edit_singleline(&mut app.annotation_text);
                }
            });
        });
}