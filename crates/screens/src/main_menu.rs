use bevy::prelude::*;

use bevy::state::state_scoped::DespawnOnExit;

use cruft_game_flow::{FlowRequest, FrontEndState};
use cruft_ui::ui::{UiBuilder, UiEntityCommandsExt};

use crate::common::spawn_grid_background;

pub struct MainMenuScreenPlugin;

impl Plugin for MainMenuScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(FrontEndState::MainMenu), spawn_main_menu);
    }
}

fn spawn_main_menu(
    mut commands: Commands,
    theme: Res<cruft_ui::UiTheme>,
    mut ui_materials: ResMut<Assets<cruft_ui::GeistGridMaterial>>,
) {
    spawn_grid_background(&mut commands, &mut ui_materials, FrontEndState::MainMenu);

    commands
        .spawn((
            DespawnOnExit(FrontEndState::MainMenu),
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
                ui.spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(32.0),
                    ..default()
                })
                .with_children(|p| {
                    let mut ui = UiBuilder::new(p, &theme);
                    ui.label_semibold("Cruft");

                    ui.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(20.0),
                        ..default()
                    })
                    .with_children(|p| {
                        let mut ui = UiBuilder::new(p, &theme);
                        ui.button(cruft_ui::UiButtonVariant::Primary, |ui| {
                            ui.icon('\u{e9b2}');
                            ui.label("Start");
                        })
                        .click(on_start_clicked)
                        .size(Val::Px(180.0), Val::Px(60.0));
                    });
                });
            })
            .size(Val::Px(420.0), Val::Auto);
        });
}

fn on_start_clicked(_ev: On<cruft_ui::UiClick>, mut writer: MessageWriter<FlowRequest>) {
    writer.write(FlowRequest::EnterSaveSelect);
}
