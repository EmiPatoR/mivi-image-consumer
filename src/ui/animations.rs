// ui/animations.rs - Animation state and systems

use crate::app::EchoViewer;
use std::f32::consts::PI;
use std::time::Instant;

// Animation state for UI elements
pub struct AnimationState {
    pub transition_time: f32,
    pub sidebar_hover: bool,
    pub hover_progress: f32,
    pub button_hover_states: Vec<bool>,
    pub panel_reveal_progress: f32,
    pub startup_progress: f32,
    pub last_update: Instant,
    pub pulse_value: f32,
    pub pulse_direction: bool,
    pub tool_selection_animation: f32,
    pub selected_tool_index: usize,
    pub previous_tool_index: usize,
    pub brightness_change_anim: f32,
    pub contrast_change_anim: f32,
    pub reconnect_pulse: f32,
    pub zoom_anim: f32,
    pub target_zoom: f32,
}

impl Default for AnimationState {
    fn default() -> Self {
        Self {
            transition_time: 0.2,
            sidebar_hover: false,
            hover_progress: 0.0,
            button_hover_states: vec![false; 20], // Preallocate for known maximum buttons
            panel_reveal_progress: 0.0,
            startup_progress: 0.0,
            last_update: Instant::now(),
            pulse_value: 0.0,
            pulse_direction: true,
            tool_selection_animation: 0.0,
            selected_tool_index: 0,
            previous_tool_index: 0,
            brightness_change_anim: 0.0,
            contrast_change_anim: 0.0,
            reconnect_pulse: 0.0,
            zoom_anim: 1.0,
            target_zoom: 1.0,
        }
    }
}

// Update all animations based on time delta
pub fn update_animations(app: &mut EchoViewer, dt: f32) {
    // Tool selection animation
    if app.animation.selected_tool_index != app.animation.previous_tool_index {
        app.animation.tool_selection_animation = 0.0;
        app.animation.previous_tool_index = app.animation.selected_tool_index;
    }

    app.animation.tool_selection_animation = (app.animation.tool_selection_animation + dt * 4.0).min(1.0);

    // Hover animations
    for i in 0..app.animation.button_hover_states.len() {
        // Skip animation if this button index isn't being used
        if i >= 10 {
            continue;
        }

        let is_hovered = app.hovered_button == Some(i);

        if is_hovered && !app.animation.button_hover_states[i] {
            app.animation.button_hover_states[i] = true;
        } else if !is_hovered && app.animation.button_hover_states[i] {
            app.animation.button_hover_states[i] = false;
        }
    }

    // Sidebar hover animation
    if app.animation.sidebar_hover {
        app.animation.hover_progress = (app.animation.hover_progress + dt * 4.0).min(1.0);
    } else {
        app.animation.hover_progress = (app.animation.hover_progress - dt * 4.0).max(0.0);
    }

    // Panel reveal animation
    app.animation.panel_reveal_progress = (app.animation.panel_reveal_progress + dt * 3.0).min(1.0);

    // Startup animation
    app.animation.startup_progress = (app.animation.startup_progress + dt * 2.0).min(1.0);

    // Pulsing animation
    if app.animation.pulse_direction {
        app.animation.pulse_value += dt * 2.0;
        if app.animation.pulse_value >= 1.0 {
            app.animation.pulse_value = 1.0;
            app.animation.pulse_direction = false;
        }
    } else {
        app.animation.pulse_value -= dt * 2.0;
        if app.animation.pulse_value <= 0.0 {
            app.animation.pulse_value = 0.0;
            app.animation.pulse_direction = true;
        }
    }

    // Reconnect pulse
    app.animation.reconnect_pulse = (app.animation.reconnect_pulse + dt * 6.0) % (PI * 2.0);

    // Brightness/contrast animations
    app.animation.brightness_change_anim = (app.animation.brightness_change_anim - dt * 3.0).max(0.0);
    app.animation.contrast_change_anim = (app.animation.contrast_change_anim - dt * 3.0).max(0.0);

    // Smooth zoom animation
    let zoom_diff = app.animation.target_zoom - app.animation.zoom_anim;
    if zoom_diff.abs() > 0.001 {
        app.animation.zoom_anim += zoom_diff * (dt * 8.0).min(1.0);
    } else {
        app.animation.zoom_anim = app.animation.target_zoom;
    }

    // Global elapsed time for animations
    app.elapsed_time += dt;

    // Panel alpha animation
    app.panel_alpha = (app.panel_alpha + dt * 2.0).min(1.0);
}

// Generate a pulsing animation value
pub fn generate_pulse(time: f32, speed: f32, min: f32, max: f32) -> f32 {
    let pulse = ((time * speed).sin() + 1.0) * 0.5;
    min + pulse * (max - min)
}