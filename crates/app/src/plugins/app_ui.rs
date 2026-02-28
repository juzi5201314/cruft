//! App 侧 UI（loading / main menu）插件：把 UI 状态机与 UI 树构建从 main 解耦出来。

use bevy::prelude::*;

use cruft_ui::ui::{UiBuilder, UiEntityCommandsExt};

pub struct AppUiPlugin;

impl Plugin for AppUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(cruft_ui::CruftUiPlugin)
            .init_state::<AppScreen>()
            .add_systems(OnEnter(AppScreen::Loading), spawn_loading_screen)
            .add_systems(Update, tick_loading.run_if(in_state(AppScreen::Loading)))
            .add_systems(OnExit(AppScreen::Loading), despawn_screen_roots)
            .add_systems(OnEnter(AppScreen::MainMenu), spawn_main_menu)
            .add_systems(OnExit(AppScreen::MainMenu), despawn_screen_roots);
    }
}

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
enum AppScreen {
    #[default]
    Loading,
    MainMenu,
}

#[derive(Component)]
struct ScreenRoot;

#[derive(Component)]
struct LoadingProgressBar;

#[derive(Resource)]
struct LoadingDelay(Timer);

fn spawn_loading_screen(
    mut commands: Commands,
    theme: Res<cruft_ui::UiTheme>,
    mut ui_materials: ResMut<Assets<cruft_ui::GeistGridMaterial>>,
) {
    commands.insert_resource(LoadingDelay(Timer::from_seconds(1.0, TimerMode::Once)));

    spawn_grid_background(&mut commands, &mut ui_materials);

    // UI Content (Loading)
    commands
        .spawn((
            ScreenRoot,
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

                ui.progress(0.0, Val::Px(280.0)).insert(LoadingProgressBar);

                ui.spawn(Node {
                    height: Val::Px(10.0),
                    ..default()
                });

                ui.label("Initializing assets & textures");
            })
            .size(Val::Px(360.0), Val::Auto);
        });
}

fn tick_loading(
    time: Res<Time>,
    mut delay: ResMut<LoadingDelay>,
    mut progress: Query<&mut cruft_ui::UiProgress, With<LoadingProgressBar>>,
    mut next: ResMut<NextState<AppScreen>>,
) {
    delay.0.tick(time.delta());
    let t = delay.0.fraction().clamp(0.0, 1.0);

    for mut p in &mut progress {
        p.value = t;
    }

    if delay.0.is_finished() {
        next.set(AppScreen::MainMenu);
    }
}

fn spawn_main_menu(
    mut commands: Commands,
    theme: Res<cruft_ui::UiTheme>,
    mut ui_materials: ResMut<Assets<cruft_ui::GeistGridMaterial>>,
) {
    spawn_grid_background(&mut commands, &mut ui_materials);

    commands
        .spawn((
            ScreenRoot,
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
                    ui.spawn(Node {
                        width: Val::Percent(100.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|p| {
                        let mut ui = UiBuilder::new(p, &theme);
                        ui.label_semibold("Cruft");
                    });

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
                            ui.icon('\u{e9b2}'); // Play icon
                            ui.label("Start");
                        })
                        .size(Val::Px(160.0), Val::Px(60.0));

                        ui.button(cruft_ui::UiButtonVariant::Secondary, |ui| {
                            ui.icon('\u{e9bb}'); // Menu icon
                            ui.label("Menu");
                        })
                        .size(Val::Px(160.0), Val::Px(60.0));
                    });
                });
            })
            .size(Val::Px(360.0), Val::Auto);
        });
}

fn spawn_grid_background(
    commands: &mut Commands,
    ui_materials: &mut Assets<cruft_ui::GeistGridMaterial>,
) {
    commands.spawn((
        ScreenRoot,
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

fn despawn_screen_roots(
    mut commands: Commands,
    roots: Query<Entity, (With<ScreenRoot>, Without<ChildOf>)>,
) {
    for root in &roots {
        commands.entity(root).despawn();
    }
}
