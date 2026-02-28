use bevy::ecs::world::{FromWorld, World};
use bevy::prelude::*;

/// UI 字体资源句柄。
#[derive(Resource, Debug, Clone, Default)]
pub struct UiFontResources {
    pub sans: Handle<Font>,
    pub sans_semibold: Handle<Font>,
    pub mono: Handle<Font>,
    pub icons: Handle<Font>,
}

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
    pub fonts: UiFontResources,
}

impl FromWorld for UiTheme {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world
            .get_resource::<AssetServer>()
            .expect("CruftUiPlugin requires AssetServer (add DefaultPlugins before CruftUiPlugin)");

        let fonts = UiFontResources {
            sans: asset_server.load("fonts/Geist-Regular.ttf"),
            sans_semibold: asset_server.load("fonts/Geist-SemiBold.ttf"),
            mono: asset_server.load("fonts/GeistMono-Regular.ttf"),
            icons: asset_server.load("icons/lucide.ttf"),
        };
        Self::geist_light(fonts)
    }
}

impl UiTheme {
    pub fn geist_light(fonts: UiFontResources) -> Self {
        Self {
            bg: Color::WHITE,
            fg: Color::BLACK,
            muted_fg: Color::srgb(0.4, 0.4, 0.4),
            border: Color::srgb(0.92, 0.92, 0.92),
            accent: Color::srgb(0.94, 0.94, 0.94),
            primary_bg: Color::BLACK,
            primary_fg: Color::WHITE,
            secondary_bg: Color::WHITE,
            secondary_fg: Color::BLACK,
            radius: 8.0,
            fonts,
        }
    }

    pub fn geist_dark(fonts: UiFontResources) -> Self {
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
            fonts,
        }
    }
}
