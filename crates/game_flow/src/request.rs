use bevy::prelude::*;

use crate::state::{
    AppState, FrontEndState, GameStartGeneration, GameStartKind, InGameState, PendingGameStart,
};

/// UI/输入/系统发出的流程请求。
#[derive(Message, Debug, Clone)]
pub enum FlowRequest {
    EnterFrontEnd,
    EnterSaveSelect,
    StartLoadSave(String),
    StartNewSave { display_name: String },
    FinishGameLoading,
    TogglePause,
    Resume,
    QuitToMainMenu,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlowSet {
    Apply,
    Boot,
}

#[derive(Debug, Clone, Copy)]
pub struct FlowSnapshot {
    pub app: AppState,
    pub frontend: Option<FrontEndState>,
    pub ingame: Option<InGameState>,
    pub generation: u64,
}

#[derive(Debug, Default, Clone)]
pub struct FlowActions {
    pub set_app: Option<AppState>,
    pub set_frontend: Option<FrontEndState>,
    pub set_ingame: Option<InGameState>,
    pub start_game: Option<GameStartKind>,
    pub bump_generation: bool,
}

pub fn reduce(snapshot: FlowSnapshot, request: &FlowRequest) -> FlowActions {
    use AppState::*;
    use FlowRequest::*;

    let mut out = FlowActions::default();

    match request {
        EnterFrontEnd => {
            out.set_app = Some(FrontEnd);
            out.set_frontend = Some(FrontEndState::MainMenu);
        }
        EnterSaveSelect => {
            if snapshot.app == FrontEnd {
                out.set_frontend = Some(FrontEndState::SaveSelect);
            }
        }
        StartLoadSave(id) => {
            if snapshot.app == FrontEnd {
                out.bump_generation = true;
                out.start_game = Some(GameStartKind::LoadSave(id.clone()));
                out.set_app = Some(InGame);
                // InGameState 会随 SubStates 默认进入 Loading。
            }
        }
        StartNewSave { display_name } => {
            if snapshot.app == FrontEnd {
                out.bump_generation = true;
                out.start_game = Some(GameStartKind::NewSave {
                    display_name: display_name.clone(),
                });
                out.set_app = Some(InGame);
            }
        }
        FinishGameLoading => {
            if snapshot.app == InGame && snapshot.ingame == Some(InGameState::Loading) {
                out.set_ingame = Some(InGameState::Playing);
            }
        }
        TogglePause => {
            if snapshot.app == InGame {
                match snapshot.ingame {
                    Some(InGameState::Playing) => out.set_ingame = Some(InGameState::Paused),
                    Some(InGameState::Paused) => out.set_ingame = Some(InGameState::Playing),
                    _ => {}
                }
            }
        }
        Resume => {
            if snapshot.app == InGame && snapshot.ingame == Some(InGameState::Paused) {
                out.set_ingame = Some(InGameState::Playing);
            }
        }
        QuitToMainMenu => {
            out.set_app = Some(FrontEnd);
            out.set_frontend = Some(FrontEndState::MainMenu);
        }
    }

    out
}

/// 唯一写入 `NextState<_>` 的系统：按 request reducer 输出执行状态切换与 intent 写入。
pub fn apply_flow_requests(
    mut commands: Commands,
    mut requests: MessageReader<FlowRequest>,
    app: Res<State<AppState>>,
    frontend: Option<Res<State<FrontEndState>>>,
    ingame: Option<Res<State<InGameState>>>,
    mut next_app: ResMut<NextState<AppState>>,
    mut next_frontend: Option<ResMut<NextState<FrontEndState>>>,
    mut next_ingame: Option<ResMut<NextState<InGameState>>>,
    mut gen: ResMut<GameStartGeneration>,
) {
    let snapshot = FlowSnapshot {
        app: *app.get(),
        frontend: frontend.as_ref().map(|s| *s.get()),
        ingame: ingame.as_ref().map(|s| *s.get()),
        generation: gen.0,
    };

    let merged = resolve_requests(snapshot, requests.read());

    if merged.bump_generation {
        gen.0 = gen.0.saturating_add(1);
    }

    if let Some(kind) = merged.start_game {
        commands.insert_resource(PendingGameStart {
            generation: gen.0,
            kind,
        });
    }

    if let Some(s) = merged.set_app {
        next_app.set(s);
    }
    if let Some(s) = merged.set_frontend {
        if let Some(ref mut next) = next_frontend {
            next.set(s);
        }
    }
    if let Some(s) = merged.set_ingame {
        if let Some(ref mut next) = next_ingame {
            next.set(s);
        }
    }
}

fn resolve_requests<'a>(snapshot: FlowSnapshot, requests: impl IntoIterator<Item = &'a FlowRequest>) -> FlowActions {
    let mut current_snapshot = snapshot;
    let mut merged = FlowActions::default();

    for req in requests {
        let a = reduce(current_snapshot, req);
        if a.set_app.is_some() {
            merged.set_app = a.set_app;
            if merged.set_app != Some(AppState::InGame) {
                merged.start_game = None;
            }
        }
        if a.set_frontend.is_some() {
            merged.set_frontend = a.set_frontend;
        }
        if a.set_ingame.is_some() {
            merged.set_ingame = a.set_ingame;
        }
        if a.start_game.is_some() {
            merged.start_game = a.start_game;
        }
        apply_actions(&mut current_snapshot, &a);

        if current_snapshot.app != AppState::InGame {
            merged.start_game = None;
        }
    }

    merged.bump_generation = merged.start_game.is_some();
    merged
}

fn apply_actions(snapshot: &mut FlowSnapshot, actions: &FlowActions) {
    if let Some(app) = actions.set_app {
        snapshot.app = app;
        match app {
            AppState::BootLoading => {
                snapshot.frontend = None;
                snapshot.ingame = None;
            }
            AppState::FrontEnd => {
                snapshot.frontend = Some(FrontEndState::default());
                snapshot.ingame = None;
            }
            AppState::InGame => {
                snapshot.frontend = None;
                snapshot.ingame = Some(InGameState::default());
            }
        }
    }

    if let Some(frontend) = actions.set_frontend {
        snapshot.frontend = Some(frontend);
    }

    if let Some(ingame) = actions.set_ingame {
        snapshot.ingame = Some(ingame);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(
        app: AppState,
        fe: Option<FrontEndState>,
        ig: Option<InGameState>,
        gen: u64,
    ) -> FlowSnapshot {
        FlowSnapshot {
            app,
            frontend: fe,
            ingame: ig,
            generation: gen,
        }
    }

    #[test]
    fn main_menu_to_save_select() {
        let s = snap(AppState::FrontEnd, Some(FrontEndState::MainMenu), None, 0);
        let a = reduce(s, &FlowRequest::EnterSaveSelect);
        assert_eq!(a.set_frontend, Some(FrontEndState::SaveSelect));
    }

    #[test]
    fn start_load_bumps_generation_and_enters_ingame() {
        let s = snap(
            AppState::FrontEnd,
            Some(FrontEndState::SaveSelect),
            None,
            41,
        );
        let a = reduce(s, &FlowRequest::StartLoadSave("abc".into()));
        assert!(a.bump_generation);
        assert_eq!(a.set_app, Some(AppState::InGame));
        assert!(matches!(a.start_game, Some(GameStartKind::LoadSave(_))));
    }

    #[test]
    fn toggle_pause_only_in_ingame() {
        let s = snap(AppState::InGame, None, Some(InGameState::Playing), 0);
        let a = reduce(s, &FlowRequest::TogglePause);
        assert_eq!(a.set_ingame, Some(InGameState::Paused));
    }

    #[test]
    fn enter_frontend_then_enter_save_select_uses_updated_snapshot() {
        let s = snap(AppState::BootLoading, None, None, 0);
        let requests = [FlowRequest::EnterFrontEnd, FlowRequest::EnterSaveSelect];

        let a = resolve_requests(s, requests.iter());

        assert_eq!(a.set_app, Some(AppState::FrontEnd));
        assert_eq!(a.set_frontend, Some(FrontEndState::SaveSelect));
    }

    #[test]
    fn start_load_then_quit_to_main_menu_clears_pending_start() {
        let s = snap(
            AppState::FrontEnd,
            Some(FrontEndState::SaveSelect),
            None,
            7,
        );
        let requests = [
            FlowRequest::StartLoadSave("slot-1".into()),
            FlowRequest::QuitToMainMenu,
        ];

        let a = resolve_requests(s, requests.iter());

        assert_eq!(a.set_app, Some(AppState::FrontEnd));
        assert_eq!(a.set_frontend, Some(FrontEndState::MainMenu));
        assert!(a.start_game.is_none());
        assert!(!a.bump_generation);
    }

    #[test]
    fn toggle_pause_then_resume_results_in_playing() {
        let s = snap(AppState::InGame, None, Some(InGameState::Playing), 0);
        let requests = [FlowRequest::TogglePause, FlowRequest::Resume];

        let a = resolve_requests(s, requests.iter());

        assert_eq!(a.set_ingame, Some(InGameState::Playing));
    }
}
