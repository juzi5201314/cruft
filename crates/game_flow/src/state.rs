use bevy::prelude::*;

/// 顶层应用状态：用于拆分“启动/前台菜单/游戏内”。
#[derive(States, Default, Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub enum AppState {
    /// 启动阶段：真实加载任务聚合（贴图、存档索引等）。
    #[default]
    BootLoading,
    /// 前台：主菜单/存档选择等。
    FrontEnd,
    /// 游戏内：加载存档/游戏中/暂停。
    InGame,
}

/// 前台子状态（仅在 `AppState::FrontEnd` 期间存在）。
#[derive(SubStates, Default, Clone, Copy, Eq, PartialEq, Hash, Debug)]
#[source(AppState = AppState::FrontEnd)]
#[states(scoped_entities)]
pub enum FrontEndState {
    #[default]
    MainMenu,
    SaveSelect,
}

/// 游戏内子状态（仅在 `AppState::InGame` 期间存在）。
#[derive(SubStates, Default, Clone, Copy, Eq, PartialEq, Hash, Debug)]
#[source(AppState = AppState::InGame)]
#[states(scoped_entities)]
pub enum InGameState {
    /// InGame 默认进入 Loading（通过 intent 决定 load/new 的具体任务）。
    #[default]
    Loading,
    Playing,
    Paused,
}

/// 一次进入 InGame 的意图：由 FrontEnd 触发，Loading 阶段读取并启动任务。
#[derive(Resource, Debug, Clone)]
pub struct PendingGameStart {
    pub generation: u64,
    pub kind: GameStartKind,
}

#[derive(Debug, Clone)]
pub enum GameStartKind {
    LoadSave(String),
    NewSave { display_name: String },
}

#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct GameStartGeneration(pub u64);

