use std::io;
use std::sync::Arc;

use bevy::prelude::*;
use bevy::tasks::{futures_lite::future, poll_once, IoTaskPool, Task};

use cruft_game_flow::{
    AppState, BootReadiness, BootReady, FlowRequest, GameStartKind, InGameState,
    PendingGameStart,
};

use crate::io::{
    create_new_save, copy_save, load_save_minimal, read_save_world_info, rename_save, scan_index,
    soft_delete_save, touch_last_played,
};
use crate::types::{LoadedSave, SaveId, SaveMeta, SaveRootDir, SaveWorldInfo};

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
pub struct SaveInfoRequest {
    pub id: SaveId,
}

#[derive(Message, Debug, Clone)]
pub enum SaveInfoResult {
    Loaded { info: SaveWorldInfo },
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

/// 当前会话“正在游戏内”的存档（用于 UI/调试）。
#[derive(Resource, Debug, Default, Clone)]
pub struct CurrentSave(pub Option<LoadedSave>);

#[derive(Resource, Debug, Clone, Copy)]
struct ActiveLoadGeneration(pub u64);

#[derive(Resource)]
struct ScanTask(Task<io::Result<Vec<SaveMeta>>>);

#[derive(Resource)]
struct OpTask(Task<io::Result<SaveOpTaskResult>>);

#[derive(Resource)]
struct LoadTask(Task<LoadTaskResult>);

#[derive(Resource)]
struct InfoTask(Task<io::Result<SaveWorldInfo>>);

type LoadTaskResult = Result<(SaveMeta, u64), (String, u64)>;

#[derive(Debug)]
enum SaveOpTaskResult {
    Created(SaveMeta),
    Copied(SaveMeta),
    Renamed(SaveMeta),
    Deleted(SaveId),
    Rescanned,
}

pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SaveOpRequest>()
            .add_message::<SaveOpResult>()
            .add_message::<SaveLoadRequest>()
            .add_message::<SaveLoadResult>()
            .add_message::<SaveInfoRequest>()
            .add_message::<SaveInfoResult>()
            .init_resource::<SaveRootDir>()
            .init_resource::<SaveIndex>()
            .init_resource::<SaveIndexReady>()
            .init_resource::<CurrentSave>()
            .add_systems(OnEnter(AppState::BootLoading), start_scan_index)
            .add_systems(OnEnter(InGameState::Loading), start_in_game_loading)
            .add_systems(OnExit(AppState::InGame), clear_current_save)
            .add_systems(
                Update,
                (
                    poll_scan_index,
                    start_save_ops.run_if(in_state(AppState::FrontEnd)),
                    poll_save_ops.run_if(in_state(AppState::FrontEnd)),
                    start_save_info.run_if(in_state(AppState::FrontEnd)),
                    poll_save_info.run_if(in_state(AppState::FrontEnd)),
                    start_save_loads.run_if(in_state(AppState::InGame)),
                    poll_save_loads.run_if(in_state(AppState::InGame)),
                    handle_in_game_load_results.run_if(in_state(InGameState::Loading)),
                ),
            );
    }
}

fn start_scan_index(
    mut commands: Commands,
    root: Res<SaveRootDir>,
    existing_task: Option<Res<ScanTask>>,
) {
    if existing_task.is_some() {
        return;
    }

    let root_dir = root.0.clone();
    let task = IoTaskPool::get().spawn(async move { scan_index(&root_dir) });
    commands.insert_resource(ScanTask(task));
}

fn poll_scan_index(
    mut commands: Commands,
    mut index: ResMut<SaveIndex>,
    mut ready: ResMut<SaveIndexReady>,
    mut boot: ResMut<BootReadiness>,
    mut results: MessageWriter<SaveOpResult>,
    mut task: Option<ResMut<ScanTask>>,
) {
    let Some(task) = task.as_mut() else {
        return;
    };

    if let Some(result) = future::block_on(poll_once(&mut task.0)) {
        commands.remove_resource::<ScanTask>();
        match result {
            Ok(items) => {
                let items: Arc<[SaveMeta]> = Arc::from(items);
                index.items = items.clone();
                ready.0 = true;
                boot.0.insert(BootReady::SAVE_INDEX);
                results.write(SaveOpResult::IndexUpdated { items });
            }
            Err(err) => {
                ready.0 = true;
                boot.0.insert(BootReady::SAVE_INDEX);
                results.write(SaveOpResult::Failed {
                    message: format!("scan save index failed: {err}"),
                });
            }
        }
    }
}

fn start_save_ops(
    mut commands: Commands,
    root: Res<SaveRootDir>,
    mut requests: MessageReader<SaveOpRequest>,
    existing_task: Option<Res<OpTask>>,
) {
    let Some(request) = requests.read().last().cloned() else {
        return;
    };
    if existing_task.is_some() {
        return;
    }

    let root_dir = root.0.clone();
    let task = IoTaskPool::get().spawn(async move {
        let result = match request {
            SaveOpRequest::CreateNew { display_name } => {
                SaveOpTaskResult::Created(create_new_save(&root_dir, display_name)?)
            }
            SaveOpRequest::Copy { id } => SaveOpTaskResult::Copied(copy_save(&root_dir, &id)?),
            SaveOpRequest::Rename { id, new_name } => {
                SaveOpTaskResult::Renamed(rename_save(&root_dir, &id, new_name)?)
            }
            SaveOpRequest::Delete { id } => {
                soft_delete_save(&root_dir, &id)?;
                SaveOpTaskResult::Deleted(id)
            }
            SaveOpRequest::Rescan => SaveOpTaskResult::Rescanned,
        };
        Ok(result)
    });
    commands.insert_resource(OpTask(task));
}

fn poll_save_ops(
    mut commands: Commands,
    root: Res<SaveRootDir>,
    mut index: ResMut<SaveIndex>,
    mut results: MessageWriter<SaveOpResult>,
    mut task: Option<ResMut<OpTask>>,
) {
    let Some(task) = task.as_mut() else {
        return;
    };

    if let Some(result) = future::block_on(poll_once(&mut task.0)) {
        commands.remove_resource::<OpTask>();
        match result {
            Ok(op_result) => {
                match op_result {
                    SaveOpTaskResult::Created(meta) => {
                        results.write(SaveOpResult::Created { meta });
                    }
                    SaveOpTaskResult::Copied(meta) => {
                        results.write(SaveOpResult::Copied { meta });
                    }
                    SaveOpTaskResult::Renamed(meta) => {
                        results.write(SaveOpResult::Renamed { meta });
                    }
                    SaveOpTaskResult::Deleted(id) => {
                        results.write(SaveOpResult::Deleted { id });
                    }
                    SaveOpTaskResult::Rescanned => {}
                }

                match scan_index(&root.0) {
                    Ok(items) => {
                        let items: Arc<[SaveMeta]> = Arc::from(items);
                        index.items = items.clone();
                        results.write(SaveOpResult::IndexUpdated { items });
                    }
                    Err(err) => {
                        results.write(SaveOpResult::Failed {
                            message: format!("rescan save index failed: {err}"),
                        });
                    }
                }
            }
            Err(err) => {
                results.write(SaveOpResult::Failed {
                    message: format!("save op failed: {err}"),
                });
            }
        }
    }
}


fn start_save_info(
    mut commands: Commands,
    root: Res<SaveRootDir>,
    mut requests: MessageReader<SaveInfoRequest>,
    existing_task: Option<Res<InfoTask>>,
) {
    let Some(request) = requests.read().last().cloned() else {
        return;
    };
    if existing_task.is_some() {
        return;
    }

    let root_dir = root.0.clone();
    let task = IoTaskPool::get().spawn(async move { read_save_world_info(&root_dir, &request.id) });
    commands.insert_resource(InfoTask(task));
}

fn poll_save_info(
    mut commands: Commands,
    mut results: MessageWriter<SaveInfoResult>,
    mut task: Option<ResMut<InfoTask>>,
) {
    let Some(task) = task.as_mut() else {
        return;
    };

    if let Some(result) = future::block_on(poll_once(&mut task.0)) {
        commands.remove_resource::<InfoTask>();
        match result {
            Ok(info) => {
                results.write(SaveInfoResult::Loaded { info });
            }
            Err(err) => {
                results.write(SaveInfoResult::Failed {
                    message: format!("load save info failed: {err}"),
                });
            }
        }
    }
}

fn start_in_game_loading(
    mut commands: Commands,
    root: Res<SaveRootDir>,
    pending: Option<Res<PendingGameStart>>,
    mut load_requests: MessageWriter<SaveLoadRequest>,
) {
    let Some(pending) = pending else {
        return;
    };

    let generation = pending.generation;
    match &pending.kind {
        GameStartKind::LoadSave(id) => {
            load_requests.write(SaveLoadRequest {
                id: SaveId(id.clone()),
                generation,
            });
        }
        GameStartKind::NewSave { display_name } => {
            let root_dir = root.0.clone();
            let display_name = display_name.clone();
            let task = IoTaskPool::get().spawn(async move {
                match create_new_save(&root_dir, display_name) {
                    Ok(meta) => Ok((meta, generation)),
                    Err(err) => Err((format!("create new save failed: {err}"), generation)),
                }
            });
            commands.insert_resource(LoadTask(task));
        }
    }

    commands.insert_resource(ActiveLoadGeneration(generation));
    commands.remove_resource::<PendingGameStart>();
}

fn start_save_loads(
    mut commands: Commands,
    root: Res<SaveRootDir>,
    mut requests: MessageReader<SaveLoadRequest>,
    existing_task: Option<Res<LoadTask>>,
) {
    let Some(request) = requests.read().last().cloned() else {
        return;
    };
    if existing_task.is_some() {
        return;
    }

    let root_dir = root.0.clone();
    let task = IoTaskPool::get().spawn(async move {
        match load_save_minimal(&root_dir, &request.id)
            .and_then(|meta| touch_last_played(&root_dir, &SaveId(meta.id.clone())))
        {
            Ok(meta) => Ok((meta, request.generation)),
            Err(err) => Err((format!("load save failed: {err}"), request.generation)),
        }
    });
    commands.insert_resource(LoadTask(task));
}

fn poll_save_loads(
    mut commands: Commands,
    mut current_save: ResMut<CurrentSave>,
    mut results: MessageWriter<SaveLoadResult>,
    mut task: Option<ResMut<LoadTask>>,
) {
    let Some(task) = task.as_mut() else {
        return;
    };

    if let Some(result) = future::block_on(poll_once(&mut task.0)) {
        commands.remove_resource::<LoadTask>();
        match result {
            Ok((meta, generation)) => {
                let loaded = LoadedSave { meta };
                current_save.0 = Some(loaded.clone());
                results.write(SaveLoadResult::Loaded {
                    save: loaded,
                    generation,
                });
            }
            Err((message, generation)) => {
                results.write(SaveLoadResult::Failed {
                    message,
                    generation,
                });
            }
        }
    }
}

fn handle_in_game_load_results(
    mut commands: Commands,
    active_generation: Option<Res<ActiveLoadGeneration>>,
    mut current_save: ResMut<CurrentSave>,
    mut results: MessageReader<SaveLoadResult>,
    mut flow: MessageWriter<FlowRequest>,
) {
    let Some(active_generation) = active_generation else {
        return;
    };
    let expected_generation = active_generation.0;

    for msg in results.read() {
        match msg {
            SaveLoadResult::Loaded { save, generation } if *generation == expected_generation => {
                current_save.0 = Some(save.clone());
                flow.write(FlowRequest::FinishGameLoading);
                commands.remove_resource::<ActiveLoadGeneration>();
            }
            SaveLoadResult::Failed {
                generation,
                message: _,
            } if *generation == expected_generation => {
                flow.write(FlowRequest::QuitToMainMenu);
                commands.remove_resource::<ActiveLoadGeneration>();
            }
            _ => {}
        }
    }
}

fn clear_current_save(mut commands: Commands, mut current: ResMut<CurrentSave>) {
    current.0 = None;
    commands.remove_resource::<ActiveLoadGeneration>();
    commands.remove_resource::<LoadTask>();
}
