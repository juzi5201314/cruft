use bevy::ecs::system::IntoObserverSystem;
use bevy::prelude::*;

use crate::components::{
    UiButton, UiButtonLabel, UiButtonStyleOverride, UiButtonVariant, UiCard, UiProgress,
    UiProgressFill, UiResponsiveFlex,
};
use crate::events::UiClick;
use crate::theme::UiTheme;

pub struct ButtonBuilder<'a> {
    button: EntityCommands<'a>,
    label: Entity,
    theme: &'a UiTheme,
}

impl<'a> ButtonBuilder<'a> {
    pub fn insert<C: Component>(mut self, component: C) -> Self {
        self.button.insert(component);
        self
    }

    pub fn size(mut self, width: Val, height: Val) -> Self {
        self.button
            .entry::<Node>()
            .and_modify(move |mut node| {
                node.width = width;
                node.height = height;
            })
            .or_insert(Node {
                width,
                height,
                ..default()
            });
        self
    }

    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.button
            .commands()
            .entity(self.label)
            .insert(Text::new(text));
        self
    }

    pub fn variant(mut self, variant: UiButtonVariant) -> Self {
        self.button.insert(UiButton { variant });
        self
    }

    pub fn styles(mut self, styles: UiButtonStyleOverride) -> Self {
        self.button.insert(styles);
        self
    }

    /// 绑定点击回调（Observer）。回调的第一个参数必须是 `On<UiClick>`。
    pub fn click<B: Bundle, M>(mut self, handler: impl IntoObserverSystem<UiClick, B, M>) -> Self {
        self.button.observe(handler);
        self
    }

    pub fn id(&self) -> Entity {
        self.button.id()
    }

    pub fn theme(&self) -> &UiTheme {
        self.theme
    }
}

pub fn button<'a>(parent: &'a mut ChildSpawnerCommands, theme: &'a UiTheme) -> ButtonBuilder<'a> {
    let mut label = Entity::PLACEHOLDER;
    let mut button = parent.spawn((
        Button,
        UiButton {
            variant: UiButtonVariant::Primary,
        },
        Node {
            padding: UiRect::axes(px(20.0), px(12.0)),
            border: UiRect::all(px(0.0)),
            border_radius: BorderRadius::all(px(theme.radius)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        Outline::new(px(2.0), px(2.0), Color::NONE),
    ));
    button.with_children(|p| {
        label = p
            .spawn((
                UiButtonLabel,
                Text::new(""),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(theme.fg),
            ))
            .id();
    });

    ButtonBuilder {
        button,
        label,
        theme,
    }
}

pub struct ProgressBuilder<'a> {
    root: EntityCommands<'a>,
    fill: Entity,
    theme: &'a UiTheme,
}

impl<'a> ProgressBuilder<'a> {
    pub fn insert<C: Component>(mut self, component: C) -> Self {
        self.root.insert(component);
        self
    }

    pub fn value(mut self, value: f32) -> Self {
        self.root.insert(UiProgress {
            value: value.clamp(0.0, 1.0),
        });
        self
    }

    pub fn styles(mut self, bg: Option<Color>, fill: Option<Color>) -> Self {
        if let Some(bg) = bg {
            self.root.insert(BackgroundColor(bg));
        }
        if let Some(fill) = fill {
            self.root
                .commands()
                .entity(self.fill)
                .insert(BackgroundColor(fill));
        }
        self
    }

    pub fn id(&self) -> Entity {
        self.root.id()
    }

    pub fn theme(&self) -> &UiTheme {
        self.theme
    }
}

pub fn progress<'a>(
    parent: &'a mut ChildSpawnerCommands,
    theme: &'a UiTheme,
    value: f32,
    width: Val,
) -> ProgressBuilder<'a> {
    let mut fill = Entity::PLACEHOLDER;
    let mut root = parent.spawn((
        UiProgress {
            value: value.clamp(0.0, 1.0),
        },
        Node {
            width,
            height: px(8.0),
            border_radius: BorderRadius::all(px(4.0)),
            overflow: Overflow::clip(),
            ..default()
        },
    ));
    root.with_children(|p| {
        fill = p
            .spawn((
                UiProgressFill,
                Node {
                    width: Val::Percent(value.clamp(0.0, 1.0) * 100.0),
                    height: Val::Percent(100.0),
                    border_radius: BorderRadius::all(px(4.0)),
                    ..default()
                },
            ))
            .id();
    });

    ProgressBuilder { root, fill, theme }
}

pub struct CardBuilder<'a> {
    card: EntityCommands<'a>,
    theme: &'a UiTheme,
}

impl<'a> CardBuilder<'a> {
    pub fn insert<C: Component>(mut self, component: C) -> Self {
        self.card.insert(component);
        self
    }

    pub fn size(mut self, width: Val, height: Val) -> Self {
        self.card
            .entry::<Node>()
            .and_modify(move |mut node| {
                node.width = width;
                node.height = height;
            })
            .or_insert(Node {
                width,
                height,
                ..default()
            });
        self
    }

    pub fn with_children(
        mut self,
        children: impl FnOnce(&mut ChildSpawnerCommands, &UiTheme),
    ) -> Self {
        let theme = self.theme;
        self.card.with_children(|p| children(p, theme));
        self
    }

    pub fn id(&self) -> Entity {
        self.card.id()
    }
}

pub fn card<'a>(parent: &'a mut ChildSpawnerCommands, theme: &'a UiTheme) -> CardBuilder<'a> {
    let card = parent.spawn((
        UiCard,
        Node {
            padding: UiRect::all(px(32.0)),
            border: UiRect::all(px(1.0)),
            border_radius: BorderRadius::all(px(theme.radius)),
            flex_direction: FlexDirection::Column,
            ..default()
        },
    ));

    CardBuilder { card, theme }
}

pub fn label(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    text: impl Into<String>,
) -> Entity {
    parent
        .spawn((
            Text::new(text),
            TextFont {
                font_size: 14.0,
                ..default()
            },
            TextColor(theme.muted_fg),
        ))
        .id()
}

pub fn responsive_flex(
    entity: &mut EntityCommands<'_>,
    breakpoint_px: f32,
    narrow: FlexDirection,
    wide: FlexDirection,
) {
    entity.insert(UiResponsiveFlex {
        breakpoint_px,
        narrow,
        wide,
    });
}

fn px(v: f32) -> Val {
    Val::Px(v)
}
