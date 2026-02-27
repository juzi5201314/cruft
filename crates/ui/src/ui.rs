use bevy::ecs::system::IntoObserverSystem;
use bevy::prelude::*;

use crate::components::{
    UiButton, UiButtonLabel, UiButtonStyleOverride, UiButtonVariant, UiCard, UiProgress,
    UiProgressFill, UiResponsiveFlex,
};
use crate::events::UiClick;
use crate::theme::UiTheme;

/// UI Builder context that holds the Spawner and Theme.
pub struct UiBuilder<'a, 'w, 't> {
    pub parent: &'a mut ChildSpawnerCommands<'w>,
    pub theme: &'t UiTheme,
}

impl<'a, 'w, 't> UiBuilder<'a, 'w, 't> {
    pub fn new(parent: &'a mut ChildSpawnerCommands<'w>, theme: &'t UiTheme) -> Self {
        Self { parent, theme }
    }

    pub fn theme(&self) -> &UiTheme {
        self.theme
    }

    /// Spawns a raw node but within the current context.
    pub fn spawn(&mut self, bundle: impl Bundle) -> EntityCommands<'_> {
        self.parent.spawn(bundle)
    }

    /// Card container (Composable).
    pub fn card<F>(&mut self, build_children: F) -> EntityCommands<'_>
    where
        F: for<'c, 'w2> FnOnce(&mut UiBuilder<'c, 'w2, 't>),
    {
        let radius = self.theme.radius;
        let theme = self.theme;
        let mut entity = self.parent.spawn((
            UiCard,
            Node {
                padding: UiRect::all(px(32.0)),
                border: UiRect::all(px(1.0)),
                border_radius: BorderRadius::all(px(radius)),
                flex_direction: FlexDirection::Column,
                ..default()
            },
        ));
        entity.with_children(|p| {
            let mut child_ui = UiBuilder::new(p, theme);
            build_children(&mut child_ui);
        });
        entity
    }

    /// Button container (Composable).
    pub fn button<F>(&mut self, variant: UiButtonVariant, build_children: F) -> EntityCommands<'_>
    where
        F: for<'c, 'w2> FnOnce(&mut UiBuilder<'c, 'w2, 't>),
    {
        let radius = self.theme.radius;
        let theme = self.theme;
        let mut entity = self.parent.spawn((
            Button,
            UiButton { variant },
            Node {
                padding: UiRect::axes(px(20.0), px(12.0)),
                border: UiRect::all(px(0.0)),
                border_radius: BorderRadius::all(px(radius)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Outline::new(px(2.0), px(2.0), Color::NONE),
        ));
        entity.with_children(|p| {
            let mut child_ui = UiBuilder::new(p, theme);
            build_children(&mut child_ui);
        });
        entity
    }

    /// Atomic label using the theme's foreground color.
    pub fn label(&mut self, text: impl Into<String>) -> EntityCommands<'_> {
        let fg = self.theme.fg;
        self.parent.spawn((
            UiButtonLabel,
            Text::new(text),
            TextFont {
                font_size: 14.0,
                ..default()
            },
            TextColor(fg),
        ))
    }

    /// Progress bar (Atomic root).
    pub fn progress(&mut self, value: f32, width: Val) -> EntityCommands<'_> {
        let mut root = self.parent.spawn((
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
            p.spawn((
                UiProgressFill,
                Node {
                    width: Val::Percent(value.clamp(0.0, 1.0) * 100.0),
                    height: Val::Percent(100.0),
                    border_radius: BorderRadius::all(px(4.0)),
                    ..default()
                },
            ));
        });
        root
    }
}

/// Extension trait for EntityCommands to provide Geist-style modifiers.
pub trait UiEntityCommandsExt {
    fn size(&mut self, width: Val, height: Val) -> &mut Self;
    fn styles(&mut self, styles: UiButtonStyleOverride) -> &mut Self;
    fn click<B: Bundle, M>(&mut self, handler: impl IntoObserverSystem<UiClick, B, M>) -> &mut Self;
    fn responsive_flex(
        &mut self,
        breakpoint_px: f32,
        narrow: FlexDirection,
        wide: FlexDirection,
    ) -> &mut Self;
}

impl<'a> UiEntityCommandsExt for EntityCommands<'a> {
    fn size(&mut self, width: Val, height: Val) -> &mut Self {
        self.entry::<Node>()
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

    fn styles(&mut self, styles: UiButtonStyleOverride) -> &mut Self {
        self.insert(styles)
    }

    fn click<B: Bundle, M>(
        &mut self,
        handler: impl IntoObserverSystem<UiClick, B, M>,
    ) -> &mut Self {
        self.observe(handler)
    }

    fn responsive_flex(
        &mut self,
        breakpoint_px: f32,
        narrow: FlexDirection,
        wide: FlexDirection,
    ) -> &mut Self {
        self.insert(UiResponsiveFlex {
            breakpoint_px,
            narrow,
            wide,
        })
    }
}

fn px(v: f32) -> Val {
    Val::Px(v)
}
