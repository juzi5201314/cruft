use bevy::prelude::*;

/// 当 UI Button 被按下时触发的事件（EntityEvent，target 为按钮实体）。
#[derive(EntityEvent, Debug, Clone, Copy)]
pub struct UiClick {
    pub entity: Entity,
}
