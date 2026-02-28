//! 应用层屏幕 UI：BootLoading/MainMenu/SaveSelect/Pause。

mod boot_loading;
mod common;
mod in_game_loading;
mod main_menu;
mod pause_menu;
mod save_select;

use bevy::prelude::*;

pub struct ScreensPlugin;

impl Plugin for ScreensPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(cruft_ui::CruftUiPlugin)
            .add_systems(Startup, spawn_ui_camera)
            .add_plugins((
            boot_loading::BootLoadingScreenPlugin,
            in_game_loading::InGameLoadingScreenPlugin,
            main_menu::MainMenuScreenPlugin,
            save_select::SaveSelectScreenPlugin,
            pause_menu::PauseMenuScreenPlugin,
        ));
    }
}

fn spawn_ui_camera(mut commands: Commands) {
    commands.spawn(Camera2d::default());
}
