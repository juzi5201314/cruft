use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::input::keyboard::{KeyboardInput, KeyCode};
use bevy::input::ButtonState;

use crate::components::{
    UiButton, UiButtonLabel, UiButtonStyleOverride, UiButtonVariant, UiCard, UiProgress,
    UiFocus, UiProgressFill, UiResponsiveFlex, UiTextInput, UiTextInputValueText,
};
use crate::events::{UiCancel, UiClick, UiSubmit};
use crate::theme::UiTheme;

/// Cruft 默认 UI 插件：语义组件 + Geist 皮肤 + Observers 点击事件。
pub struct CruftUiPlugin;

impl Plugin for CruftUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(UiMaterialPlugin::<GeistGridMaterial>::default())
            .init_resource::<UiTheme>()
            .init_resource::<UiFocus>()
            .add_systems(
                Update,
                (
                    emit_ui_click,
                    update_text_input_focus,
                    handle_text_input_keyboard,
                    apply_geist_button_skin,
                    apply_geist_card_skin,
                    apply_geist_progress_skin,
                    apply_geist_text_input_skin,
                    update_progress_fill,
                    update_responsive_flex,
                    sync_text_input_value_text,
                ),
            );
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone, Copy)]
pub struct GeistGridMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    #[uniform(0)]
    pub grid_color: LinearRgba,
    #[uniform(0)]
    pub spacing: f32,
    #[uniform(0)]
    pub thickness: f32,
}

impl UiMaterial for GeistGridMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/geist_grid.wgsl".into()
    }
}

fn emit_ui_click(
    mut commands: Commands,
    query: Query<(Entity, &Interaction), (With<UiButton>, Changed<Interaction>)>,
) {
    for (entity, interaction) in &query {
        if *interaction == Interaction::Pressed {
            commands.trigger(UiClick { entity });
        }
    }
}

fn apply_geist_button_skin(
    mut commands: Commands,
    theme: Res<UiTheme>,
    mut buttons: Query<(
        Entity,
        &UiButton,
        &Interaction,
        &mut Node,
        Option<&mut BackgroundColor>,
        Option<&mut BorderColor>,
        Option<&mut Outline>,
        Option<&UiButtonStyleOverride>,
        &Children,
    )>,
    mut labels: Query<&mut TextColor, With<UiButtonLabel>>,
) {
    for (entity, button, interaction, mut node, bg, border, outline, styles, children) in
        &mut buttons
    {
        let (mut bg_color, mut fg_color, mut border_color, border_width) = match button.variant {
            UiButtonVariant::Primary => (theme.primary_bg, theme.primary_fg, Color::NONE, 0.0),
            UiButtonVariant::Secondary => {
                (theme.secondary_bg, theme.secondary_fg, theme.border, 1.0)
            }
            UiButtonVariant::Ghost => (Color::NONE, theme.fg, Color::NONE, 0.0),
        };

        match *interaction {
            Interaction::Pressed => {
                bg_color = bg_color.with_alpha(0.85);
            }
            Interaction::Hovered => match button.variant {
                UiButtonVariant::Primary => bg_color = bg_color.with_alpha(0.95),
                UiButtonVariant::Secondary | UiButtonVariant::Ghost => bg_color = theme.accent,
            },
            Interaction::None => {}
        }

        let radius = styles.and_then(|s| s.radius).unwrap_or(theme.radius);
        node.border_radius = BorderRadius::all(Val::Px(radius));
        node.border = UiRect::all(Val::Px(border_width));

        if let Some(styles) = styles {
            if let Some(bg) = styles.bg {
                bg_color = bg;
            }
            if let Some(fg) = styles.fg {
                fg_color = fg;
            }
            if let Some(border) = styles.border {
                border_color = border;
                node.border = UiRect::all(Val::Px(1.0));
            }
        }

        if let Some(mut bg) = bg {
            bg.0 = bg_color;
        } else {
            commands.entity(entity).insert(BackgroundColor(bg_color));
        }

        if let Some(mut border) = border {
            *border = BorderColor::all(border_color);
        } else {
            commands
                .entity(entity)
                .insert(BorderColor::all(border_color));
        }

        if let Some(mut outline) = outline {
            outline.color = if *interaction == Interaction::Hovered {
                theme.fg.with_alpha(0.15)
            } else {
                Color::NONE
            };
        }

        for child in children.iter() {
            if let Ok(mut text_color) = labels.get_mut(child) {
                text_color.0 = fg_color;
            }
        }
    }
}

fn apply_geist_card_skin(
    mut commands: Commands,
    theme: Res<UiTheme>,
    mut cards: Query<
        (
            Entity,
            &mut Node,
            Option<&mut BackgroundColor>,
            Option<&mut BorderColor>,
            Option<&BoxShadow>,
        ),
        With<UiCard>,
    >,
) {
    let theme_dirty = theme.is_changed();

    for (entity, mut node, bg, border, shadow) in &mut cards {
        let needs_init = bg.is_none() || border.is_none() || shadow.is_none();
        if !needs_init && !theme_dirty {
            continue;
        }

        node.border_radius = BorderRadius::all(Val::Px(theme.radius));

        if let Some(mut bg) = bg {
            bg.0 = theme.bg;
        } else {
            commands.entity(entity).insert(BackgroundColor(theme.bg));
        }

        if let Some(mut border) = border {
            *border = BorderColor::all(theme.border);
        } else {
            commands
                .entity(entity)
                .insert(BorderColor::all(theme.border));
        }

        if shadow.is_none() || theme_dirty {
            commands.entity(entity).insert(BoxShadow::new(
                Color::srgba(0.0, 0.0, 0.0, 0.05),
                Val::Px(0.0),
                Val::Px(1.0),
                Val::Px(2.0),
                Val::Px(0.0),
            ));
        }
    }
}

fn apply_geist_progress_skin(
    mut commands: Commands,
    theme: Res<UiTheme>,
    progresses: Query<(Entity, &Children, Option<&BackgroundColor>), With<UiProgress>>,
    fills: Query<Option<&BackgroundColor>, With<UiProgressFill>>,
) {
    let theme_dirty = theme.is_changed();

    for (entity, children, bg) in &progresses {
        if bg.is_none() || theme_dirty {
            commands
                .entity(entity)
                .insert(BackgroundColor(theme.border));
        }
        for child in children.iter() {
            if let Ok(fill_bg) = fills.get(child) {
                if fill_bg.is_none() || theme_dirty {
                    commands
                        .entity(child)
                        .insert(BackgroundColor(theme.primary_bg));
                }
            }
        }
    }
}

fn update_progress_fill(
    progresses: Query<(&UiProgress, &Children), Changed<UiProgress>>,
    mut nodes: Query<&mut Node>,
    fills: Query<(), With<UiProgressFill>>,
) {
    for (progress, children) in &progresses {
        for child in children.iter() {
            if !fills.contains(child) {
                continue;
            }
            if let Ok(mut node) = nodes.get_mut(child) {
                node.width = Val::Percent(progress.value.clamp(0.0, 1.0) * 100.0);
            }
        }
    }
}

fn update_responsive_flex(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut query: Query<(&UiResponsiveFlex, &mut Node)>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let width = window.resolution.width();

    for (responsive, mut node) in &mut query {
        let target = if width < responsive.breakpoint_px {
            responsive.narrow
        } else {
            responsive.wide
        };
        if node.flex_direction != target {
            node.flex_direction = target;
        }
    }
}

fn update_text_input_focus(
    mut focus: ResMut<UiFocus>,
    inputs: Query<(Entity, &Interaction), (With<UiTextInput>, Changed<Interaction>)>,
) {
    for (entity, interaction) in &inputs {
        if *interaction == Interaction::Pressed {
            focus.0 = Some(entity);
        }
    }
}

fn handle_text_input_keyboard(
    mut commands: Commands,
    mut reader: MessageReader<KeyboardInput>,
    mut focus: ResMut<UiFocus>,
    mut inputs: Query<&mut UiTextInput>,
) {
    let Some(focused) = focus.0 else {
        return;
    };

    let Ok(mut input) = inputs.get_mut(focused) else {
        focus.0 = None;
        return;
    };

    for ev in reader.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }

        match ev.key_code {
            KeyCode::Enter => {
                commands.trigger(UiSubmit { entity: focused });
                focus.0 = None;
                break;
            }
            KeyCode::Escape => {
                commands.trigger(UiCancel { entity: focused });
                focus.0 = None;
                break;
            }
            KeyCode::Backspace => {
                input.value.pop();
            }
            _ => {
                if let Some(text) = &ev.text {
                    input.value.push_str(text.as_str());
                }
            }
        }
    }
}

fn apply_geist_text_input_skin(
    mut commands: Commands,
    theme: Res<UiTheme>,
    focus: Res<UiFocus>,
    mut inputs: Query<(
        Entity,
        &Interaction,
        &mut Node,
        Option<&mut BackgroundColor>,
        Option<&mut BorderColor>,
        Option<&mut Outline>,
    ), With<UiTextInput>>,
) {
    let theme_dirty = theme.is_changed() || focus.is_changed();

    for (entity, interaction, mut node, bg, border, outline) in &mut inputs {
        let focused = focus.0 == Some(entity);
        let needs_init = bg.is_none() || border.is_none() || outline.is_none();
        if !needs_init && !theme_dirty && *interaction == Interaction::None {
            continue;
        }

        node.border_radius = BorderRadius::all(Val::Px(theme.radius));
        node.border = UiRect::all(Val::Px(1.0));

        let mut bg_color = theme.secondary_bg;
        if *interaction == Interaction::Hovered {
            bg_color = theme.accent;
        }

        let border_color = theme.border;
        let outline_color = if focused {
            theme.fg.with_alpha(0.15)
        } else if *interaction == Interaction::Hovered {
            theme.fg.with_alpha(0.08)
        } else {
            Color::NONE
        };

        if let Some(mut bg) = bg {
            bg.0 = bg_color;
        } else {
            commands.entity(entity).insert(BackgroundColor(bg_color));
        }

        if let Some(mut border) = border {
            *border = BorderColor::all(border_color);
        } else {
            commands.entity(entity).insert(BorderColor::all(border_color));
        }

        if let Some(mut outline) = outline {
            outline.color = outline_color;
        } else {
            commands
                .entity(entity)
                .insert(Outline::new(Val::Px(2.0), Val::Px(2.0), outline_color));
        }
    }
}

fn sync_text_input_value_text(
    theme: Res<UiTheme>,
    inputs: Query<(&UiTextInput, &Children), Or<(Changed<UiTextInput>, Added<UiTextInput>)>>,
    mut text_query: Query<(&mut Text, &mut TextColor), With<UiTextInputValueText>>,
) {
    for (input, children) in &inputs {
        for child in children.iter() {
            if let Ok((mut text, mut color)) = text_query.get_mut(child) {
                if input.value.is_empty() {
                    text.0 = input.placeholder.clone();
                    color.0 = theme.muted_fg;
                } else {
                    text.0 = input.value.clone();
                    color.0 = theme.fg;
                }
            }
        }
    }
}
