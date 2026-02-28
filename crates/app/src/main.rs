use std::path::PathBuf;

use bevy::asset::AssetPlugin;
use bevy::prelude::*;

fn main() {
    let save_root_dir = cruft_save::SaveRootDir(
        std::env::var_os("CRUFT_SAVE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(cruft_save::SaveRootDir::default_path),
    );

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
        .insert_resource(save_root_dir)
        .add_plugins(cruft_game_flow::GameFlowPlugin)
        .add_plugins(cruft_save::SavePlugin)
        .add_plugins(cruft_voxel::VoxelPlugin)
        .add_plugins(cruft_proc_textures::ProcTexturesPlugin)
        .add_plugins(cruft_screens::ScreensPlugin)
        .add_plugins(cruft_gameplay::GameplayPlugin)
        .run();
}
