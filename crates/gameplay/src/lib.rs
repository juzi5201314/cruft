//! InGame 骨架：WorldRoot、相机/灯光、暂停 gating + FPS 控制（体素碰撞）。

mod fps_controller;
mod plugin;
mod voxel_collision;

use bevy::prelude::*;

/// Gameplay 子系统入口插件。
///
/// 注意：`GameplayPlugin` 只在此处定义，避免与 `plugin` 模块重复命名。
pub struct GameplayPlugin;

impl Plugin for GameplayPlugin {
    fn build(&self, app: &mut App) {
        plugin::build(app);
    }
}
