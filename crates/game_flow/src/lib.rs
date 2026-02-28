//! 游戏流程（状态机/请求/boot readiness）crate。

pub mod boot;
pub mod request;
pub mod state;

pub use boot::{BootProgress, BootReadiness, BootReady};
pub use request::FlowRequest;
pub use state::{
    AppState, FrontEndState, GameStartGeneration, GameStartKind, InGameState, PendingGameStart,
};

use bevy::prelude::*;

/// Flow 插件：安装状态机与请求事件，并提供唯一的状态切换写入点。
pub struct GameFlowPlugin;

impl Plugin for GameFlowPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<FlowRequest>()
            .init_state::<AppState>()
            .add_sub_state::<FrontEndState>()
            .add_sub_state::<InGameState>()
            .init_resource::<state::GameStartGeneration>()
            .init_resource::<boot::BootReadiness>()
            .init_resource::<boot::BootProgress>()
            .add_systems(
                Update,
                (
                    request::apply_flow_requests.in_set(request::FlowSet::Apply),
                    boot::update_boot_progress
                        .in_set(request::FlowSet::Boot)
                        .run_if(in_state(AppState::BootLoading)),
                )
                    .chain(),
            )
            .configure_sets(Update, (request::FlowSet::Apply, request::FlowSet::Boot).chain());
    }
}
