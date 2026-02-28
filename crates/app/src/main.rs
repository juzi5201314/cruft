use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "cruft", about = "Cruft - a Minecraft-like sandbox game built with Bevy")]
struct Cli {
    /// 存档根目录（优先级最高）。
    #[arg(long, env = "CRUFT_SAVE_DIR", value_name = "DIR")]
    save_dir: Option<std::path::PathBuf>,
}

fn main() {
    let cli = Cli::parse();
    let save_root_dir = cruft_save::SaveRootDir(
        cli.save_dir
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
        .add_plugins(cruft_proc_textures::ProcTexturesPlugin)
        .add_plugins(cruft_screens::ScreensPlugin)
        .add_plugins(cruft_gameplay::GameplayPlugin)
        .run();
}
