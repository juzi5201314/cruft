use bevy::prelude::*;

use bevy::state::state_scoped::DespawnOnExit;

use cruft_game_flow::{FlowRequest, InGameState};
use cruft_ui::ui::{UiBuilder, UiEntityCommandsExt};

pub struct PauseMenuScreenPlugin;

impl Plugin for PauseMenuScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(InGameState::Paused), spawn_pause_menu);
    }
}

fn spawn_pause_menu(mut commands: Commands, theme: Res<cruft_ui::UiTheme>) {
    commands
        .spawn((
            DespawnOnExit(InGameState::Paused),
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
                ui.label_semibold("Paused");
                ui.spawn(Node {
                    height: Val::Px(16.0),
                    ..default()
                });

                ui.button(cruft_ui::UiButtonVariant::Primary, |ui| {
                    ui.label("Resume");
                })
                    .click(on_resume_clicked)
                    .size(Val::Px(200.0), Val::Px(52.0));

                ui.spawn(Node {
                    height: Val::Px(12.0),
                    ..default()
                });

                ui.button(cruft_ui::UiButtonVariant::Secondary, |ui| {
                    ui.label("Quit to Main Menu");
                })
                    .click(on_quit_clicked)
                    .size(Val::Px(200.0), Val::Px(52.0));
            })
            .size(Val::Px(360.0), Val::Auto);
        });
}

fn on_resume_clicked(_ev: On<cruft_ui::UiClick>, mut writer: MessageWriter<FlowRequest>) {
    writer.write(FlowRequest::Resume);
}

fn on_quit_clicked(_ev: On<cruft_ui::UiClick>, mut writer: MessageWriter<FlowRequest>) {
    writer.write(FlowRequest::QuitToMainMenu);
}
