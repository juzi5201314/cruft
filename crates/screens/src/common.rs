use bevy::prelude::*;
use bevy::state::state_scoped::DespawnOnExit;

pub fn spawn_grid_background<S: States>(
    commands: &mut Commands,
    ui_materials: &mut Assets<cruft_ui::GeistGridMaterial>,
    scope: S,
) {
    commands.spawn((
        DespawnOnExit(scope),
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            position_type: PositionType::Absolute,
            ..default()
        },
        MaterialNode(ui_materials.add(cruft_ui::GeistGridMaterial {
            color: LinearRgba::WHITE,
            grid_color: LinearRgba::from(Color::srgb(0.9, 0.9, 0.9)),
            spacing: 0.05,
            thickness: 0.01,
        })),
    ));
}

