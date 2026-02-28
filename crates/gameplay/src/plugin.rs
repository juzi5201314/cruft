use bevy::prelude::*;

use bevy::state::state_scoped::DespawnOnExit;

use cruft_game_flow::{AppState, FlowRequest, InGameState};
use cruft_voxel::VoxelCenter;

use crate::fps_controller::{
    apply_mouse_look, apply_movement_and_physics, lock_cursor, unlock_cursor, FpsCamera,
    FpsController, FpsGrounded, FpsLook, FpsPlayer, FpsVelocity,
};

pub(crate) fn build(app: &mut App) {
    app.add_systems(OnEnter(AppState::InGame), spawn_world_root)
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

fn spawn_world_root(mut commands: Commands) {
    let controller = FpsController::default();
    let eye_height = controller.eye_height;
    let initial_eye = Vec3::new(-2.5, 4.5, 9.0);
    let forward = (Vec3::ZERO - initial_eye).normalize_or_zero();
    let yaw = forward.x.atan2(-forward.z);
    let pitch = forward
        .y
        .asin()
        .clamp(-controller.max_pitch_radians, controller.max_pitch_radians);
    let player_feet = initial_eye - Vec3::new(0.0, eye_height, 0.0);

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
                        FpsCamera,
                        Transform::from_translation(Vec3::new(0.0, eye_height, 0.0))
                            .with_rotation(Quat::from_rotation_x(pitch)),
                    ));
                });
            parent.spawn((
                PointLight {
                    shadows_enabled: true,
                    ..default()
                },
                Transform::from_xyz(4.0, 8.0, 4.0),
            ));
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
