use bevy::prelude::*;

use bevy::state::state_scoped::DespawnOnExit;

use cruft_game_flow::{AppState, BootProgress};
use cruft_ui::ui::{UiBuilder, UiEntityCommandsExt};

use crate::common::spawn_grid_background;

pub struct BootLoadingScreenPlugin;

impl Plugin for BootLoadingScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::BootLoading), spawn_boot_loading_screen)
            .add_systems(
                Update,
                update_boot_loading_screen
                    .run_if(resource_changed::<BootProgress>)
                    .run_if(in_state(AppState::BootLoading)),
            );
    }
}

#[derive(Component)]
struct BootLoadingProgressBar;

#[derive(Component)]
struct BootLoadingLabel;

fn spawn_boot_loading_screen(
    mut commands: Commands,
    theme: Res<cruft_ui::UiTheme>,
    mut ui_materials: ResMut<Assets<cruft_ui::GeistGridMaterial>>,
) {
    spawn_grid_background(&mut commands, &mut ui_materials, AppState::BootLoading);

    commands
        .spawn((
            DespawnOnExit(AppState::BootLoading),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_direction: FlexDirection::Column,
                ..default()
            },
        ))
        .with_children(|parent| {
            let mut ui = UiBuilder::new(parent, &theme);
            ui.card(|ui| {
                ui.label_semibold("Loading…");

                ui.spawn(Node {
                    height: Val::Px(16.0),
                    ..default()
                });

                ui.progress(0.0, Val::Px(320.0))
                    .insert(BootLoadingProgressBar);

                ui.spawn(Node {
                    height: Val::Px(10.0),
                    ..default()
                });

                ui.label("Initializing…").insert(BootLoadingLabel);
            })
            .size(Val::Px(420.0), Val::Auto);
        });
}

fn update_boot_loading_screen(
    progress: Res<BootProgress>,
    mut bars: Query<&mut cruft_ui::UiProgress, With<BootLoadingProgressBar>>,
    mut labels: Query<&mut Text, With<BootLoadingLabel>>,
) {
    for mut bar in &mut bars {
        bar.value = progress.value;
    }
    for mut text in &mut labels {
        text.0 = progress.label.clone();
    }
}
