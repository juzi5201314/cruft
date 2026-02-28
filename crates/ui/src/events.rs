use bevy::prelude::*;

/// 当 UI Button 被按下时触发的事件（EntityEvent，target 为按钮实体）。
#[derive(EntityEvent, Debug, Clone, Copy)]
pub struct UiClick {
    pub entity: Entity,
}

/// 当 TextInput 提交（Enter）时触发。
#[derive(EntityEvent, Debug, Clone, Copy)]
pub struct UiSubmit {
    pub entity: Entity,
}

/// 当 TextInput 取消（Escape）时触发。
#[derive(EntityEvent, Debug, Clone, Copy)]
pub struct UiCancel {
    pub entity: Entity,
}
