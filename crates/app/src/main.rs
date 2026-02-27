use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy_embedded_assets::{EmbeddedAssetPlugin, PluginMode};

mod plugins;

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

fn main() {
    App::new()
        .add_plugins(EmbeddedAssetPlugin {
            mode: PluginMode::ReplaceDefault,
        })
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: "assets".to_string(),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins(plugins::ProceduralTexturePlugin)
        .add_plugins(cruft_ui::CruftUiPlugin)
        .init_state::<AppScreen>()
        .add_systems(OnEnter(AppScreen::Loading), spawn_loading_screen)
        .add_systems(Update, tick_loading.run_if(in_state(AppScreen::Loading)))
        .add_systems(OnExit(AppScreen::Loading), despawn_screen_roots)
        .add_systems(OnEnter(AppScreen::MainMenu), spawn_main_menu)
        .add_systems(OnExit(AppScreen::MainMenu), despawn_screen_roots)
        .run();
}

fn spawn_loading_screen(
    mut commands: Commands,
    theme: Res<cruft_ui::UiTheme>,
    mut ui_materials: ResMut<Assets<cruft_ui::GeistGridMaterial>>,
) {
    commands.insert_resource(LoadingDelay(Timer::from_seconds(1.0, TimerMode::Once)));

    // Grid background
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
            cruft_ui::ui::card(parent, &theme)
                .size(Val::Px(420.0), Val::Auto)
                .with_children(|p, theme| {
                    p.spawn((
                        Text::new("Loading…"),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(theme.fg),
                    ));

                    p.spawn(Node {
                        height: Val::Px(16.0),
                        ..default()
                    });

                    cruft_ui::ui::progress(p, theme, 0.0, Val::Px(360.0))
                        .insert(LoadingProgressBar);

                    p.spawn(Node {
                        height: Val::Px(10.0),
                        ..default()
                    });

                    cruft_ui::ui::label(p, theme, "Initializing assets & textures");
                });
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
        // 开发示例：用 1s 的 timer 模拟加载进度
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
    // Grid background
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
            cruft_ui::ui::card(parent, &theme)
                .size(Val::Px(460.0), Val::Auto)
                .with_children(|p, theme| {
                    p.spawn((
                        Text::new("Cruft"),
                        TextFont {
                            font_size: 24.0,
                            ..default()
                        },
                        TextColor(theme.fg),
                    ));

                    p.spawn(Node {
                        height: Val::Px(16.0),
                        ..default()
                    });

                    let mut buttons = p.spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(12.0),
                        row_gap: Val::Px(12.0),
                        ..default()
                    });
                    cruft_ui::ui::responsive_flex(
                        &mut buttons,
                        520.0,
                        FlexDirection::Column,
                        FlexDirection::Row,
                    );

                    buttons.with_children(|row| {
                        cruft_ui::ui::button(row, theme)
                            .text("Start")
                            .variant(cruft_ui::UiButtonVariant::Primary);
                        cruft_ui::ui::button(row, theme)
                            .text("Menu")
                            .variant(cruft_ui::UiButtonVariant::Secondary);
                    });
                });
        });
}

fn despawn_screen_roots(
    mut commands: Commands,
    roots: Query<Entity, (With<ScreenRoot>, Without<ChildOf>)>,
) {
    for root in &roots {
        // Bevy v0.18 的 ChildOf/Children 关系会在父实体 despawn 时自动级联 despawn 子孙。
        commands.entity(root).despawn();
    }
}
