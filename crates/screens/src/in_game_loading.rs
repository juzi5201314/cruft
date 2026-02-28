use bevy::prelude::*;
use bevy::state::state_scoped::DespawnOnExit;

use cruft_game_flow::InGameState;
use cruft_save::SaveLoadResult;
use cruft_ui::ui::{UiBuilder, UiEntityCommandsExt};

pub struct InGameLoadingScreenPlugin;

impl Plugin for InGameLoadingScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InGameLoadingError>()
            .add_systems(
                OnEnter(InGameState::Loading),
                (clear_loading_error, spawn_loading_overlay).chain(),
            )
            .add_systems(
                Update,
                (
                    capture_loading_error.run_if(in_state(InGameState::Loading)),
                    update_loading_overlay
                        .run_if(resource_changed::<InGameLoadingError>)
                        .run_if(in_state(InGameState::Loading)),
                ),
            );
    }
}

#[derive(Resource, Debug, Default, Clone)]
struct InGameLoadingError(pub Option<String>);

#[derive(Component)]
struct LoadingLabel;

#[derive(Component)]
struct LoadingErrorLabel;

#[derive(Component)]
struct QuitButtonContainer;

fn clear_loading_error(mut err: ResMut<InGameLoadingError>) {
    err.0 = None;
}

fn spawn_loading_overlay(mut commands: Commands, theme: Res<cruft_ui::UiTheme>) {
    commands
        .spawn((
            DespawnOnExit(InGameState::Loading),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|parent| {
            let mut ui = UiBuilder::new(parent, &theme);
            ui.card(|ui| {
                ui.label_semibold("Loading…").insert(LoadingLabel);
                ui.spawn(Node {
                    height: Val::Px(10.0),
                    ..default()
                });
                ui.label("Preparing world & save data");
                ui.spawn(Node {
                    height: Val::Px(10.0),
                    ..default()
                });
                ui.label_mono("").insert(LoadingErrorLabel);
                ui.spawn((
                    QuitButtonContainer,
                    Node {
                        height: Val::Px(0.0),
                        ..default()
                    },
                ));
            })
            .size(Val::Px(420.0), Val::Auto);
        });
}

fn capture_loading_error(
    mut reader: MessageReader<SaveLoadResult>,
    mut err: ResMut<InGameLoadingError>,
) {
    for msg in reader.read() {
        if let SaveLoadResult::Failed { message, .. } = msg {
            err.0 = Some(message.clone());
        }
    }
}

fn update_loading_overlay(
    mut commands: Commands,
    theme: Res<cruft_ui::UiTheme>,
    err: Res<InGameLoadingError>,
    mut labels: Query<&mut Text, With<LoadingErrorLabel>>,
    containers: Query<(Entity, Option<&Children>), With<QuitButtonContainer>>,
) {
    for mut text in &mut labels {
        text.0 = err.0.clone().unwrap_or_default();
    }

    if err.0.is_none() {
        return;
    }

    let Ok((container, children)) = containers.single() else {
        return;
    };
    if children.is_some_and(|c| !c.is_empty()) {
        return;
    }

    commands.entity(container).with_children(|parent| {
        let mut ui = UiBuilder::new(parent, &theme);
        ui.spawn(Node {
            height: Val::Px(12.0),
            ..default()
        });
        ui.button(cruft_ui::UiButtonVariant::Secondary, |ui| {
            ui.label("Quit to Main Menu");
        })
        .click(on_quit_to_menu)
        .size(Val::Px(200.0), Val::Px(44.0));
    });
}

fn on_quit_to_menu(
    _ev: On<cruft_ui::UiClick>,
    mut writer: MessageWriter<cruft_game_flow::FlowRequest>,
) {
    writer.write(cruft_game_flow::FlowRequest::QuitToMainMenu);
}
