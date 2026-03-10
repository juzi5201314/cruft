use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use bevy::prelude::*;
use bevy::render::extract_component::ExtractComponent;
use bevy::render::extract_resource::ExtractResource;
use bevy::tasks::{futures_lite::future, poll_once, AsyncComputeTaskPool, Task};

use cruft_game_flow::{AppState, FlowRequest, InGameState};
use cruft_proc_textures::TextureRegistry;

use crate::blocks::BlockDefs;
use crate::coords::ChunkKey;
use crate::meshing::{mesh, MeshingInput, MeshingOutput};
use crate::storage::Storage;
use crate::terrain::Terrain;
use crate::CHUNK_SIZE;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub enum VoxelPhase {
    Loading,
    Playing,
}

impl Default for VoxelPhase {
    fn default() -> Self {
        Self::Loading
    }
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

/// 当前帧用于渲染的 PackedQuad 全量快照（由主世界生成，RenderApp 提取）。
#[derive(Resource, Debug, Default, Clone, ExtractResource)]
pub struct VoxelQuadStore {
    pub data: Arc<[u64]>,
}

#[derive(Resource)]
pub struct VoxelWorld {
    pub defs: BlockDefs,
    pub storage: Storage,
    pub terrain: Terrain,
}

impl Default for VoxelWorld {
    fn default() -> Self {
        Self {
            defs: BlockDefs::default(),
            storage: Storage::default(),
            terrain: Terrain::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Component, PartialEq, Eq)]
pub struct ChunkGeneration(pub u32);

#[derive(Debug, Clone, Copy, Component, PartialEq, Eq)]
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
}

#[derive(Resource, Default)]
struct VoxelQuadArena {
    data: Vec<u64>,
    free: Vec<FreeRange>,
    dirty: bool,
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
    }
}

pub struct VoxelPlugin;

impl Plugin for VoxelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VoxelConfig>()
            .init_resource::<VoxelCenter>()
            .init_resource::<VoxelPhase>()
            .init_resource::<VoxelWorld>()
            .init_resource::<VoxelQuadStore>()
            .init_resource::<VoxelLoadingTracker>()
            .init_resource::<VoxelMeshingTasks>()
            .init_resource::<VoxelQuadArena>()
            .add_plugins(crate::render::VoxelRenderPlugin)
            .add_systems(OnEnter(InGameState::Loading), start_voxel_loading)
            .add_systems(OnEnter(InGameState::Playing), enter_voxel_playing)
            .add_systems(OnExit(AppState::InGame), cleanup_voxel_world)
            .add_systems(
                Update,
                (
                    update_loading_tracker_system,
                    stream_chunks_system,
                    spawn_meshing_tasks_system,
                    poll_meshing_tasks_system,
                    sync_quad_store_system,
                )
                    .chain()
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

fn start_voxel_loading(
    mut phase: ResMut<VoxelPhase>,
    config: Res<VoxelConfig>,
    texture_registry: Option<Res<TextureRegistry>>,
    mut tracker: ResMut<VoxelLoadingTracker>,
    mut world: ResMut<VoxelWorld>,
    mut arena: ResMut<VoxelQuadArena>,
    mut store: ResMut<VoxelQuadStore>,
) {
    *phase = VoxelPhase::Loading;

    // “退出存档后清空”：每次进入 Loading 都重置世界与渲染数据。
    world.storage.clear();
    arena.data.clear();
    arena.free.clear();
    arena.dirty = true;
    store.data = Arc::from([]);
    tracker.required.clear();
    tracker.completed.clear();
    tracker.finished = false;


    if let Some(texture_registry) = texture_registry {
        match BlockDefs::from_registry(&texture_registry) {
            Ok(defs) => world.defs = defs,
            Err(message) => log::error!("Failed to resolve BlockDefs from texture registry: {message}"),
        }
    }
    let _ = &config;
    // required/completed 由 Update 阶段按实时 VoxelCenter 维护（避免“中心点更新后 required 永远达不成”）。
    tracker.required.clear();
}

fn enter_voxel_playing(mut phase: ResMut<VoxelPhase>) {
    *phase = VoxelPhase::Playing;
}

fn cleanup_voxel_world(
    mut phase: ResMut<VoxelPhase>,
    mut tracker: ResMut<VoxelLoadingTracker>,
    mut tasks: ResMut<VoxelMeshingTasks>,
    mut arena: ResMut<VoxelQuadArena>,
    mut store: ResMut<VoxelQuadStore>,
    world: Res<VoxelWorld>,
) {
    *phase = VoxelPhase::Loading;
    tracker.required.clear();
    tracker.completed.clear();
    tracker.finished = false;
    tasks.tasks.clear();
    arena.data.clear();
    arena.free.clear();
    arena.dirty = true;
    store.data = Arc::from([]);
    world.storage.clear();
}

fn stream_chunks_system(
    mut commands: Commands,
    config: Res<VoxelConfig>,
    center: Res<VoxelCenter>,
    phase: Res<VoxelPhase>,
    world: Res<VoxelWorld>,
    mut arena: ResMut<VoxelQuadArena>,
    existing: Query<(Entity, &ChunkKey, &ChunkDrawRange)>,
) {
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

        ensure_generated_chunk(&world.storage, &world.terrain, key);
        // 新 chunk 出现会改变边界可见面：强制通知 6 邻居 remesh。
        for n in key.neighbors_6() {
            world.storage.mark_dirty(n);
        }

        let generation = world
            .storage
            .get_chunk(key)
            .map(|c| c.generation())
            .unwrap_or(0);

        commands.spawn((
            key,
            ChunkGeneration(generation),
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
                arena.dirty = true;
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
    chunks: Query<(&ChunkKey, &ChunkGeneration, &ChunkDrawRange)>,
) {
    if tasks.tasks.len() >= config.max_inflight_meshing {
        return;
    }

    let (center_key, _) = ChunkKey::from_world_voxel(center.0);

    // 近处优先：按 dist2 排序挑选需要 remesh 的 chunk。
    let mut candidates: Vec<(i32, ChunkKey, u32)> = Vec::new();
    for (key, last_gen, draw) in &chunks {
        let Some(chunk) = world.storage.get_chunk(*key) else {
            continue;
        };
        let gen = chunk.generation();
        if gen == last_gen.0 && draw.opaque_len != 0 && !chunk.is_dirty() {
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
            key,
            generation: snapshot_gen,
            padded,
        };
        let defs = world.defs.clone();

        let task = pool.spawn(async move { mesh(&input, &defs) });
        tasks.tasks.insert(key, task);
    }
}

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

        let opaque_u64: Vec<u64> = result.opaque.iter().map(|q| q.0).collect();
        let len = opaque_u64.len() as u32;
        let offset = arena.alloc(len);
        arena.write(offset, &opaque_u64);

        range.opaque_offset = offset;
        range.opaque_len = len;
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

fn sync_quad_store_system(mut arena: ResMut<VoxelQuadArena>, mut store: ResMut<VoxelQuadStore>) {
    if !arena.dirty {
        return;
    }
    arena.dirty = false;
    store.data = Arc::from(arena.data.clone());
}

fn update_loading_tracker_system(
    config: Res<VoxelConfig>,
    center: Res<VoxelCenter>,
    phase: Res<VoxelPhase>,
    world: Res<VoxelWorld>,
    tasks: Res<VoxelMeshingTasks>,
    mut tracker: ResMut<VoxelLoadingTracker>,
    chunks: Query<(&ChunkKey, &ChunkGeneration)>,
) {
    if !matches!(*phase, VoxelPhase::Loading) {
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

fn ensure_generated_chunk(storage: &Storage, terrain: &Terrain, key: ChunkKey) {
    let chunk = storage.get_or_create_chunk(key);
    if !chunk.is_dirty() && chunk.generation() > 1 {
        return;
    }

    let base = key.min_world_voxel();
    // 性能：Playing 半径 8 会覆盖大量 chunk；这里按 brick/heightmap 快速判定全 AIR / 全 STONE，
    // 只对地表附近少量 brick 逐体素填充，避免 32^3 全填。
    let mut heights = [0i32; 32 * 32];
    for lz in 0..32i32 {
        for lx in 0..32i32 {
            heights[(lx as usize) + (lz as usize) * 32] =
                terrain.height_at(base.x + lx, base.z + lz);
        }
    }
    chunk.fill_terrain_heightmap(base.y, &heights);
    // 生成只做一次；保持 generation，用于后续 meshing。
    chunk.clear_dirty();
}
