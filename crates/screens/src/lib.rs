//! 应用层屏幕 UI：BootLoading/MainMenu/SaveSelect/Pause。

mod boot_loading;
mod common;
mod dev_hud;
mod in_game_loading;
mod main_menu;
mod pause_menu;
mod save_select;

use bevy::prelude::*;

use cruft_game_flow::AppState;

pub struct ScreensPlugin;

impl Plugin for ScreensPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(cruft_ui::CruftUiPlugin)
            .add_systems(Startup, spawn_ui_camera)
            .add_systems(OnEnter(AppState::InGame), configure_ui_camera_for_in_game)
            .add_systems(OnExit(AppState::InGame), configure_ui_camera_for_frontend)
            .add_plugins(dev_hud::DevHudPlugin)
            .add_plugins((
                boot_loading::BootLoadingScreenPlugin,
                in_game_loading::InGameLoadingScreenPlugin,
                main_menu::MainMenuScreenPlugin,
                save_select::SaveSelectScreenPlugin,
                pause_menu::PauseMenuScreenPlugin,
            ));
    }
}

#[derive(Component)]
struct UiCamera;

fn spawn_ui_camera(mut commands: Commands) {
    commands.spawn((UiCamera, Camera2d::default()));
}

fn configure_ui_camera_for_in_game(mut cameras: Query<&mut Camera, With<UiCamera>>) {
    for mut camera in &mut cameras {
        // UI 需要覆盖在 3D 世界之上，但不能清屏（否则会把 3D 画面擦掉）。
        camera.order = 10;
        camera.clear_color = ClearColorConfig::None;
    }
}

fn configure_ui_camera_for_frontend(mut cameras: Query<&mut Camera, With<UiCamera>>) {
    for mut camera in &mut cameras {
        // 前端只有 UI 相机时必须清屏，否则可能显示未定义内容。
        camera.order = 0;
        camera.clear_color = ClearColorConfig::Default;
    }
}
