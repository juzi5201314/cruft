use bevy::prelude::*;

use bevy::render::experimental::occlusion_culling::OcclusionCulling;
use bevy::state::state_scoped::DespawnOnExit;

use cruft_game_flow::{AppState, FlowRequest, InGameState};
use cruft_voxel::{VoxelCenter, VoxelWorld};

use crate::fps_controller::{
    apply_mouse_look, apply_movement_and_physics, lock_cursor, unlock_cursor, FpsCamera,
    FpsController, FpsGrounded, FpsLook, FpsPlayer, FpsVelocity,
};

pub(crate) fn build(app: &mut App) {
    app.add_systems(OnEnter(InGameState::Loading), spawn_world_root)
        .add_systems(OnExit(AppState::InGame), unlock_cursor)
        .add_systems(OnEnter(InGameState::Loading), unlock_cursor)
        .add_systems(OnEnter(InGameState::Playing), lock_cursor)
        .add_systems(OnEnter(InGameState::Paused), unlock_cursor)
        .add_systems(
            Update,
            (
                toggle_pause_on_escape
                    .in_set(GameplaySet::Presentation)
                    .run_if(in_state(AppState::InGame)),
                sync_voxel_center_from_player
                    .in_set(GameplaySet::Presentation)
                    .run_if(in_state(AppState::InGame)),
                (apply_mouse_look, apply_movement_and_physics)
                    .chain()
                    .in_set(GameplaySet::Simulation),
            ),
        )
        .configure_sets(
            Update,
            (
                GameplaySet::Simulation.run_if(in_state(InGameState::Playing)),
                GameplaySet::Presentation,
            )
                .chain(),
        );
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum GameplaySet {
    Simulation,
    Presentation,
}

#[derive(Component)]
struct WorldRoot;

fn spawn_world_root(
    mut commands: Commands,
    voxel_world: Option<Res<VoxelWorld>>,
    center: Option<ResMut<VoxelCenter>>,
) {
    let controller = FpsController::default();
    let eye_height = controller.eye_height;

    // 出生点：按地形高度把玩家放到地表上方，避免“出生在地下看上表面”
    // 造成“方块上下颠倒”的错觉。
    let spawn_x = -2.5f32;
    let spawn_z = 9.0f32;
    let wx = spawn_x.floor() as i32;
    let wz = spawn_z.floor() as i32;
    let surface_y = voxel_world
        .as_ref()
        .map(|w| w.terrain.height_at(wx, wz))
        .unwrap_or(0);
    let player_feet_y = (surface_y + 1) as f32;

    let initial_eye = Vec3::new(spawn_x, player_feet_y + eye_height, spawn_z);
    let forward = (Vec3::ZERO - initial_eye).normalize_or_zero();
    let yaw = forward.x.atan2(-forward.z);
    let pitch = forward
        .y
        .asin()
        .clamp(-controller.max_pitch_radians, controller.max_pitch_radians);
    let player_feet = Vec3::new(spawn_x, player_feet_y, spawn_z);

    if let Some(mut center) = center {
        center.0 = IVec3::new(
            player_feet.x.floor() as i32,
            player_feet.y.floor() as i32,
            player_feet.z.floor() as i32,
        );
    }

    commands
        .spawn((
            WorldRoot,
            DespawnOnExit(AppState::InGame),
            Transform::default(),
            GlobalTransform::default(),
            Visibility::default(),
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    FpsPlayer,
                    controller,
                    FpsLook { yaw, pitch },
                    FpsVelocity::default(),
                    FpsGrounded::default(),
                    Transform::from_translation(player_feet)
                        .with_rotation(Quat::from_rotation_y(yaw)),
                    Visibility::default(),
                ))
                .with_children(|player| {
                    player.spawn((
                        Camera3d::default(),
                        OcclusionCulling,
                        FpsCamera,
                        Transform::from_translation(Vec3::new(0.0, eye_height, 0.0))
                            .with_rotation(Quat::from_rotation_x(pitch)),
                    ));
                });
            parent.spawn((
                DirectionalLight {
                    shadows_enabled: true,
                    illuminance: 22_000.0,
                    ..default()
                },
                // 方向光替代点光阴影，避免每帧 6 面阴影渲染带来的高开销。
                Transform::from_rotation(Quat::from_euler(EulerRot::ZYX, 0.0, -0.85, -0.95)),
            ));
        });

    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.67, 0.70, 0.76),
        brightness: 72.0,
        ..default()
    });
}

fn sync_voxel_center_from_player(
    center: Option<ResMut<VoxelCenter>>,
    players: Query<&Transform, With<FpsPlayer>>,
) {
    let Some(mut center) = center else {
        return;
    };
    let Ok(player) = players.single() else {
        return;
    };
    let feet = player.translation;
    center.0 = IVec3::new(
        feet.x.floor() as i32,
        feet.y.floor() as i32,
        feet.z.floor() as i32,
    );
}

fn toggle_pause_on_escape(
    keys: Res<ButtonInput<KeyCode>>,
    app: Res<State<AppState>>,
    ingame: Option<Res<State<InGameState>>>,
    mut writer: MessageWriter<FlowRequest>,
) {
    if *app.get() != AppState::InGame {
        return;
    }
    let Some(ingame) = ingame else {
        return;
    };
    let state = *ingame.get();
    if !matches!(state, InGameState::Playing | InGameState::Paused) {
        return;
    }

    if keys.just_pressed(KeyCode::Escape) {
        writer.write(FlowRequest::TogglePause);
    }
}
