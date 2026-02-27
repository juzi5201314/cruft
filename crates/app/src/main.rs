use bevy::asset::AssetPlugin;
use bevy::prelude::*;

mod plugins;

fn main() {
    App::new()
        // CruftUiAssetsPlugin handles EmbeddedAssetPlugin for the UI crate
        .add_plugins(cruft_ui::CruftUiAssetsPlugin)
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: "assets".to_string(),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins(plugins::ProceduralTexturePlugin)
        .add_plugins(plugins::AppUiPlugin)
        .run();
}
