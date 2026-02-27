use bevy::prelude::*;

/// UI 主题 Token（默认实现为 Geist 风格）。
#[derive(Resource, Debug, Clone)]
pub struct UiTheme {
    pub bg: Color,
    pub fg: Color,
    pub muted_fg: Color,
    pub border: Color,
    pub accent: Color,
    pub primary_bg: Color,
    pub primary_fg: Color,
    pub secondary_bg: Color,
    pub secondary_fg: Color,
    pub radius: f32,
}

impl UiTheme {
    pub fn geist_light() -> Self {
        Self {
            bg: Color::WHITE,
            fg: Color::BLACK,
            muted_fg: Color::srgb(0.4, 0.4, 0.4),
            border: Color::srgb(0.92, 0.92, 0.92),
            accent: Color::srgb(0.98, 0.98, 0.98),
            primary_bg: Color::BLACK,
            primary_fg: Color::WHITE,
            secondary_bg: Color::WHITE,
            secondary_fg: Color::BLACK,
            radius: 8.0,
        }
    }

    pub fn geist_dark() -> Self {
        Self {
            bg: Color::BLACK,
            fg: Color::WHITE,
            muted_fg: Color::srgb(0.6, 0.6, 0.6),
            border: Color::srgb(0.15, 0.15, 0.15),
            accent: Color::srgb(0.05, 0.05, 0.05),
            primary_bg: Color::WHITE,
            primary_fg: Color::BLACK,
            secondary_bg: Color::BLACK,
            secondary_fg: Color::WHITE,
            radius: 8.0,
        }
    }
}
