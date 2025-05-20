// ui/animations.rs - Animation state and systems

use crate::app::EchoViewer;
use std::f32::consts::PI;
use std::time::Instant;

// Animation settings for performance control
pub struct AnimationSettings {
    pub enabled: bool,           // Global toggle
    pub quality_level: u8,       // 1-3 (low, medium, high)
    pub disable_when_capturing: bool, // Turn off animations during capture
}

impl Default for AnimationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            quality_level: 2,
            disable_when_capturing: true,
        }
    }
}

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

// Easing functions for smoother animations
fn ease_in_out(t: f32) -> f32 {
    if t < 0.5 {
        2.0 * t * t
    } else {
        -1.0 + (4.0 - 2.0 * t) * t
    }
}

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t) * (1.0 - t)
}

// Update all animations based on time delta
pub fn update_animations(app: &mut EchoViewer, dt: f32) {
    // Slow down all animations by using a smaller time delta
    let slower_dt = dt * 0.6; // Reduce speed by 40%

    // Skip all animations if disabled or when capturing
    if app.animation_settings.is_some() &&
        (!app.animation_settings.as_ref().unwrap().enabled ||
            (app.animation_settings.as_ref().unwrap().disable_when_capturing && app.is_capturing.unwrap_or(false))) {
        return;
    }

    // Tool selection animation
    if app.animation.selected_tool_index != app.animation.previous_tool_index {
        app.animation.tool_selection_animation = 0.0;
        app.animation.previous_tool_index = app.animation.selected_tool_index;
    }

    // Apply animation with slower speed
    app.animation.tool_selection_animation =
        (app.animation.tool_selection_animation + slower_dt * 3.0).min(1.0);

    // Apply easing for smoother animation
    app.animation.tool_selection_animation = ease_out(app.animation.tool_selection_animation);

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
        app.animation.hover_progress = (app.animation.hover_progress + slower_dt * 3.0).min(1.0);
    } else {
        app.animation.hover_progress = (app.animation.hover_progress - slower_dt * 3.0).max(0.0);
    }

    // Apply easing for smoother transitions
    app.animation.hover_progress = ease_out(app.animation.hover_progress);

    // Panel reveal animation
    app.animation.panel_reveal_progress = (app.animation.panel_reveal_progress + slower_dt * 2.0).min(1.0);

    // Apply easing
    app.animation.panel_reveal_progress = ease_out(app.animation.panel_reveal_progress);

    // Startup animation
    app.animation.startup_progress = (app.animation.startup_progress + slower_dt * 1.5).min(1.0);

    // Apply easing
    app.animation.startup_progress = ease_in_out(app.animation.startup_progress);

    // Pulsing animation - slower 
    if app.animation.pulse_direction {
        app.animation.pulse_value += slower_dt * 1.5;  // Slower pulse
        if app.animation.pulse_value >= 1.0 {
            app.animation.pulse_value = 1.0;
            app.animation.pulse_direction = false;
        }
    } else {
        app.animation.pulse_value -= slower_dt * 1.5;  // Slower pulse
        if app.animation.pulse_value <= 0.0 {
            app.animation.pulse_value = 0.0;
            app.animation.pulse_direction = true;
        }
    }

    // Apply sinusoidal curve for more natural pulsing
    app.animation.pulse_value = (1.0 - (app.animation.pulse_value * PI).cos()) * 0.5;

    // Reconnect pulse
    app.animation.reconnect_pulse = (app.animation.reconnect_pulse + slower_dt * 4.0) % (PI * 2.0);

    // Brightness/contrast animations
    app.animation.brightness_change_anim = (app.animation.brightness_change_anim - slower_dt * 2.0).max(0.0);
    app.animation.contrast_change_anim = (app.animation.contrast_change_anim - slower_dt * 2.0).max(0.0);

    // Smooth zoom animation with easing
    let zoom_diff = app.animation.target_zoom - app.animation.zoom_anim;
    if zoom_diff.abs() > 0.001 {
        // Use easing for smoother zoom with slower speed
        let zoom_speed = ease_out((slower_dt * 5.0).min(1.0));
        app.animation.zoom_anim += zoom_diff * zoom_speed;
    } else {
        app.animation.zoom_anim = app.animation.target_zoom;
    }

    // Global elapsed time for animations
    app.elapsed_time += slower_dt;

    // Panel alpha animation with easing
    app.panel_alpha = (app.panel_alpha + slower_dt * 1.5).min(1.0);
    app.panel_alpha = ease_out(app.panel_alpha);
}

// Generate a pulsing animation value
pub fn generate_pulse(time: f32, speed: f32, min: f32, max: f32) -> f32 {
    let pulse = ((time * speed).sin() + 1.0) * 0.5;
    min + pulse * (max - min)
}