use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use bevy::prelude::*;
use bevy::render::extract_component::ExtractComponent;
use bevy::render::extract_resource::ExtractResource;
use bevy::tasks::{futures_lite::future, poll_once, AsyncComputeTaskPool, Task};

use cruft_game_flow::{AppState, FlowRequest, InGameState};
use cruft_proc_textures::TextureRegistry;
use cruft_save::CurrentSave;
use cruft_worldgen_spec::WorldGenConfig;

use crate::blocks::BlockDefs;
use crate::coords::{chunk_index, ChunkKey};
use crate::meshing::{mesh, MeshingInput, MeshingOutput};
use crate::storage::Storage;
use crate::worldgen::{build_generator, GeneratedChunk, WorldGenerator};
use crate::CHUNK_SIZE;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource, Default)]
pub enum VoxelPhase {
    #[default]
    Loading,
    Playing,
}

#[derive(Debug, Clone, Copy, Resource)]
pub struct VoxelConfig {
    pub loading_ready_radius: i32,
    pub playing_stream_radius: i32,
    pub max_inflight_meshing: usize,
}

impl Default for VoxelConfig {
    fn default() -> Self {
        Self {
            loading_ready_radius: 2,
            playing_stream_radius: 8,
            max_inflight_meshing: 64,
        }
    }
}

/// 体素世界流式的“中心点”（世界 voxel 坐标）。由 gameplay 每帧更新。
#[derive(Debug, Clone, Copy, Resource)]
pub struct VoxelCenter(pub IVec3);

impl Default for VoxelCenter {
    fn default() -> Self {
        Self(IVec3::ZERO)
    }
}

#[derive(Debug, Clone)]
pub struct VoxelQuadUploadOp {
    pub offset: u32,
    pub data: Arc<[u64]>,
}

/// 主世界到 RenderApp 的增量上传契约。
///
/// - `full`：当 buffer 扩容或世界重置时发送全量快照
/// - `updates`：常规情况下只发送局部更新片段
#[derive(Resource, Debug, Default, Clone, ExtractResource)]
pub struct VoxelQuadUploadQueue {
    pub epoch: u64,
    pub quad_capacity: u32,
    pub full: Option<Arc<[u64]>>,
    pub updates: Arc<[VoxelQuadUploadOp]>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct VoxelMaterialFaceLayers {
    pub albedo: u16,
    pub normal: u16,
    pub orm: u16,
    pub emissive: u16,
    pub height: u16,
}

#[derive(Debug, Clone, Default, ExtractResource, Resource)]
pub struct VoxelMaterialTable {
    pub entries: Vec<VoxelMaterialEntry>,
}

#[derive(Debug, Clone, Default)]
pub struct VoxelMaterialEntry {
    pub alpha_mode: u32,
    pub cutout_threshold: f32,
    pub top: VoxelMaterialFaceLayers,
    pub bottom: VoxelMaterialFaceLayers,
    pub north: VoxelMaterialFaceLayers,
    pub south: VoxelMaterialFaceLayers,
    pub east: VoxelMaterialFaceLayers,
    pub west: VoxelMaterialFaceLayers,
}

#[derive(Resource)]
pub struct VoxelWorld {
    pub defs: BlockDefs,
    pub storage: Storage,
    pub worldgen_config: WorldGenConfig,
    pub generator: Box<dyn WorldGenerator>,
}

impl VoxelWorld {
    pub fn set_worldgen_config(&mut self, config: WorldGenConfig) {
        self.worldgen_config = config.clone();
        self.generator = build_generator(&config);
    }

    pub fn sample_surface_height(&self, wx: i32, wz: i32) -> i32 {
        self.generator.sample_surface_height(wx, wz)
    }
}

impl Default for VoxelWorld {
    fn default() -> Self {
        let worldgen_config = WorldGenConfig::default();
        Self {
            defs: BlockDefs::default(),
            storage: Storage::default(),
            generator: build_generator(&worldgen_config),
            worldgen_config,
        }
    }
}

#[derive(Debug, Clone, Copy, Component, PartialEq, Eq)]
pub struct ChunkGeneration(pub u32);

#[derive(Debug, Clone, Copy, Component, PartialEq, Eq, ExtractComponent)]
pub struct ChunkBounds {
    pub min: IVec3,
    pub max: IVec3,
}

#[derive(Debug, Clone, Copy, Component, PartialEq, Eq, Default, ExtractComponent)]
pub struct ChunkDrawRange {
    pub opaque_offset: u32,
    pub opaque_len: u32,
    pub cutout_offset: u32,
    pub cutout_len: u32,
    pub transparent_offset: u32,
    pub transparent_len: u32,
}

#[derive(Resource, Default)]
struct VoxelLoadingTracker {
    required: HashSet<ChunkKey>,
    completed: HashSet<ChunkKey>,
    finished: bool,
}

#[derive(Resource, Default)]
struct VoxelMeshingTasks {
    tasks: HashMap<ChunkKey, Task<MeshingOutput>>,
    epoch: u64,
}

#[derive(Resource, Default)]
struct VoxelLoadGate {
    worldgen_ready: bool,
    applied_world_id: Option<String>,
}

#[derive(Resource, Default)]
struct VoxelQuadArena {
    data: Vec<u64>,
    free: Vec<FreeRange>,
    dirty: bool,
    full_sync_required: bool,
    pending_updates: Vec<VoxelQuadUploadOp>,
}

#[derive(Debug, Clone, Copy)]
struct FreeRange {
    offset: u32,
    len: u32,
}

impl VoxelQuadArena {
    fn alloc(&mut self, len: u32) -> u32 {
        if len == 0 {
            return 0;
        }

        if let Some((idx, range)) = self
            .free
            .iter()
            .enumerate()
            .find(|(_, r)| r.len >= len)
            .map(|(i, r)| (i, *r))
        {
            self.free.swap_remove(idx);
            // 简单 first-fit：若剩余空间足够大则切分回 free list。
            if range.len > len {
                self.free.push(FreeRange {
                    offset: range.offset + len,
                    len: range.len - len,
                });
            }
            return range.offset;
        }

        let offset = self.data.len() as u32;
        self.data.resize(self.data.len() + (len as usize), 0);
        // 扩容会导致 GPU buffer 重建，下一帧需要全量同步。
        self.full_sync_required = true;
        self.dirty = true;
        offset
    }

    fn free(&mut self, offset: u32, len: u32) {
        if len == 0 {
            return;
        }
        self.free.push(FreeRange { offset, len });
    }

    fn write(&mut self, offset: u32, quads: &[u64]) {
        if quads.is_empty() {
            return;
        }
        let start = offset as usize;
        let end = start + quads.len();
        self.data[start..end].copy_from_slice(quads);
        self.dirty = true;

        if !self.full_sync_required {
            self.pending_updates.push(VoxelQuadUploadOp {
                offset,
                data: Arc::from(quads.to_vec().into_boxed_slice()),
            });
        }
    }
}

pub struct VoxelPlugin;

impl Plugin for VoxelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VoxelConfig>()
            .init_resource::<VoxelCenter>()
            .init_resource::<VoxelPhase>()
            .init_resource::<VoxelWorld>()
            .init_resource::<VoxelMaterialTable>()
            .init_resource::<VoxelQuadUploadQueue>()
            .init_resource::<VoxelLoadingTracker>()
            .init_resource::<VoxelMeshingTasks>()
            .init_resource::<VoxelLoadGate>()
            .init_resource::<VoxelQuadArena>()
            .add_plugins(crate::render::VoxelRenderPlugin)
            .add_systems(OnEnter(InGameState::Loading), start_voxel_loading)
            .add_systems(OnEnter(InGameState::Playing), enter_voxel_playing)
            .add_systems(OnExit(AppState::InGame), cleanup_voxel_world)
            .add_systems(
                Update,
                (
                    configure_worldgen_from_save_system,
                    sync_block_materials_from_registry_system,
                    update_loading_tracker_system,
                    stream_chunks_system,
                    spawn_meshing_tasks_system,
                    poll_meshing_tasks_system,
                    sync_upload_queue_system,
                )
                    .chain()
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

fn sync_block_materials_from_registry_system(
    registry: Option<Res<TextureRegistry>>,
    mut world: ResMut<VoxelWorld>,
    mut table: ResMut<VoxelMaterialTable>,
) {
    let Some(registry) = registry else {
        return;
    };
    if !world.defs.is_resolved() {
        if let Err(error) = world.defs.resolve_from_registry(&registry) {
            log::error!("Failed to resolve voxel block materials: {error}");
            return;
        }
    }
    if !table.entries.is_empty() {
        return;
    }

    table.entries = world
        .defs
        .iter()
        .map(|(_index, def)| {
            let Some(texture) = registry.get(def.texture_name) else {
                return VoxelMaterialEntry::default();
            };
            let convert = |face: &cruft_proc_textures::ResolvedFace| VoxelMaterialFaceLayers {
                albedo: face.albedo.layer_index,
                normal: face.normal.layer_index,
                orm: face.orm.layer_index,
                emissive: face.emissive.layer_index,
                height: face.height.layer_index,
            };
            VoxelMaterialEntry {
                alpha_mode: texture.alpha_mode as u32,
                cutout_threshold: texture.cutout_threshold,
                top: convert(&texture.top),
                bottom: convert(&texture.bottom),
                north: convert(&texture.north),
                south: convert(&texture.south),
                east: convert(&texture.east),
                west: convert(&texture.west),
            }
        })
        .collect();
}

#[expect(
    clippy::too_many_arguments,
    reason = "启动清理需要同时重置多个子系统资源"
)]
fn start_voxel_loading(
    mut phase: ResMut<VoxelPhase>,
    config: Res<VoxelConfig>,
    mut tracker: ResMut<VoxelLoadingTracker>,
    mut tasks: ResMut<VoxelMeshingTasks>,
    world: ResMut<VoxelWorld>,
    mut load_gate: ResMut<VoxelLoadGate>,
    mut arena: ResMut<VoxelQuadArena>,
    mut uploads: ResMut<VoxelQuadUploadQueue>,
) {
    *phase = VoxelPhase::Loading;

    // “退出存档后清空”：每次进入 Loading 都重置世界与渲染数据。
    world.storage.clear();
    arena.data.clear();
    arena.free.clear();
    arena.pending_updates.clear();
    arena.full_sync_required = true;
    arena.dirty = true;
    uploads.epoch = uploads.epoch.wrapping_add(1);
    uploads.quad_capacity = 0;
    uploads.full = Some(Arc::from([]));
    uploads.updates = Arc::from([]);
    tracker.required.clear();
    tracker.completed.clear();
    tracker.finished = false;
    load_gate.worldgen_ready = false;
    load_gate.applied_world_id = None;
    tasks.tasks.clear();
    tasks.epoch = tasks.epoch.wrapping_add(1);
    let _ = &config;
    tracker.required.clear();
}

fn configure_worldgen_from_save_system(
    phase: Res<VoxelPhase>,
    current_save: Option<Res<CurrentSave>>,
    mut load_gate: ResMut<VoxelLoadGate>,
    mut world: ResMut<VoxelWorld>,
) {
    if !matches!(*phase, VoxelPhase::Loading) {
        load_gate.worldgen_ready = true;
        return;
    }

    let Some(current_save) = current_save else {
        load_gate.worldgen_ready = false;
        return;
    };
    let Some(loaded) = current_save.0.as_ref() else {
        load_gate.worldgen_ready = false;
        return;
    };

    if load_gate.applied_world_id.as_deref() != Some(loaded.world_header.world_uuid.as_str()) {
        world.set_worldgen_config(loaded.world_header.generator.clone());
        load_gate.applied_world_id = Some(loaded.world_header.world_uuid.clone());
    }

    load_gate.worldgen_ready = true;
}

fn enter_voxel_playing(mut phase: ResMut<VoxelPhase>, mut load_gate: ResMut<VoxelLoadGate>) {
    *phase = VoxelPhase::Playing;
    load_gate.worldgen_ready = true;
}

fn cleanup_voxel_world(
    mut phase: ResMut<VoxelPhase>,
    mut tracker: ResMut<VoxelLoadingTracker>,
    mut tasks: ResMut<VoxelMeshingTasks>,
    mut load_gate: ResMut<VoxelLoadGate>,
    mut arena: ResMut<VoxelQuadArena>,
    mut uploads: ResMut<VoxelQuadUploadQueue>,
    world: Res<VoxelWorld>,
) {
    *phase = VoxelPhase::Loading;
    tracker.required.clear();
    tracker.completed.clear();
    tracker.finished = false;
    tasks.tasks.clear();
    load_gate.worldgen_ready = false;
    tasks.epoch = tasks.epoch.wrapping_add(1);
    load_gate.applied_world_id = None;
    arena.data.clear();
    arena.free.clear();
    arena.pending_updates.clear();
    arena.full_sync_required = true;
    arena.dirty = true;
    uploads.epoch = uploads.epoch.wrapping_add(1);
    uploads.quad_capacity = 0;
    uploads.full = Some(Arc::from([]));
    uploads.updates = Arc::from([]);
    world.storage.clear();
}

#[expect(
    clippy::too_many_arguments,
    reason = "chunk 流送系统需要同时访问配置、阶段、世界与现有实体查询"
)]
fn stream_chunks_system(
    mut commands: Commands,
    config: Res<VoxelConfig>,
    center: Res<VoxelCenter>,
    phase: Res<VoxelPhase>,
    load_gate: Res<VoxelLoadGate>,
    world: Res<VoxelWorld>,
    mut arena: ResMut<VoxelQuadArena>,
    existing: Query<(Entity, &ChunkKey, &ChunkDrawRange)>,
) {
    if matches!(*phase, VoxelPhase::Loading) && !load_gate.worldgen_ready {
        return;
    }

    let (center_key, _) = ChunkKey::from_world_voxel(center.0);
    let radius = match *phase {
        VoxelPhase::Loading => config.loading_ready_radius,
        VoxelPhase::Playing => config.playing_stream_radius,
    };

    let desired = desired_sphere(center_key, radius);

    let mut existing_keys = HashSet::new();
    for (_, key, _) in &existing {
        existing_keys.insert(*key);
    }

    // spawn missing
    for key in desired.iter().copied() {
        if existing_keys.contains(&key) {
            continue;
        }

        ensure_generated_chunk(&world, key);
        // 新 chunk 出现会改变边界可见面：强制通知 6 邻居 remesh。
        for n in key.neighbors_6() {
            world.storage.mark_dirty(n);
        }

        commands.spawn((
            key,
            // 0 表示“未提交过 meshing 结果”，避免空 chunk 永久重复 remesh。
            ChunkGeneration(0),
            ChunkBounds::from_key(key),
            ChunkDrawRange::default(),
        ));
    }

    // despawn outside (only in Playing)
    if matches!(*phase, VoxelPhase::Playing) {
        for (entity, key, range) in &existing {
            if desired.contains(key) {
                continue;
            }
            if range.opaque_len > 0 {
                arena.free(range.opaque_offset, range.opaque_len);
            }
            if range.cutout_len > 0 {
                arena.free(range.cutout_offset, range.cutout_len);
            }
            if range.transparent_len > 0 {
                arena.free(range.transparent_offset, range.transparent_len);
            }
            commands.entity(entity).despawn();
        }
    }
}

fn spawn_meshing_tasks_system(
    config: Res<VoxelConfig>,
    center: Res<VoxelCenter>,
    world: Res<VoxelWorld>,
    mut tasks: ResMut<VoxelMeshingTasks>,
    phase: Res<VoxelPhase>,
    load_gate: Res<VoxelLoadGate>,
    chunks: Query<(&ChunkKey, &ChunkGeneration)>,
) {
    if !world.defs.is_resolved() {
        return;
    }

    if matches!(*phase, VoxelPhase::Loading) && !load_gate.worldgen_ready {
        return;
    }

    if tasks.tasks.len() >= config.max_inflight_meshing {
        return;
    }

    let (center_key, _) = ChunkKey::from_world_voxel(center.0);

    // 近处优先：按 dist2 排序挑选需要 remesh 的 chunk。
    let mut candidates: Vec<(i32, ChunkKey, u32)> = Vec::new();
    for (key, last_gen) in &chunks {
        let Some(chunk) = world.storage.get_chunk(*key) else {
            continue;
        };
        let gen = chunk.generation();
        if gen == last_gen.0 && !chunk.is_dirty() {
            continue;
        }
        if tasks.tasks.contains_key(key) {
            continue;
        }

        // Loading 阶段只处理内圈，避免把 CPU 吃满导致进不了 Playing。
        if matches!(*phase, VoxelPhase::Loading) {
            let dx = key.x - center_key.x;
            let dy = key.y - center_key.y;
            let dz = key.z - center_key.z;
            if dx * dx + dy * dy + dz * dz
                > config.loading_ready_radius * config.loading_ready_radius
            {
                continue;
            }
        }

        let dx = key.x - center_key.x;
        let dy = key.y - center_key.y;
        let dz = key.z - center_key.z;
        let dist2 = dx * dx + dy * dy + dz * dz;
        candidates.push((dist2, *key, gen));
    }
    candidates.sort_by_key(|(d, _, _)| *d);

    let pool = AsyncComputeTaskPool::get();
    for (_, key, _gen) in candidates {
        if tasks.tasks.len() >= config.max_inflight_meshing {
            break;
        }

        let (padded, snapshot_gen) = world.storage.padded_snapshot(key);
        let input = MeshingInput {
            epoch: tasks.epoch,
            key,
            generation: snapshot_gen,
            padded,
        };
        let defs = world.defs.clone();

        let task = pool.spawn(async move { mesh(&input, &defs) });
        tasks.tasks.insert(key, task);
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "meshing 结果提交路径涉及 commands、task、tracker 与 flow 消息"
)]
fn poll_meshing_tasks_system(
    mut commands: Commands,
    world: Res<VoxelWorld>,
    mut tasks: ResMut<VoxelMeshingTasks>,
    mut arena: ResMut<VoxelQuadArena>,
    mut tracker: ResMut<VoxelLoadingTracker>,
    phase: Res<VoxelPhase>,
    mut flow: MessageWriter<FlowRequest>,
    mut chunks: Query<(Entity, &ChunkKey, &mut ChunkGeneration, &mut ChunkDrawRange)>,
) {
    let keys: Vec<ChunkKey> = tasks.tasks.keys().copied().collect();
    for key in keys {
        let Some(mut task) = tasks.tasks.remove(&key) else {
            continue;
        };
        if let Some(result) = future::block_on(poll_once(&mut task)) {
            if result.epoch != tasks.epoch {
                continue;
            }
            commit_meshing_result(
                &mut commands,
                &world,
                &mut arena,
                &mut chunks,
                &mut tracker,
                &phase,
                &mut flow,
                result,
            );
        } else {
            tasks.tasks.insert(key, task);
        }
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "提交函数需要原子更新 arena、chunk 组件与 loading 进度"
)]
fn commit_meshing_result(
    commands: &mut Commands,
    world: &VoxelWorld,
    arena: &mut VoxelQuadArena,
    chunks: &mut Query<(Entity, &ChunkKey, &mut ChunkGeneration, &mut ChunkDrawRange)>,
    tracker: &mut VoxelLoadingTracker,
    phase: &VoxelPhase,
    flow: &mut MessageWriter<FlowRequest>,
    result: MeshingOutput,
) {
    let Some(chunk) = world.storage.get_chunk(result.key) else {
        return;
    };
    let current_gen = chunk.generation();
    if result.is_stale_against(current_gen) {
        return;
    }

    // commit：写入 quad arena，并更新 chunk entity 的 draw range/generation。
    for (entity, key, mut gen, mut range) in chunks.iter_mut() {
        if *key != result.key {
            continue;
        }

        if range.opaque_len > 0 {
            arena.free(range.opaque_offset, range.opaque_len);
        }
        if range.cutout_len > 0 {
            arena.free(range.cutout_offset, range.cutout_len);
        }
        if range.transparent_len > 0 {
            arena.free(range.transparent_offset, range.transparent_len);
        }

        let opaque_u64: Vec<u64> = result.opaque.iter().map(|q| q.0).collect();
        let cutout_u64: Vec<u64> = result.cutout.iter().map(|q| q.0).collect();
        let transparent_u64: Vec<u64> = result.transparent.iter().map(|q| q.0).collect();

        let opaque_len = opaque_u64.len() as u32;
        let cutout_len = cutout_u64.len() as u32;
        let transparent_len = transparent_u64.len() as u32;

        let opaque_offset = arena.alloc(opaque_len);
        let cutout_offset = arena.alloc(cutout_len);
        let transparent_offset = arena.alloc(transparent_len);

        arena.write(opaque_offset, &opaque_u64);
        arena.write(cutout_offset, &cutout_u64);
        arena.write(transparent_offset, &transparent_u64);

        range.opaque_offset = opaque_offset;
        range.opaque_len = opaque_len;
        range.cutout_offset = cutout_offset;
        range.cutout_len = cutout_len;
        range.transparent_offset = transparent_offset;
        range.transparent_len = transparent_len;
        *gen = ChunkGeneration(result.generation);

        commands.entity(entity).insert(*range);

        // generation 一致时才允许 clear_dirty。
        chunk.clear_dirty();
        break;
    }

    if matches!(*phase, VoxelPhase::Loading) && tracker.required.contains(&result.key) {
        tracker.completed.insert(result.key);
        if !tracker.finished && tracker.completed.len() == tracker.required.len() {
            tracker.finished = true;
            flow.write(FlowRequest::FinishGameLoading);
        }
    }
}

fn sync_upload_queue_system(
    mut arena: ResMut<VoxelQuadArena>,
    mut uploads: ResMut<VoxelQuadUploadQueue>,
) {
    if !arena.dirty {
        return;
    }

    arena.dirty = false;
    uploads.epoch = uploads.epoch.wrapping_add(1);
    uploads.quad_capacity = arena.data.len() as u32;

    if arena.full_sync_required {
        uploads.full = Some(Arc::from(arena.data.clone().into_boxed_slice()));
        uploads.updates = Arc::from([]);
        arena.full_sync_required = false;
        arena.pending_updates.clear();
        return;
    }

    let updates = std::mem::take(&mut arena.pending_updates);
    uploads.full = None;
    uploads.updates = Arc::from(updates.into_boxed_slice());
}

#[expect(
    clippy::too_many_arguments,
    reason = "加载追踪需综合配置、中心点、任务与 chunk 代际状态"
)]
fn update_loading_tracker_system(
    config: Res<VoxelConfig>,
    center: Res<VoxelCenter>,
    phase: Res<VoxelPhase>,
    load_gate: Res<VoxelLoadGate>,
    world: Res<VoxelWorld>,
    tasks: Res<VoxelMeshingTasks>,
    mut tracker: ResMut<VoxelLoadingTracker>,
    chunks: Query<(&ChunkKey, &ChunkGeneration)>,
) {
    if !matches!(*phase, VoxelPhase::Loading) {
        return;
    }

    if !load_gate.worldgen_ready {
        tracker.required.clear();
        tracker.completed.clear();
        tracker.finished = false;
        return;
    }

    let (center_key, _) = ChunkKey::from_world_voxel(center.0);
    let desired = desired_sphere(center_key, config.loading_ready_radius);

    if desired == tracker.required {
        return;
    }

    tracker.required = desired;
    tracker.completed.clear();
    tracker.finished = false;

    // 若 required 更新时，有些 chunk 已经完成过 meshing（包含“全空气输出 0 quad”的情况），这里预填 completed，
    // 避免加载界面卡死在“等待已经完成的任务”上。
    for (key, last_gen) in &chunks {
        if !tracker.required.contains(key) {
            continue;
        }
        if tasks.tasks.contains_key(key) {
            continue;
        }
        let Some(chunk) = world.storage.get_chunk(*key) else {
            continue;
        };
        if chunk.is_dirty() {
            continue;
        }
        if chunk.generation() != last_gen.0 {
            continue;
        }
        tracker.completed.insert(*key);
    }
}

fn desired_sphere(center: ChunkKey, radius: i32) -> HashSet<ChunkKey> {
    let mut out = HashSet::new();
    let r2 = radius * radius;
    for dy in -radius..=radius {
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                if dx * dx + dy * dy + dz * dz > r2 {
                    continue;
                }
                out.insert(ChunkKey::new(center.x + dx, center.y + dy, center.z + dz));
            }
        }
    }
    out
}

impl ChunkBounds {
    pub fn from_key(key: ChunkKey) -> Self {
        let min = key.min_world_voxel();
        let max = min + IVec3::splat(CHUNK_SIZE);
        Self { min, max }
    }
}

fn ensure_generated_chunk(world: &VoxelWorld, key: ChunkKey) {
    let chunk = world.storage.get_or_create_chunk(key);
    if !chunk.is_dirty() && chunk.generation() > 1 {
        return;
    }

    let base = key.min_world_voxel();
    match world.generator.generate_chunk(key) {
        GeneratedChunk::SurfaceColumns(columns) => {
            chunk.fill_surface_columns(base.y, columns.as_ref());
        }
        GeneratedChunk::Dense(values) => {
            chunk.fill_direct(|lx, ly, lz| values[chunk_index(lx, ly, lz)]);
        }
    }

    // 生成只做一次；保持 generation，用于后续 meshing。
    chunk.clear_dirty();
}
