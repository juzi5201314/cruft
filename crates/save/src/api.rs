use std::cmp::Reverse;
use std::path::PathBuf;
use std::sync::Arc;

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_state::prelude::*;
use bevy_tasks::{futures_lite::future, poll_once, IoTaskPool, Task};

use cruft_game_flow::{
    AppState, BootReadiness, BootReady, FlowRequest, GameStartGeneration, GameStartKind,
    InGameState, PendingGameStart,
};
use cruft_worldgen_spec::WorldGenPreset;

use crate::io;
use crate::types::{LoadedSave, SaveId, SaveMeta, SaveRootDir};

/// 存档索引。
#[derive(Resource, Debug, Default, Clone)]
pub struct SaveIndex {
    pub items: Arc<[SaveMeta]>,
}

/// 存档索引是否已准备完成（BootLoading 聚合用）。
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct SaveIndexReady(pub bool);

#[derive(Message, Debug, Clone)]
pub enum SaveOpRequest {
    CreateNew { display_name: String },
    Copy { id: SaveId },
    Rename { id: SaveId, new_name: String },
    Delete { id: SaveId },
    Rescan,
}

#[derive(Message, Debug, Clone)]
pub enum SaveOpResult {
    IndexUpdated { items: Arc<[SaveMeta]> },
    Created { meta: SaveMeta },
    Copied { meta: SaveMeta },
    Renamed { meta: SaveMeta },
    Deleted { id: SaveId },
    Failed { message: String },
}

#[derive(Message, Debug, Clone)]
pub struct SaveLoadRequest {
    pub id: SaveId,
    pub generation: u64,
}

#[derive(Message, Debug, Clone)]
pub enum SaveLoadResult {
    Loaded { save: LoadedSave, generation: u64 },
    Failed { message: String, generation: u64 },
}

/// 当前会话“正在游戏内”的存档。
#[derive(Resource, Debug, Default, Clone)]
pub struct CurrentSave(pub Option<LoadedSave>);

#[derive(Resource, Default)]
struct SaveScanTask(Option<Task<Result<Arc<[SaveMeta]>, String>>>);

#[derive(Resource, Default)]
struct SaveOpTask(Option<Task<SaveOpResult>>);

#[derive(Debug)]
struct LoadOutcome {
    generation: u64,
    result: Result<LoadedSave, String>,
}

#[derive(Resource, Default)]
struct SaveLoadTask(Option<Task<LoadOutcome>>);

pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SaveOpRequest>()
            .add_message::<SaveOpResult>()
            .add_message::<SaveLoadRequest>()
            .add_message::<SaveLoadResult>()
            .init_resource::<SaveIndex>()
            .init_resource::<SaveIndexReady>()
            .init_resource::<CurrentSave>()
            .init_resource::<SaveScanTask>()
            .init_resource::<SaveOpTask>()
            .init_resource::<SaveLoadTask>()
            .add_systems(OnEnter(AppState::BootLoading), start_scan_index)
            .add_systems(OnEnter(InGameState::Loading), start_ingame_load)
            .add_systems(OnExit(AppState::InGame), clear_current_save)
            .add_systems(
                Update,
                (
                    poll_scan_index.run_if(in_state(AppState::BootLoading)),
                    (start_save_ops, poll_save_ops)
                        .chain()
                        .run_if(in_state(AppState::FrontEnd)),
                    poll_ingame_load.run_if(in_state(AppState::InGame)),
                ),
            );
    }
}

fn start_scan_index(
    root: Res<SaveRootDir>,
    mut ready: ResMut<SaveIndexReady>,
    mut scan_task: ResMut<SaveScanTask>,
) {
    ready.0 = false;

    let root = root.0.clone();
    let pool = IoTaskPool::get();
    scan_task.0 = Some(pool.spawn(async move { scan_index_task(root) }));
}

fn poll_scan_index(
    mut index: ResMut<SaveIndex>,
    mut ready: ResMut<SaveIndexReady>,
    mut boot: ResMut<BootReadiness>,
    mut scan_task: ResMut<SaveScanTask>,
) {
    let Some(mut task) = scan_task.0.take() else {
        return;
    };

    if let Some(result) = future::block_on(poll_once(&mut task)) {
        match result {
            Ok(items) => index.items = items,
            Err(err) => {
                log::warn!("save index scan failed: {err}");
                index.items = Arc::from([]);
            }
        }
        ready.0 = true;
        boot.0.insert(BootReady::SAVE_INDEX);
    } else {
        scan_task.0 = Some(task);
    }
}

fn start_save_ops(
    root: Res<SaveRootDir>,
    mut requests: MessageReader<SaveOpRequest>,
    mut op_task: ResMut<SaveOpTask>,
) {
    if op_task.0.is_some() {
        return;
    }

    let mut last: Option<SaveOpRequest> = None;
    for req in requests.read() {
        last = Some(req.clone());
    }

    let Some(request) = last else {
        return;
    };

    let root = root.0.clone();
    let pool = IoTaskPool::get();
    op_task.0 = Some(pool.spawn(async move { run_op(root, request) }));
}

fn poll_save_ops(
    mut op_task: ResMut<SaveOpTask>,
    mut results: MessageWriter<SaveOpResult>,
    mut index: ResMut<SaveIndex>,
) {
    let Some(mut task) = op_task.0.take() else {
        return;
    };

    if let Some(result) = future::block_on(poll_once(&mut task)) {
        apply_op_to_index(&mut index, &result);
        results.write(result);
    } else {
        op_task.0 = Some(task);
    }
}

fn start_ingame_load(
    root: Res<SaveRootDir>,
    pending: Option<Res<PendingGameStart>>,
    mut load_task: ResMut<SaveLoadTask>,
    mut current: ResMut<CurrentSave>,
    mut commands: Commands,
    mut results: MessageWriter<SaveLoadResult>,
    mut flow: MessageWriter<FlowRequest>,
) {
    current.0 = None;

    if load_task.0.is_some() {
        return;
    }

    let Some(pending) = pending else {
        results.write(SaveLoadResult::Failed {
            message: "missing PendingGameStart".to_string(),
            generation: 0,
        });
        flow.write(FlowRequest::QuitToMainMenu);
        return;
    };

    let generation = pending.generation;
    let kind = pending.kind.clone();
    let root = root.0.clone();

    let pool = IoTaskPool::get();
    load_task.0 = Some(pool.spawn(async move {
        let result = match kind {
            GameStartKind::LoadSave(id) => io::load_save(&root, &SaveId(id)),
            GameStartKind::NewSave {
                display_name,
                generator_preset,
            } => io::create_new_save(&root, display_name, generation, generator_preset),
        }
        .map_err(|err| err.to_string());

        LoadOutcome { generation, result }
    }));

    commands.remove_resource::<PendingGameStart>();
}
fn poll_ingame_load(
    mut load_task: ResMut<SaveLoadTask>,
    mut current: ResMut<CurrentSave>,
    generation: Res<GameStartGeneration>,
    mut results: MessageWriter<SaveLoadResult>,
    mut flow: MessageWriter<FlowRequest>,
) {
    let Some(mut task) = load_task.0.take() else {
        return;
    };

    if let Some(outcome) = future::block_on(poll_once(&mut task)) {
        // 丢弃旧 generation 的结果：如果用户快速"进入 -> 退出 -> 再进入"，
        // 老会话的异步结果不能污染当前会话。
        if outcome.generation != generation.0 {
            return;
        }

        match outcome.result {
            Ok(save) => {
                current.0 = Some(save.clone());
                results.write(SaveLoadResult::Loaded {
                    save,
                    generation: outcome.generation,
                });
            }
            Err(message) => {
                current.0 = None;
                results.write(SaveLoadResult::Failed {
                    message,
                    generation: outcome.generation,
                });
                flow.write(FlowRequest::QuitToMainMenu);
            }
        }
    } else {
        load_task.0 = Some(task);
    }
}

fn clear_current_save(
    mut current: ResMut<CurrentSave>,
    mut load_task: ResMut<SaveLoadTask>,
    mut op_task: ResMut<SaveOpTask>,
) {
    // “退出存档后清空”：任何从 InGame 离开都清空会话存档。
    current.0 = None;
    load_task.0 = None;
    op_task.0 = None;
}

fn scan_index_task(root: PathBuf) -> Result<Arc<[SaveMeta]>, String> {
    let items = io::scan_index(&root).map_err(|err| err.to_string())?;
    Ok(Arc::from(items.into_boxed_slice()))
}

fn run_op(root: PathBuf, request: SaveOpRequest) -> SaveOpResult {
    match request {
        SaveOpRequest::CreateNew { display_name } => {
            match io::create_new_save(&root, display_name, 0, WorldGenPreset::ModernSurface)
                .map(|save| save.meta)
            {
                Ok(meta) => SaveOpResult::Created { meta },
                Err(err) => SaveOpResult::Failed {
                    message: err.to_string(),
                },
            }
        }
        SaveOpRequest::Copy { id } => match io::copy_save(&root, &id) {
            Ok(meta) => SaveOpResult::Copied { meta },
            Err(err) => SaveOpResult::Failed {
                message: err.to_string(),
            },
        },
        SaveOpRequest::Rename { id, new_name } => match io::rename_save(&root, &id, new_name) {
            Ok(meta) => SaveOpResult::Renamed { meta },
            Err(err) => SaveOpResult::Failed {
                message: err.to_string(),
            },
        },
        SaveOpRequest::Delete { id } => match io::soft_delete_save(&root, &id) {
            Ok(()) => SaveOpResult::Deleted { id },
            Err(err) => SaveOpResult::Failed {
                message: err.to_string(),
            },
        },
        SaveOpRequest::Rescan => match io::scan_index(&root) {
            Ok(items) => SaveOpResult::IndexUpdated {
                items: Arc::from(items.into_boxed_slice()),
            },
            Err(err) => SaveOpResult::Failed {
                message: err.to_string(),
            },
        },
    }
}

fn apply_op_to_index(index: &mut SaveIndex, result: &SaveOpResult) {
    match result {
        SaveOpResult::IndexUpdated { items } => {
            index.items = items.clone();
        }
        SaveOpResult::Created { meta }
        | SaveOpResult::Copied { meta }
        | SaveOpResult::Renamed { meta } => {
            let mut items = index.items.to_vec();
            if let Some(i) = items.iter().position(|m| m.id == meta.id) {
                items[i] = meta.clone();
            } else {
                items.push(meta.clone());
            }
            items.sort_by_key(|m| Reverse(m.last_played_at));
            index.items = Arc::from(items.into_boxed_slice());
        }
        SaveOpResult::Deleted { id } => {
            let mut items = index.items.to_vec();
            items.retain(|m| m.id != id.0);
            index.items = Arc::from(items.into_boxed_slice());
        }
        SaveOpResult::Failed { .. } => {}
    }
}
