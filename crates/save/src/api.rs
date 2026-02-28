use std::sync::Arc;

use bevy::prelude::*;
use bevy::tasks::{IoTaskPool, Task};
use bevy::tasks::{futures_lite::future, poll_once};

use cruft_game_flow::{AppState, BootReadiness, BootReady, FlowRequest, GameStartKind, InGameState, PendingGameStart};

use crate::io as save_io;
use crate::types::{LoadedSave, SaveId, SaveMeta, SaveRootDir};

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum SaveSet {
    Scan,
    Ops,
    Loading,
}

#[derive(Resource, Debug, Default, Clone)]
pub struct SaveIndex {
    pub items: Arc<[SaveMeta]>,
}

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

#[derive(Resource, Debug)]
struct ScanTask(Task<Result<Vec<SaveMeta>, String>>);

#[derive(Resource, Debug)]
struct OpTask(Task<Result<SaveOpResult, String>>);

#[derive(Resource, Debug)]
struct LoadTask(Task<SaveLoadResult>);

#[derive(Resource, Debug, Clone, Copy)]
struct ActiveLoadGeneration(u64);

#[derive(Resource, Debug, Default)]
pub struct CurrentSave(pub Option<LoadedSave>);

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
            .add_systems(OnEnter(AppState::BootLoading), start_scan_index)
            .add_systems(
                Update,
                (
                    poll_scan_index.in_set(SaveSet::Scan),
                    (start_save_ops, poll_save_ops).chain().in_set(SaveSet::Ops),
                    (
                        start_loading_from_pending
                            .run_if(in_state(AppState::InGame))
                            .run_if(in_state(InGameState::Loading)),
                        start_load_task
                            .run_if(in_state(AppState::InGame))
                            .run_if(in_state(InGameState::Loading)),
                        poll_load_task
                            .run_if(in_state(AppState::InGame))
                            .run_if(in_state(InGameState::Loading)),
                    )
                        .chain()
                        .in_set(SaveSet::Loading),
                ),
            )
            .configure_sets(Update, (SaveSet::Scan, SaveSet::Ops, SaveSet::Loading).chain());
    }
}

fn start_scan_index(mut commands: Commands, root: Res<SaveRootDir>) {
    let root = root.0.clone();
    let task = IoTaskPool::get().spawn(async move {
        save_io::scan_index(&root).map_err(|e| e.to_string())
    });
    commands.insert_resource(ScanTask(task));
}

fn poll_scan_index(
    mut commands: Commands,
    scan: Option<ResMut<ScanTask>>,
    mut index: ResMut<SaveIndex>,
    mut ready: ResMut<SaveIndexReady>,
    mut boot: ResMut<BootReadiness>,
) {
    let Some(mut scan) = scan else { return };
    if let Some(result) = future::block_on(poll_once(&mut scan.0)) {
        commands.remove_resource::<ScanTask>();
        match result {
            Ok(items) => {
                index.items = Arc::from(items);
                ready.0 = true;
                boot.0.insert(BootReady::SAVE_INDEX);
            }
            Err(err) => {
                log::warn!("scan saves failed: {err}");
                ready.0 = true;
                boot.0.insert(BootReady::SAVE_INDEX);
            }
        }
    }
}

fn start_save_ops(
    mut commands: Commands,
    mut requests: MessageReader<SaveOpRequest>,
    root: Res<SaveRootDir>,
    existing: Option<Res<OpTask>>,
) {
    if existing.is_some() {
        return;
    }

    let mut last = None;
    for req in requests.read() {
        last = Some(req.clone());
    }
    let Some(req) = last else {
        return;
    };

    let root = root.0.clone();
    let task = IoTaskPool::get().spawn(async move {
        let res = match req {
            SaveOpRequest::Rescan => save_io::scan_index(&root)
                .map(|items| SaveOpResult::IndexUpdated { items: Arc::from(items) })
                .map_err(|e| e.to_string()),
            SaveOpRequest::CreateNew { display_name } => save_io::create_new_save(&root, display_name)
                .map(|meta| SaveOpResult::Created { meta })
                .map_err(|e| e.to_string()),
            SaveOpRequest::Copy { id } => save_io::copy_save(&root, &id)
                .map(|meta| SaveOpResult::Copied { meta })
                .map_err(|e| e.to_string()),
            SaveOpRequest::Rename { id, new_name } => save_io::rename_save(&root, &id, new_name)
                .map(|meta| SaveOpResult::Renamed { meta })
                .map_err(|e| e.to_string()),
            SaveOpRequest::Delete { id } => save_io::soft_delete_save(&root, &id)
                .map(|_| SaveOpResult::Deleted { id })
                .map_err(|e| e.to_string()),
        };
        res
    });

    commands.insert_resource(OpTask(task));
}

fn poll_save_ops(
    mut commands: Commands,
    task: Option<ResMut<OpTask>>,
    mut results: MessageWriter<SaveOpResult>,
    mut index: ResMut<SaveIndex>,
) {
    let Some(mut task) = task else { return };
    if let Some(result) = future::block_on(poll_once(&mut task.0)) {
        commands.remove_resource::<OpTask>();
        match result {
            Ok(r) => {
                apply_op_to_index(&mut index, &r);
                let _ = results.write(r);
            }
            Err(err) => {
                let _ = results.write(SaveOpResult::Failed { message: err });
            }
        }
    }
}

fn apply_op_to_index(index: &mut SaveIndex, r: &SaveOpResult) {
    let mut v: Vec<SaveMeta> = index.items.iter().cloned().collect();
    match r {
        SaveOpResult::IndexUpdated { items } => {
            index.items = items.clone();
            return;
        }
        SaveOpResult::Created { meta } | SaveOpResult::Copied { meta } => {
            v.push(meta.clone());
        }
        SaveOpResult::Renamed { meta } => {
            for item in &mut v {
                if item.id == meta.id {
                    *item = meta.clone();
                }
            }
        }
        SaveOpResult::Deleted { id } => {
            v.retain(|m| m.id != id.0);
        }
        SaveOpResult::Failed { .. } => return,
    }
    v.sort_by(|a, b| b.last_played_at.cmp(&a.last_played_at));
    index.items = Arc::from(v);
}

fn start_loading_from_pending(
    mut commands: Commands,
    pending: Option<Res<PendingGameStart>>,
    root: Res<SaveRootDir>,
    mut load_requests: MessageWriter<SaveLoadRequest>,
    mut flow: MessageWriter<FlowRequest>,
    existing: Option<Res<LoadTask>>,
) {
    if existing.is_some() {
        return;
    }
    let Some(pending) = pending else {
        log::warn!("enter InGame::Loading without PendingGameStart");
        flow.write(FlowRequest::QuitToMainMenu);
        return;
    };

    match &pending.kind {
        GameStartKind::LoadSave(id) => {
            load_requests.write(SaveLoadRequest {
                id: SaveId(id.clone()),
                generation: pending.generation,
            });
        }
        GameStartKind::NewSave { display_name } => {
            // 直接在 Loading 阶段创建并加载，避免复杂跨状态链式操作。
            let root = root.0.clone();
            let name = display_name.clone();
            let generation = pending.generation;
            let task = IoTaskPool::get().spawn(async move {
                match save_io::create_new_save(&root, name) {
                    Ok(meta) => SaveLoadResult::Loaded {
                        save: LoadedSave { meta },
                        generation,
                    },
                    Err(err) => SaveLoadResult::Failed {
                        message: err.to_string(),
                        generation,
                    },
                }
            });
            commands.insert_resource(LoadTask(task));
            commands.insert_resource(ActiveLoadGeneration(generation));
        }
    }

    // Pending intent 只消费一次，避免重复触发。
    commands.remove_resource::<PendingGameStart>();
}

fn poll_load_task(
    mut commands: Commands,
    load_task: Option<ResMut<LoadTask>>,
    mut load_results: MessageWriter<SaveLoadResult>,
    mut current: ResMut<CurrentSave>,
    mut flow: MessageWriter<FlowRequest>,
    active: Option<Res<ActiveLoadGeneration>>,
    mut index: ResMut<SaveIndex>,
) {
    let Some(mut load_task) = load_task else {
        return;
    };
    if let Some(result) = future::block_on(poll_once(&mut load_task.0)) {
        commands.remove_resource::<LoadTask>();
        let expected = active.map(|a| a.0);
        commands.remove_resource::<ActiveLoadGeneration>();

        let accept = match (&result, expected) {
            (SaveLoadResult::Loaded { generation, .. }, Some(exp)) => *generation == exp,
            (SaveLoadResult::Failed { generation, .. }, Some(exp)) => *generation == exp,
            (_, None) => true,
        };
        if !accept {
            // 过期结果，直接丢弃。
            return;
        }

        match &result {
            SaveLoadResult::Loaded { save, .. } => {
                current.0 = Some(save.clone());
                ensure_index_contains(&mut index, &save.meta);
                let _ = flow.write(FlowRequest::FinishGameLoading);
            }
            SaveLoadResult::Failed { .. } => {}
        }
        let _ = load_results.write(result);
    }
}

fn start_load_task(
    mut commands: Commands,
    mut requests: MessageReader<SaveLoadRequest>,
    root: Res<SaveRootDir>,
    existing: Option<Res<LoadTask>>,
) {
    if existing.is_some() {
        return;
    }

    let mut last = None;
    for req in requests.read() {
        last = Some(req.clone());
    }
    let Some(req) = last else {
        return;
    };

    let root = root.0.clone();
    let task = IoTaskPool::get().spawn(async move {
        match save_io::load_save_minimal(&root, &req.id) {
            Ok(meta) => SaveLoadResult::Loaded {
                save: LoadedSave { meta },
                generation: req.generation,
            },
            Err(err) => SaveLoadResult::Failed {
                message: err.to_string(),
                generation: req.generation,
            },
        }
    });

    commands.insert_resource(LoadTask(task));
    commands.insert_resource(ActiveLoadGeneration(req.generation));
}

fn ensure_index_contains(index: &mut SaveIndex, meta: &SaveMeta) {
    if index.items.iter().any(|m| m.id == meta.id) {
        return;
    }
    let mut v: Vec<SaveMeta> = index.items.iter().cloned().collect();
    v.push(meta.clone());
    v.sort_by(|a, b| b.last_played_at.cmp(&a.last_played_at));
    index.items = Arc::from(v);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn scan_create_rename_copy_delete() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        let meta = save_io::create_new_save(root, "A".into()).unwrap();
        let items = save_io::scan_index(root).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, meta.id);

        let id = SaveId(meta.id.clone());
        let renamed = save_io::rename_save(root, &id, "B".into()).unwrap();
        assert_eq!(renamed.display_name, "B");

        let copied = save_io::copy_save(root, &id).unwrap();
        assert_ne!(copied.id, meta.id);

        save_io::soft_delete_save(root, &id).unwrap();
        let items2 = save_io::scan_index(root).unwrap();
        assert_eq!(items2.len(), 1);
        assert_eq!(items2[0].id, copied.id);
    }
}
