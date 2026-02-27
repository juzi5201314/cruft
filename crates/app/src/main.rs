use bevy::prelude::*;
use bevy::asset::AssetPlugin;
use bevy_embedded_assets::{EmbeddedAssetPlugin, PluginMode};

mod plugins;

fn main() {
    App::new()
        .add_plugins(EmbeddedAssetPlugin {
            mode: PluginMode::ReplaceDefault,
        })
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: "assets".to_string(),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins(plugins::ProceduralTexturePlugin)
        .run();
}
