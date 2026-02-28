use std::sync::Arc;

use bevy::prelude::*;

use cruft_game_flow::{AppState, BootReadiness, BootReady};

use crate::types::{LoadedSave, SaveId, SaveMeta};

/// 存档索引（内存占位实现：默认为空）。
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

/// 当前会话“正在游戏内”的存档（内存占位实现：用于 UI/调试）。
#[derive(Resource, Debug, Default, Clone)]
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
            .add_systems(OnEnter(AppState::BootLoading), mark_index_ready)
            .add_systems(OnExit(AppState::InGame), clear_current_save)
            .add_systems(
                Update,
                (
                    drain_ops.run_if(in_state(AppState::FrontEnd)),
                    drain_loads.run_if(in_state(AppState::InGame)),
                ),
            );
    }
}

fn mark_index_ready(
    mut index: ResMut<SaveIndex>,
    mut ready: ResMut<SaveIndexReady>,
    mut boot: ResMut<BootReadiness>,
) {
    // 内存占位实现：索引永远可用且为空。
    index.items = Arc::from([]);
    ready.0 = true;
    boot.0.insert(BootReady::SAVE_INDEX);
}

fn drain_ops(mut requests: MessageReader<SaveOpRequest>, mut results: MessageWriter<SaveOpResult>) {
    for _ in requests.read() {
        // TODO(voxel): 内存存档阶段暂不实现 Copy/Rename/Delete/Rescan。
        let _ = results.write(SaveOpResult::Failed {
            message: "Save ops are not implemented (in-memory placeholder)".to_string(),
        });
    }
}

fn drain_loads(
    mut requests: MessageReader<SaveLoadRequest>,
    mut results: MessageWriter<SaveLoadResult>,
) {
    for req in requests.read() {
        // TODO(voxel): 内存存档阶段暂不实现 Load。
        let _ = results.write(SaveLoadResult::Failed {
            message: "Save load is not implemented (in-memory placeholder)".to_string(),
            generation: req.generation,
        });
    }
}

fn clear_current_save(mut current: ResMut<CurrentSave>) {
    // “退出存档后清空”：任何从 InGame 离开都清空会话存档。
    current.0 = None;
}
