use bevy::prelude::*;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiButton {
    pub variant: UiButtonVariant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiButtonVariant {
    Primary,
    Secondary,
    Ghost,
}

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct UiButtonStyleOverride {
    pub bg: Option<Color>,
    pub fg: Option<Color>,
    pub border: Option<Color>,
    pub radius: Option<f32>,
}

#[derive(Component)]
pub struct UiButtonLabel;

#[derive(Component, Debug, Clone, Copy)]
pub struct UiProgress {
    pub value: f32, // 0.0..=1.0
}

#[derive(Component)]
pub struct UiProgressFill;

#[derive(Component)]
pub struct UiCard;

#[derive(Component, Debug, Clone, Copy)]
pub struct UiResponsiveFlex {
    pub breakpoint_px: f32,
    pub narrow: FlexDirection,
    pub wide: FlexDirection,
}

/// 最小文本输入框（用于存档重命名/新建命名等）。
#[derive(Component, Debug, Clone)]
pub struct UiTextInput {
    pub value: String,
    pub placeholder: String,
}

#[derive(Component)]
pub struct UiTextInputValueText;

#[derive(Component)]
pub struct UiModalOverlay;

/// 当前 UI focus（用于 TextInput）。
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct UiFocus(pub Option<Entity>);
