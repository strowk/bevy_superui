use bevy::prelude::*;

pub const BG: Color = Color::srgb(0.07, 0.07, 0.10);
pub const PANEL: Color = Color::srgba(0.13, 0.14, 0.20, 0.92);
pub const PANEL_BORDER: Color = Color::srgb(0.28, 0.30, 0.42);
pub const TEXT: Color = Color::srgb(0.90, 0.92, 0.98);
pub const TEXT_DIM: Color = Color::srgb(0.60, 0.63, 0.72);
pub const ACCENT: Color = Color::srgb(0.35, 0.75, 1.0);
pub const DANGER: Color = Color::srgb(0.95, 0.35, 0.38);
pub const GOOD: Color = Color::srgb(0.45, 0.85, 0.45);
pub const WARN: Color = Color::srgb(0.95, 0.75, 0.30);

pub const SPACE: f32 = 8.0;
pub const RADIUS: f32 = 6.0;
pub const FONT: f32 = 15.0;
pub const FONT_SM: f32 = 12.0;
pub const FONT_LG: f32 = 28.0;

/// HP-fraction color ramp used by health bars and nameplates.
pub fn hp_color(frac: f32) -> Color {
    let f = frac.clamp(0.0, 1.0);
    Color::srgb(0.9 * (1.0 - f) + 0.15, 0.25 + 0.6 * f, 0.28)
}
