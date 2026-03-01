use std::time::Duration;

use bevy::diagnostic::{
    DiagnosticsStore, FrameTimeDiagnosticsPlugin, SystemInformationDiagnosticsPlugin,
};
use bevy::prelude::*;
use bevy::state::state_scoped::DespawnOnExit;
use bevy::ui::FocusPolicy;

use cruft_game_flow::AppState;
use cruft_voxel::ChunkDrawRange;

pub struct DevHudPlugin;

impl Plugin for DevHudPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FrameTimeDiagnosticsPlugin::default())
            .add_plugins(SystemInformationDiagnosticsPlugin::default())
            .init_resource::<DevHudEnabled>()
            .init_resource::<DevHudRefreshTimer>()
            .add_systems(OnEnter(AppState::InGame), spawn_dev_hud)
            .add_systems(
                Update,
                (
                    toggle_dev_hud.run_if(in_state(AppState::InGame)),
                    apply_dev_hud_visibility
                        .run_if(resource_changed::<DevHudEnabled>)
                        .run_if(in_state(AppState::InGame)),
                    update_dev_hud_text.run_if(in_state(AppState::InGame)),
                ),
            );
    }
}

#[derive(Resource, Debug, Clone, Copy)]
struct DevHudEnabled(pub bool);

impl Default for DevHudEnabled {
    fn default() -> Self {
        Self(cfg!(debug_assertions))
    }
}

#[derive(Resource, Debug, Clone)]
struct DevHudRefreshTimer(pub Timer);

impl Default for DevHudRefreshTimer {
    fn default() -> Self {
        Self(Timer::new(
            Duration::from_secs_f32(0.2),
            TimerMode::Repeating,
        ))
    }
}

#[derive(Component)]
struct DevHudRoot;

#[derive(Component)]
struct DevHudText;

fn spawn_dev_hud(
    mut commands: Commands,
    theme: Res<cruft_ui::UiTheme>,
    enabled: Res<DevHudEnabled>,
) {
    let display = if enabled.0 {
        Display::DEFAULT
    } else {
        Display::None
    };

    let mut root = commands.spawn((
        DevHudRoot,
        DespawnOnExit(AppState::InGame),
        GlobalZIndex(100),
        FocusPolicy::Pass,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            right: Val::Px(12.0),
            display,
            ..default()
        },
    ));

    root.with_children(|parent| {
        parent
            .spawn((
                Node {
                    padding: UiRect::all(Val::Px(10.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(10.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.60)),
                BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.12)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    DevHudText,
                    Text::new(""),
                    TextFont {
                        font: theme.fonts.mono.clone(),
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });
    });
}

fn toggle_dev_hud(keys: Res<ButtonInput<KeyCode>>, mut enabled: ResMut<DevHudEnabled>) {
    if keys.just_pressed(KeyCode::F3) {
        enabled.0 = !enabled.0;
    }
}

fn apply_dev_hud_visibility(
    enabled: Res<DevHudEnabled>,
    mut roots: Query<&mut Node, With<DevHudRoot>>,
) {
    for mut node in &mut roots {
        node.display = if enabled.0 {
            Display::DEFAULT
        } else {
            Display::None
        };
    }
}

fn update_dev_hud_text(
    enabled: Res<DevHudEnabled>,
    time: Res<Time>,
    mut timer: ResMut<DevHudRefreshTimer>,
    diagnostics: Res<DiagnosticsStore>,
    chunks: Query<&ChunkDrawRange>,
    mut texts: Query<&mut Text, With<DevHudText>>,
) {
    if !enabled.0 {
        return;
    }

    timer.0.tick(time.delta());
    if !timer.0.just_finished() {
        return;
    }

    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed());
    let frame_ms = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(|d| d.smoothed());

    let system_cpu = diagnostics
        .get(&SystemInformationDiagnosticsPlugin::SYSTEM_CPU_USAGE)
        .and_then(|d| d.smoothed());
    let system_mem = diagnostics
        .get(&SystemInformationDiagnosticsPlugin::SYSTEM_MEM_USAGE)
        .and_then(|d| d.smoothed());
    let process_cpu = diagnostics
        .get(&SystemInformationDiagnosticsPlugin::PROCESS_CPU_USAGE)
        .and_then(|d| d.smoothed());
    let process_mem_gib = diagnostics
        .get(&SystemInformationDiagnosticsPlugin::PROCESS_MEM_USAGE)
        .and_then(|d| d.smoothed());

    let mut total_chunks = 0usize;
    let mut rendered_chunks = 0usize;
    for range in &chunks {
        total_chunks += 1;
        if range.opaque_len > 0 {
            rendered_chunks += 1;
        }
    }

    let fps_s = fps
        .map(|v| format!("{v:.1}"))
        .unwrap_or_else(|| "--".to_string());
    let frame_s = frame_ms
        .map(|v| format!("{v:.2} ms"))
        .unwrap_or_else(|| "--".to_string());

    let cpu_s = match (process_cpu, system_cpu) {
        (Some(proc), Some(sys)) => format!("proc {proc:.1}%  sys {sys:.1}%"),
        _ => "--".to_string(),
    };
    let mem_s = match (process_mem_gib, system_mem) {
        (Some(proc), Some(sys)) => format!("proc {proc:.2} GiB  sys {sys:.1}%"),
        _ => "--".to_string(),
    };

    let text = format!(
        "FPS: {fps_s}\nFrame: {frame_s}\nChunks: {rendered_chunks}/{total_chunks}\nCPU: {cpu_s}\nMem: {mem_s}",
    );

    for mut t in &mut texts {
        t.0 = text.clone();
    }
}
