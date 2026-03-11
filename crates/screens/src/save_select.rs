use bevy::prelude::*;
use bevy::state::state_scoped::DespawnOnExit;

use cruft_game_flow::{FlowRequest, FrontEndState};
use cruft_save::{SaveId, SaveIndex, SaveOpRequest, SaveOpResult};
use cruft_ui::ui::{UiBuilder, UiEntityCommandsExt};

use crate::common::spawn_grid_background;

pub struct SaveSelectScreenPlugin;

impl Plugin for SaveSelectScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SaveSelectState>()
            .init_resource::<SpawnedModalKind>()
            .add_systems(OnEnter(FrontEndState::SaveSelect), spawn_save_select_screen)
            .add_systems(OnExit(FrontEndState::SaveSelect), reset_save_select_state)
            .add_systems(
                Update,
                (
                    rebuild_save_list
                        .run_if(resource_changed::<SaveIndex>)
                        .run_if(in_state(FrontEndState::SaveSelect)),
                    update_selection_styles
                        .run_if(resource_changed::<SaveSelectState>)
                        .run_if(in_state(FrontEndState::SaveSelect)),
                    sync_modal
                        .run_if(resource_changed::<SaveSelectState>)
                        .run_if(in_state(FrontEndState::SaveSelect)),
                    handle_save_results.run_if(in_state(FrontEndState::SaveSelect)),
                ),
            );
    }
}

#[derive(Resource, Debug, Default, Clone)]
struct SaveSelectState {
    selected: Option<String>,
    modal: Option<ModalKind>,
}

#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
struct SpawnedModalKind(pub Option<ModalKind>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModalKind {
    New,
    Rename,
    ConfirmDelete,
}

#[derive(Component)]
struct SaveSelectRoot;

#[derive(Component)]
struct SaveListContainer;

#[derive(Component)]
struct SaveEntryId(String);

#[derive(Component)]
struct SaveSelectModalRoot;

#[derive(Component)]
struct SaveSelectModalTextInput;

fn reset_save_select_state(
    mut state: ResMut<SaveSelectState>,
    mut spawned: ResMut<SpawnedModalKind>,
) {
    *state = SaveSelectState::default();
    spawned.0 = None;
}

fn spawn_save_select_screen(
    mut commands: Commands,
    theme: Res<cruft_ui::UiTheme>,
    mut ui_materials: ResMut<Assets<cruft_ui::GeistGridMaterial>>,
    index: Res<SaveIndex>,
    mut state: ResMut<SaveSelectState>,
    mut spawned: ResMut<SpawnedModalKind>,
) {
    *state = SaveSelectState::default();
    spawned.0 = None;

    spawn_grid_background(&mut commands, &mut ui_materials, FrontEndState::SaveSelect);

    let mut list_container_entity: Option<Entity> = None;

    commands
        .spawn((
            SaveSelectRoot,
            DespawnOnExit(FrontEndState::SaveSelect),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|parent| {
            let mut ui = UiBuilder::new(parent, &theme);
            ui.card(|ui| {
                ui.label_semibold("Select Save");
                ui.spawn(Node {
                    height: Val::Px(16.0),
                    ..default()
                });

                ui.spawn(Node {
                    width: Val::Px(560.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(24.0),
                    ..default()
                })
                .with_children(|p| {
                    let mut ui = UiBuilder::new(p, &theme);

                    let list_container = ui.spawn((
                        SaveListContainer,
                        Node {
                            width: Val::Px(360.0),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(10.0),
                            ..default()
                        },
                    ));
                    list_container_entity = Some(list_container.id());

                    ui.spawn(Node {
                        width: Val::Px(176.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(12.0),
                        ..default()
                    })
                    .with_children(|p| {
                        let mut ui = UiBuilder::new(p, &theme);

                        ui.button(cruft_ui::UiButtonVariant::Primary, |ui| {
                            ui.label("Load");
                        })
                        .click(on_load_clicked)
                        .size(Val::Px(176.0), Val::Px(52.0));

                        ui.button(cruft_ui::UiButtonVariant::Secondary, |ui| {
                            ui.label("New");
                        })
                        .click(on_new_clicked)
                        .size(Val::Px(176.0), Val::Px(52.0));

                        ui.button(cruft_ui::UiButtonVariant::Secondary, |ui| {
                            ui.label("Rename");
                        })
                        .click(on_rename_clicked)
                        .size(Val::Px(176.0), Val::Px(52.0));

                        ui.button(cruft_ui::UiButtonVariant::Secondary, |ui| {
                            ui.label("Copy");
                        })
                        .click(on_copy_clicked)
                        .size(Val::Px(176.0), Val::Px(52.0));

                        ui.button(cruft_ui::UiButtonVariant::Secondary, |ui| {
                            ui.label("Delete");
                        })
                        .click(on_delete_clicked)
                        .size(Val::Px(176.0), Val::Px(52.0));

                        ui.spawn(Node {
                            height: Val::Px(8.0),
                            ..default()
                        });

                        ui.button(cruft_ui::UiButtonVariant::Ghost, |ui| {
                            ui.label("Back");
                        })
                        .click(on_back_clicked)
                        .size(Val::Px(176.0), Val::Px(44.0));
                    });
                });
            })
            .size(Val::Auto, Val::Auto);
        });

    let Some(container) = list_container_entity else {
        return;
    };

    commands.entity(container).with_children(|parent| {
        let mut ui = UiBuilder::new(parent, &theme);
        for meta in index.items.iter() {
            ui.button(cruft_ui::UiButtonVariant::Secondary, |ui| {
                ui.label_semibold(&meta.display_name);
            })
            .insert(SaveEntryId(meta.id.clone()))
            .click(on_save_entry_clicked)
            .size(Val::Percent(100.0), Val::Px(44.0));
        }
    });
}

fn rebuild_save_list(
    mut commands: Commands,
    theme: Res<cruft_ui::UiTheme>,
    index: Res<SaveIndex>,
    state: Res<SaveSelectState>,
    container: Query<Entity, With<SaveListContainer>>,
    items: Query<Entity, With<SaveEntryId>>,
) {
    let Ok(container) = container.single() else {
        return;
    };

    for entity in &items {
        commands.entity(entity).try_despawn();
    }

    commands.entity(container).with_children(|parent| {
        let mut ui = UiBuilder::new(parent, &theme);
        for meta in index.items.iter() {
            let id = meta.id.clone();
            let mut button = ui.button(cruft_ui::UiButtonVariant::Secondary, |ui| {
                ui.label_semibold(&meta.display_name);
            });
            button.insert(SaveEntryId(id.clone()));
            button.click(on_save_entry_clicked);
            button.size(Val::Percent(100.0), Val::Px(44.0));

            if state.selected.as_deref() == Some(id.as_str()) {
                button.styles(cruft_ui::UiButtonStyleOverride {
                    bg: Some(theme.accent),
                    fg: None,
                    border: None,
                    radius: None,
                });
            }
        }
    });
}

fn update_selection_styles(
    mut commands: Commands,
    theme: Res<cruft_ui::UiTheme>,
    state: Res<SaveSelectState>,
    items: Query<(Entity, &SaveEntryId)>,
) {
    for (entity, id) in &items {
        if state.selected.as_deref() == Some(id.0.as_str()) {
            commands
                .entity(entity)
                .insert(cruft_ui::UiButtonStyleOverride {
                    bg: Some(theme.accent),
                    fg: None,
                    border: None,
                    radius: None,
                });
        } else {
            commands
                .entity(entity)
                .remove::<cruft_ui::UiButtonStyleOverride>();
        }
    }
}

fn sync_modal(
    mut commands: Commands,
    theme: Res<cruft_ui::UiTheme>,
    state: ResMut<SaveSelectState>,
    mut spawned: ResMut<SpawnedModalKind>,
    root: Query<Entity, With<SaveSelectRoot>>,
    existing_modal: Query<Entity, With<SaveSelectModalRoot>>,
    index: Res<SaveIndex>,
) {
    let Ok(root) = root.single() else {
        return;
    };

    if spawned.0 == state.modal {
        return;
    }

    for entity in &existing_modal {
        commands.entity(entity).try_despawn();
    }

    spawned.0 = state.modal;

    let Some(kind) = state.modal else {
        return;
    };

    match kind {
        ModalKind::New => spawn_new_modal(&mut commands, root, &theme),
        ModalKind::Rename => spawn_rename_modal(&mut commands, root, &theme, &state, &index),
        ModalKind::ConfirmDelete => spawn_delete_modal(&mut commands, root, &theme, &state),
    }
}

fn spawn_new_modal(commands: &mut Commands, root: Entity, theme: &cruft_ui::UiTheme) {
    commands.entity(root).with_children(|parent| {
        let mut ui = UiBuilder::new(parent, theme);
        ui.modal(|ui| {
            ui.card(|ui| {
                ui.label_semibold("New Save");
                ui.spawn(Node {
                    height: Val::Px(12.0),
                    ..default()
                });

                let mut input = ui.text_input("", "Save name", Val::Px(320.0));
                input.insert(SaveSelectModalTextInput);
                input.observe(on_new_submit);
                input.observe(on_modal_cancel);

                ui.spawn(Node {
                    height: Val::Px(16.0),
                    ..default()
                });
                ui.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(12.0),
                    ..default()
                })
                .with_children(|p| {
                    let mut ui = UiBuilder::new(p, theme);
                    ui.button(cruft_ui::UiButtonVariant::Primary, |ui| {
                        ui.label("Create");
                    })
                    .click(on_new_confirm_clicked)
                    .size(Val::Px(120.0), Val::Px(44.0));
                    ui.button(cruft_ui::UiButtonVariant::Secondary, |ui| {
                        ui.label("Cancel");
                    })
                    .click(on_modal_cancel_clicked)
                    .size(Val::Px(120.0), Val::Px(44.0));
                });
            })
            .size(Val::Px(420.0), Val::Auto);
        })
        .insert((
            SaveSelectModalRoot,
            DespawnOnExit(FrontEndState::SaveSelect),
        ));
    });
}

fn spawn_rename_modal(
    commands: &mut Commands,
    root: Entity,
    theme: &cruft_ui::UiTheme,
    state: &SaveSelectState,
    index: &SaveIndex,
) {
    let Some(id) = state.selected.as_deref() else {
        return;
    };
    let initial = index
        .items
        .iter()
        .find(|m| m.id == id)
        .map(|m| m.display_name.clone())
        .unwrap_or_default();

    commands.entity(root).with_children(|parent| {
        let mut ui = UiBuilder::new(parent, theme);
        ui.modal(|ui| {
            ui.card(|ui| {
                ui.label_semibold("Rename Save");
                ui.spawn(Node {
                    height: Val::Px(12.0),
                    ..default()
                });

                let mut input = ui.text_input(initial, "New name", Val::Px(320.0));
                input.insert(SaveSelectModalTextInput);
                input.observe(on_rename_submit);
                input.observe(on_modal_cancel);

                ui.spawn(Node {
                    height: Val::Px(16.0),
                    ..default()
                });
                ui.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(12.0),
                    ..default()
                })
                .with_children(|p| {
                    let mut ui = UiBuilder::new(p, theme);
                    ui.button(cruft_ui::UiButtonVariant::Primary, |ui| {
                        ui.label("Save");
                    })
                    .click(on_rename_confirm_clicked)
                    .size(Val::Px(120.0), Val::Px(44.0));
                    ui.button(cruft_ui::UiButtonVariant::Secondary, |ui| {
                        ui.label("Cancel");
                    })
                    .click(on_modal_cancel_clicked)
                    .size(Val::Px(120.0), Val::Px(44.0));
                });
            })
            .size(Val::Px(420.0), Val::Auto);
        })
        .insert((
            SaveSelectModalRoot,
            DespawnOnExit(FrontEndState::SaveSelect),
        ));
    });
}

fn spawn_delete_modal(
    commands: &mut Commands,
    root: Entity,
    theme: &cruft_ui::UiTheme,
    state: &SaveSelectState,
) {
    if state.selected.is_none() {
        return;
    }

    commands.entity(root).with_children(|parent| {
        let mut ui = UiBuilder::new(parent, theme);
        ui.modal(|ui| {
            ui.card(|ui| {
                ui.label_semibold("Delete Save?");
                ui.spawn(Node {
                    height: Val::Px(12.0),
                    ..default()
                });
                ui.label("This will move the save to .trash.");

                ui.spawn(Node {
                    height: Val::Px(16.0),
                    ..default()
                });
                ui.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(12.0),
                    ..default()
                })
                .with_children(|p| {
                    let mut ui = UiBuilder::new(p, theme);
                    ui.button(cruft_ui::UiButtonVariant::Primary, |ui| {
                        ui.label("Delete");
                    })
                    .click(on_delete_confirm_clicked)
                    .size(Val::Px(120.0), Val::Px(44.0));
                    ui.button(cruft_ui::UiButtonVariant::Secondary, |ui| {
                        ui.label("Cancel");
                    })
                    .click(on_modal_cancel_clicked)
                    .size(Val::Px(120.0), Val::Px(44.0));
                });
            })
            .size(Val::Px(420.0), Val::Auto);
        })
        .insert((
            SaveSelectModalRoot,
            DespawnOnExit(FrontEndState::SaveSelect),
        ));
    });
}

fn on_save_entry_clicked(
    ev: On<cruft_ui::UiClick>,
    mut state: ResMut<SaveSelectState>,
    ids: Query<&SaveEntryId>,
) {
    if let Ok(id) = ids.get(ev.entity) {
        state.selected = Some(id.0.clone());
    }
}

fn on_load_clicked(
    _ev: On<cruft_ui::UiClick>,
    state: Res<SaveSelectState>,
    mut flow: MessageWriter<FlowRequest>,
) {
    let Some(id) = state.selected.as_ref() else {
        return;
    };
    flow.write(FlowRequest::StartLoadSave(id.clone()));
}

fn on_new_clicked(_ev: On<cruft_ui::UiClick>, mut state: ResMut<SaveSelectState>) {
    state.modal = Some(ModalKind::New);
}

fn on_rename_clicked(_ev: On<cruft_ui::UiClick>, mut state: ResMut<SaveSelectState>) {
    if state.selected.is_some() {
        state.modal = Some(ModalKind::Rename);
    }
}

fn on_copy_clicked(
    _ev: On<cruft_ui::UiClick>,
    state: Res<SaveSelectState>,
    mut ops: MessageWriter<SaveOpRequest>,
) {
    let Some(id) = state.selected.as_ref() else {
        return;
    };
    ops.write(SaveOpRequest::Copy {
        id: SaveId(id.clone()),
    });
}

fn on_delete_clicked(_ev: On<cruft_ui::UiClick>, mut state: ResMut<SaveSelectState>) {
    if state.selected.is_some() {
        state.modal = Some(ModalKind::ConfirmDelete);
    }
}

fn on_back_clicked(_ev: On<cruft_ui::UiClick>, mut flow: MessageWriter<FlowRequest>) {
    flow.write(FlowRequest::EnterFrontEnd);
}

fn on_modal_cancel_clicked(_ev: On<cruft_ui::UiClick>, mut state: ResMut<SaveSelectState>) {
    state.modal = None;
}

fn on_modal_cancel(ev: On<cruft_ui::UiCancel>, mut state: ResMut<SaveSelectState>) {
    let _ = ev;
    state.modal = None;
}

fn read_modal_input(
    inputs: &Query<&cruft_ui::UiTextInput, With<SaveSelectModalTextInput>>,
) -> Option<String> {
    let Ok(input) = inputs.single() else {
        return None;
    };
    let v = input.value.trim().to_string();
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

fn on_new_confirm_clicked(
    _ev: On<cruft_ui::UiClick>,
    mut flow: MessageWriter<FlowRequest>,
    mut state: ResMut<SaveSelectState>,
    inputs: Query<&cruft_ui::UiTextInput, With<SaveSelectModalTextInput>>,
) {
    if let Some(name) = read_modal_input(&inputs) {
        flow.write(FlowRequest::StartNewSave { display_name: name });
        state.modal = None;
    }
}

fn on_new_submit(
    ev: On<cruft_ui::UiSubmit>,
    mut flow: MessageWriter<FlowRequest>,
    mut state: ResMut<SaveSelectState>,
    inputs: Query<&cruft_ui::UiTextInput>,
) {
    if let Ok(input) = inputs.get(ev.entity) {
        let name = input.value.trim().to_string();
        if !name.is_empty() {
            flow.write(FlowRequest::StartNewSave { display_name: name });
            state.modal = None;
        }
    }
}

fn on_rename_confirm_clicked(
    _ev: On<cruft_ui::UiClick>,
    mut ops: MessageWriter<SaveOpRequest>,
    mut state: ResMut<SaveSelectState>,
    inputs: Query<&cruft_ui::UiTextInput, With<SaveSelectModalTextInput>>,
) {
    let Some(id) = state.selected.as_ref() else {
        state.modal = None;
        return;
    };
    if let Some(new_name) = read_modal_input(&inputs) {
        ops.write(SaveOpRequest::Rename {
            id: SaveId(id.clone()),
            new_name,
        });
    }
    state.modal = None;
}

fn on_rename_submit(
    ev: On<cruft_ui::UiSubmit>,
    mut ops: MessageWriter<SaveOpRequest>,
    mut state: ResMut<SaveSelectState>,
    inputs: Query<&cruft_ui::UiTextInput>,
) {
    let Some(id) = state.selected.as_ref() else {
        state.modal = None;
        return;
    };
    if let Ok(input) = inputs.get(ev.entity) {
        let new_name = input.value.trim().to_string();
        if !new_name.is_empty() {
            ops.write(SaveOpRequest::Rename {
                id: SaveId(id.clone()),
                new_name,
            });
        }
    }
    state.modal = None;
}

fn on_delete_confirm_clicked(
    _ev: On<cruft_ui::UiClick>,
    mut ops: MessageWriter<SaveOpRequest>,
    mut state: ResMut<SaveSelectState>,
) {
    let Some(id) = state.selected.as_ref() else {
        state.modal = None;
        return;
    };
    ops.write(SaveOpRequest::Delete {
        id: SaveId(id.clone()),
    });
    state.modal = None;
}

fn handle_save_results(
    mut reader: MessageReader<SaveOpResult>,
    mut state: ResMut<SaveSelectState>,
) {
    for msg in reader.read() {
        match msg {
            SaveOpResult::Copied { meta } | SaveOpResult::Created { meta } => {
                state.selected = Some(meta.id.clone());
            }
            SaveOpResult::Deleted { id } => {
                if state.selected.as_deref() == Some(id.0.as_str()) {
                    state.selected = None;
                }
            }
            _ => {}
        }
    }
}
