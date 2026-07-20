use bevy::prelude::*;
use bevy::ui::{
    BackgroundGradient, BoxShadow, ColorStop, Gradient, LinearGradient, ShadowStyle,
};

// ── Palette ─────────────────────────────────────────────────────────────────
// Deep "space navy" base with a vivid cyan accent. All values are plain colors /
// alpha (rgba), which superui_css (flair) expresses directly.
pub const BG: Color = Color::srgb(0.035, 0.045, 0.075);
pub const PANEL: Color = Color::srgba(0.09, 0.11, 0.17, 0.90);
/// Slightly lighter panel tone used as the top stop of the panel gradient.
pub const PANEL_TOP: Color = Color::srgba(0.15, 0.18, 0.27, 0.92);
pub const PANEL_BORDER: Color = Color::srgb(0.24, 0.32, 0.48);
pub const TEXT: Color = Color::srgb(0.93, 0.96, 1.0);
pub const TEXT_DIM: Color = Color::srgb(0.55, 0.63, 0.78);
pub const ACCENT: Color = Color::srgb(0.25, 0.85, 1.0);
pub const DANGER: Color = Color::srgb(1.0, 0.33, 0.38);
pub const GOOD: Color = Color::srgb(0.35, 0.92, 0.55);
pub const WARN: Color = Color::srgb(1.0, 0.78, 0.28);

pub const SPACE: f32 = 8.0;
pub const RADIUS: f32 = 8.0;
pub const FONT: f32 = 15.0;
pub const FONT_SM: f32 = 12.0;
pub const FONT_LG: f32 = 30.0;

/// HP-fraction color ramp used by health bars and nameplates (green → amber → red).
pub fn hp_color(frac: f32) -> Color {
    let f = frac.clamp(0.0, 1.0);
    // Punchier ramp than a straight lerp: stays green while healthy, reddens fast low.
    Color::srgb(0.95 * (1.0 - f * f) + 0.10, 0.30 + 0.62 * f, 0.30)
}

// ── Color helpers ───────────────────────────────────────────────────────────
pub fn lighten(c: Color, amt: f32) -> Color {
    let s = c.to_srgba();
    Color::srgba(
        (s.red + amt).min(1.0),
        (s.green + amt).min(1.0),
        (s.blue + amt).min(1.0),
        s.alpha,
    )
}

pub fn darken(c: Color, amt: f32) -> Color {
    let s = c.to_srgba();
    Color::srgba(
        (s.red - amt).max(0.0),
        (s.green - amt).max(0.0),
        (s.blue - amt).max(0.0),
        s.alpha,
    )
}

// ── Decoration helpers (each maps to a flair-supported CSS property) ──────────

/// Soft dark drop shadow for panels/modals. flair: `box-shadow` (no inset).
pub fn panel_shadow() -> BoxShadow {
    BoxShadow(vec![ShadowStyle {
        color: Color::srgba(0.0, 0.0, 0.0, 0.55),
        x_offset: Val::Px(0.0),
        y_offset: Val::Px(6.0),
        spread_radius: Val::Px(0.0),
        blur_radius: Val::Px(20.0),
    }])
}

/// A colored outer glow for active/emphasised elements. flair: colored `box-shadow`.
pub fn glow(color: Color) -> BoxShadow {
    BoxShadow(vec![ShadowStyle {
        color: color.with_alpha(0.60),
        x_offset: Val::Px(0.0),
        y_offset: Val::Px(0.0),
        spread_radius: Val::Px(1.0),
        blur_radius: Val::Px(14.0),
    }])
}

/// Subtle top-lighter vertical panel fill. flair: `linear-gradient(...)`.
pub fn panel_gradient() -> BackgroundGradient {
    BackgroundGradient(vec![Gradient::Linear(LinearGradient::to_bottom(vec![
        ColorStop::auto(PANEL_TOP),
        ColorStop::auto(PANEL),
    ]))])
}

/// Dark text drop-shadow for legibility. flair: `text-shadow` (offset + color).
pub fn text_shadow() -> TextShadow {
    TextShadow {
        offset: Vec2::new(0.0, 2.0),
        color: Color::srgba(0.0, 0.0, 0.0, 0.7),
    }
}

/// Accent-tinted glow behind a heading, for a neon feel. flair: `text-shadow`.
pub fn title_glow() -> TextShadow {
    TextShadow {
        offset: Vec2::ZERO,
        color: ACCENT.with_alpha(0.85),
    }
}
