use bevy::prelude::*;

use bevy::state::state_scoped::DespawnOnExit;

use cruft_game_flow::{AppState, FlowRequest, InGameState};

pub struct GameplayPlugin;

impl Plugin for GameplayPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InGame), spawn_world_root)
            .add_systems(
                Update,
                (
                    toggle_pause_on_escape
                        .in_set(GameplaySet::Presentation)
                        .run_if(in_state(AppState::InGame))
                        ,
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
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum GameplaySet {
    Simulation,
    Presentation,
}

#[derive(Component)]
struct WorldRoot;

fn spawn_world_root(mut commands: Commands) {
    commands
        .spawn((
            WorldRoot,
            DespawnOnExit(AppState::InGame),
            Transform::default(),
            GlobalTransform::default(),
        ))
        .with_children(|parent| {
            parent.spawn((
                Camera3d::default(),
                Transform::from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
            ));
            parent.spawn((
                PointLight {
                    shadows_enabled: true,
                    ..default()
                },
                Transform::from_xyz(4.0, 8.0, 4.0),
            ));
        });
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
