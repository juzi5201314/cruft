//! 体素渲染：Packed-Quad + vertex pulling。
//!
//! 当前实现重点：
//! - CPU 侧增量上传 quad 数据（避免每帧全量重传）
//! - GPU frustum culling（compute 生成 indirect 命令）
//! - MDI/间接绘制提交（单次或少量 API 调用绘制所有 chunk）

use std::collections::{HashMap, HashSet};

use bevy::{
    core_pipeline::core_3d::{graph::Core3d, graph::Node3d, CORE_3D_DEPTH_FORMAT},
    core_pipeline::experimental::mip_generation::ViewDepthPyramid,
    ecs::query::QueryItem,
    prelude::*,
    render::{
        extract_component::ExtractComponentPlugin,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_graph::{
            NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner,
        },
        render_resource::{
            binding_types::{
                sampler, storage_buffer, storage_buffer_read_only, texture_2d, texture_2d_array,
                uniform_buffer,
            },
            *,
        },
        renderer::{RenderContext, RenderDevice, RenderQueue},
        texture::GpuImage,
        view::{
            ExtractedView, Msaa, ViewDepthTexture, ViewTarget, ViewUniform, ViewUniformOffset,
            ViewUniforms,
        },
        Render, RenderApp, RenderStartup, RenderSystems,
    },
};

use cruft_proc_textures::TextureRuntimePacks;

use crate::coords::ChunkKey;
use crate::world::{ChunkBounds, ChunkDrawRange, VoxelMaterialTable, VoxelQuadUploadQueue};

const SHADER_ASSET_PATH: &str = "shaders/voxel_quads.wgsl";
const CULL_SHADER_ASSET_PATH: &str = "shaders/voxel_cull.wgsl";
const VOXEL_SAMPLING_ENV: &str = "CRUFT_VOXEL_SAMPLING";
const MAX_MATERIAL_KEYS: usize = 256;
const MATERIAL_TABLE_STRIDE_U32: usize = 32;
const CHUNK_META_V2_STRIDE_U32: usize = 16;
#[allow(
    dead_code,
    reason = "VisibleChunkRecord ABI 常量在后续 compaction 消费阶段使用"
)]
const VISIBLE_CHUNK_RECORD_STRIDE_U32: usize = 4;
const VOXEL_DRAW_RECORD_STRIDE_U32: usize = 4;
#[allow(
    dead_code,
    reason = "Per-view counters/flags ABI 常量在后续 HZB/compaction 任务使用"
)]
const PER_VIEW_COUNTERS_STRIDE_U32: usize = 4;
const CULL_WORKGROUP_SIZE: u32 = 64;

#[derive(Resource, Clone)]
struct ExtractedTextureRuntimePacks(pub TextureRuntimePacks);

impl ExtractResource for ExtractedTextureRuntimePacks {
    type Source = TextureRuntimePacks;

    fn extract_resource(source: &Self::Source) -> Self {
        Self(source.clone())
    }
}

#[derive(Resource, Clone, Default)]
struct ExtractedVoxelMaterialTable(pub VoxelMaterialTable);

impl ExtractResource for ExtractedVoxelMaterialTable {
    type Source = VoxelMaterialTable;

    fn extract_resource(source: &Self::Source) -> Self {
        Self(source.clone())
    }
}

#[derive(Clone, Copy, ShaderType)]
struct VoxelCullingUniform {
    clip_from_world: Mat4,
    chunk_count: u32,
    hzb_mip_count: u32,
    hzb_enabled: u32,
    _pad0: u32,
    hzb_size: UVec2,
    _pad1: UVec2,
}

impl Default for VoxelCullingUniform {
    fn default() -> Self {
        Self {
            clip_from_world: Mat4::IDENTITY,
            chunk_count: 0,
            hzb_mip_count: 0,
            hzb_enabled: 0,
            _pad0: 0,
            hzb_size: UVec2::ONE,
            _pad1: UVec2::ZERO,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ChunkMetaV2 {
    origin: IVec3,
    min: IVec3,
    max: IVec3,
    opaque_offset: u32,
    opaque_len: u32,
    cutout_offset: u32,
    cutout_len: u32,
    flags: u32,
    reserved0: u32,
    reserved1: u32,
}

impl ChunkMetaV2 {
    const ORIGIN_X_OFFSET: usize = 0;
    const ORIGIN_Y_OFFSET: usize = 1;
    const ORIGIN_Z_OFFSET: usize = 2;
    const OPAQUE_OFFSET_OFFSET: usize = 3;
    const MIN_X_OFFSET: usize = 4;
    const MIN_Y_OFFSET: usize = 5;
    const MIN_Z_OFFSET: usize = 6;
    const OPAQUE_LEN_OFFSET: usize = 7;
    const MAX_X_OFFSET: usize = 8;
    const MAX_Y_OFFSET: usize = 9;
    const MAX_Z_OFFSET: usize = 10;
    const CUTOUT_OFFSET_OFFSET: usize = 11;
    const CUTOUT_LEN_OFFSET: usize = 12;
    const FLAGS_OFFSET: usize = 13;
    const RESERVED0_OFFSET: usize = 14;
    const RESERVED1_OFFSET: usize = 15;

    fn to_words(self) -> [u32; CHUNK_META_V2_STRIDE_U32] {
        let mut words = [0u32; CHUNK_META_V2_STRIDE_U32];
        words[Self::ORIGIN_X_OFFSET] = self.origin.x as u32;
        words[Self::ORIGIN_Y_OFFSET] = self.origin.y as u32;
        words[Self::ORIGIN_Z_OFFSET] = self.origin.z as u32;
        words[Self::OPAQUE_OFFSET_OFFSET] = self.opaque_offset;
        words[Self::MIN_X_OFFSET] = self.min.x as u32;
        words[Self::MIN_Y_OFFSET] = self.min.y as u32;
        words[Self::MIN_Z_OFFSET] = self.min.z as u32;
        words[Self::OPAQUE_LEN_OFFSET] = self.opaque_len;
        words[Self::MAX_X_OFFSET] = self.max.x as u32;
        words[Self::MAX_Y_OFFSET] = self.max.y as u32;
        words[Self::MAX_Z_OFFSET] = self.max.z as u32;
        words[Self::CUTOUT_OFFSET_OFFSET] = self.cutout_offset;
        words[Self::CUTOUT_LEN_OFFSET] = self.cutout_len;
        words[Self::FLAGS_OFFSET] = self.flags;
        words[Self::RESERVED0_OFFSET] = self.reserved0;
        words[Self::RESERVED1_OFFSET] = self.reserved1;
        words
    }
}

#[allow(
    dead_code,
    reason = "VisibleChunkRecord ABI 先锁合同，后续任务再接入 compaction 消费"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VisibleChunkRecord {
    chunk_index: u32,
    layer_mask: u32,
    draw_record_base: u32,
    reserved: u32,
}

#[allow(
    dead_code,
    reason = "VisibleChunkRecord ABI 先锁合同，后续任务再接入 compaction 消费"
)]
impl VisibleChunkRecord {
    const CHUNK_INDEX_OFFSET: usize = 0;
    const LAYER_MASK_OFFSET: usize = 1;
    const DRAW_RECORD_BASE_OFFSET: usize = 2;
    const RESERVED_OFFSET: usize = 3;

    fn to_words(self) -> [u32; VISIBLE_CHUNK_RECORD_STRIDE_U32] {
        [
            self.chunk_index,
            self.layer_mask,
            self.draw_record_base,
            self.reserved,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VoxelDrawRecord {
    chunk_meta_index: u32,
    first_instance: u32,
    layer_mask: u32,
    reserved: u32,
}

impl VoxelDrawRecord {
    const CHUNK_META_INDEX_OFFSET: usize = 0;
    const FIRST_INSTANCE_OFFSET: usize = 1;
    #[cfg(test)]
    const LAYER_MASK_OFFSET: usize = 2;
    #[cfg(test)]
    const RESERVED_OFFSET: usize = 3;

    fn to_words(self) -> [u32; VOXEL_DRAW_RECORD_STRIDE_U32] {
        [
            self.chunk_meta_index,
            self.first_instance,
            self.layer_mask,
            self.reserved,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VoxelIndirectArgs {
    vertex_count: u32,
    instance_count: u32,
    first_vertex: u32,
    first_instance: u32,
}

impl VoxelIndirectArgs {
    const VERTEX_COUNT_OFFSET: usize = 0;
    const INSTANCE_COUNT_OFFSET: usize = 1;
    const FIRST_VERTEX_OFFSET: usize = 2;
    const FIRST_INSTANCE_OFFSET: usize = 3;

    fn to_words(self) -> [u32; VOXEL_DRAW_RECORD_STRIDE_U32] {
        [
            self.vertex_count,
            self.instance_count,
            self.first_vertex,
            self.first_instance,
        ]
    }
}

#[allow(
    dead_code,
    reason = "Per-view counters/flags ABI 先锁固定 stride，后续任务再接入读写行为"
)]
mod per_view_counters_layout {
    pub(super) const VISIBLE_CHUNK_COUNT_OFFSET: usize = 0;
    pub(super) const FLAGS_OFFSET: usize = 1;
    pub(super) const RESERVED0_OFFSET: usize = 2;
    pub(super) const RESERVED1_OFFSET: usize = 3;
}

#[derive(Resource)]
struct VoxelGpuBuffers {
    /// 以 `u32` 序列上传：每个 quad 占 2 个 u32（low/high）。
    quads_u32: RawBufferVec<u32>,
    /// 运行时材质表：按 material_id 索引，每项固定 32 个 u32。
    material_table_u32: RawBufferVec<u32>,
    /// chunk 元数据表，固定 stride=16*u32（见 `voxel_quads.wgsl` / `voxel_cull.wgsl`）。
    chunk_meta_u32: RawBufferVec<u32>,
    draw_records_u32: RawBufferVec<u32>,
    uploaded_epoch: u64,
    last_material_table: Vec<u32>,
}

impl Default for VoxelGpuBuffers {
    fn default() -> Self {
        let mut quads_u32 = RawBufferVec::new(BufferUsages::STORAGE);
        quads_u32.set_label(Some("voxel_quads_u32"));
        let mut material_table_u32 = RawBufferVec::new(BufferUsages::STORAGE);
        material_table_u32.set_label(Some("voxel_material_table_u32"));
        let mut chunk_meta_u32 = RawBufferVec::new(BufferUsages::STORAGE);
        chunk_meta_u32.set_label(Some("voxel_chunk_meta_u32"));
        let mut draw_records_u32 = RawBufferVec::new(BufferUsages::STORAGE);
        draw_records_u32.set_label(Some("voxel_draw_records_u32"));
        Self {
            quads_u32,
            material_table_u32,
            chunk_meta_u32,
            draw_records_u32,
            uploaded_epoch: 0,
            last_material_table: Vec::new(),
        }
    }
}

impl VoxelGpuBuffers {
    fn chunk_count(&self) -> u32 {
        (self.draw_records_u32.len() / VOXEL_DRAW_RECORD_STRIDE_U32) as u32
    }
}

#[derive(Resource)]
struct VoxelRenderPipeline {
    view_layout: BindGroupLayoutDescriptor,
    voxel_layout: BindGroupLayoutDescriptor,
    culling_layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    culling_fallback_hzb: TextureView,
    shader: Handle<Shader>,
    culling_pipeline: CachedComputePipelineId,
    supports_multi_draw_indirect: bool,
    pipelines: HashMap<(TextureFormat, u32), CachedRenderPipelineId>,
}

#[derive(Resource, Default)]
struct VoxelRenderBindGroups {
    voxel: Option<BindGroup>,
}

#[derive(Default)]
struct VoxelViewBindGroups {
    view: Option<BindGroup>,
    culling: Option<BindGroup>,
}

#[allow(
    dead_code,
    reason = "Cutout layer is kept as a per-view placeholder for follow-up render tasks"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum VoxelDrawLayer {
    Opaque,
    Cutout,
}

#[allow(
    dead_code,
    reason = "Per-view placeholder handles/counters are reserved for follow-up GPU culling tasks"
)]
struct VoxelViewState {
    culling_uniform: UniformBuffer<VoxelCullingUniform>,
    bind_groups: VoxelViewBindGroups,
    previous_depth_texture: Option<TextureView>,
    previous_hzb_texture: Option<TextureView>,
    visible_chunk_list: Option<Buffer>,
    visible_chunk_counter: Option<Buffer>,
    indirect_u32_by_layer: HashMap<VoxelDrawLayer, RawBufferVec<u32>>,
}

impl Default for VoxelViewState {
    fn default() -> Self {
        let mut indirect_u32_by_layer = HashMap::new();
        indirect_u32_by_layer.insert(
            VoxelDrawLayer::Opaque,
            new_view_indirect_buffer(VoxelDrawLayer::Opaque),
        );
        Self {
            culling_uniform: UniformBuffer::default(),
            bind_groups: VoxelViewBindGroups::default(),
            previous_depth_texture: None,
            previous_hzb_texture: None,
            visible_chunk_list: None,
            visible_chunk_counter: None,
            indirect_u32_by_layer,
        }
    }
}

impl VoxelViewState {
    fn indirect_u32(&self, layer: VoxelDrawLayer) -> Option<&RawBufferVec<u32>> {
        self.indirect_u32_by_layer.get(&layer)
    }

    fn indirect_u32_mut(&mut self, layer: VoxelDrawLayer) -> &mut RawBufferVec<u32> {
        self.indirect_u32_by_layer
            .entry(layer)
            .or_insert_with(|| new_view_indirect_buffer(layer))
    }
}

#[derive(Resource, Default)]
struct VoxelViewStates {
    by_entity: HashMap<Entity, VoxelViewState>,
}

impl VoxelViewStates {
    fn sync_active_views(&mut self, active_views: impl IntoIterator<Item = Entity>) {
        let active_views = HashSet::<Entity>::from_iter(active_views);
        self.by_entity
            .retain(|entity, _| active_views.contains(entity));
        for entity in active_views {
            self.by_entity.entry(entity).or_default();
        }
    }

    fn view_state(&self, entity: Entity) -> Option<&VoxelViewState> {
        self.by_entity.get(&entity)
    }

    fn view_state_mut(&mut self, entity: Entity) -> &mut VoxelViewState {
        self.by_entity.entry(entity).or_default()
    }

    fn clear_bind_groups(&mut self) {
        for state in self.by_entity.values_mut() {
            state.bind_groups.view = None;
            state.bind_groups.culling = None;
        }
    }
}

fn new_view_indirect_buffer(layer: VoxelDrawLayer) -> RawBufferVec<u32> {
    let mut indirect_u32 = RawBufferVec::new(BufferUsages::INDIRECT | BufferUsages::STORAGE);
    let label = match layer {
        VoxelDrawLayer::Opaque => "voxel_view_opaque_indirect_u32",
        VoxelDrawLayer::Cutout => "voxel_view_cutout_indirect_u32",
    };
    indirect_u32.set_label(Some(label));
    indirect_u32
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct VoxelOpaquePassLabel;

pub struct VoxelRenderPlugin;

impl Plugin for VoxelRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ExtractResourcePlugin::<VoxelQuadUploadQueue>::default(),
            ExtractResourcePlugin::<ExtractedTextureRuntimePacks>::default(),
            ExtractResourcePlugin::<ExtractedVoxelMaterialTable>::default(),
            ExtractComponentPlugin::<ChunkKey>::default(),
            ExtractComponentPlugin::<ChunkBounds>::default(),
            ExtractComponentPlugin::<ChunkDrawRange>::default(),
        ));

        let render_app = app.sub_app_mut(RenderApp);
        render_app
            .init_resource::<VoxelGpuBuffers>()
            .init_resource::<VoxelRenderBindGroups>()
            .init_resource::<VoxelViewStates>()
            .add_systems(RenderStartup, init_voxel_render_pipeline)
            .add_systems(
                Render,
                prepare_voxel_gpu_buffers.in_set(RenderSystems::PrepareResources),
            )
            .add_systems(
                Render,
                prepare_voxel_bind_groups.in_set(RenderSystems::PrepareBindGroups),
            )
            .add_render_graph_node::<ViewNodeRunner<VoxelOpaquePassNode>>(
                Core3d,
                VoxelOpaquePassLabel,
            )
            .add_render_graph_edges(
                Core3d,
                (
                    Node3d::MainOpaquePass,
                    VoxelOpaquePassLabel,
                    Node3d::MainTransmissivePass,
                ),
            );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VoxelSamplingMode {
    Pixel,
    Smooth,
}

impl VoxelSamplingMode {
    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "pixel" => Some(Self::Pixel),
            "smooth" => Some(Self::Smooth),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Pixel => "pixel",
            Self::Smooth => "smooth",
        }
    }

    fn sampler_descriptor(self) -> SamplerDescriptor<'static> {
        match self {
            Self::Pixel => SamplerDescriptor {
                label: Some("voxel_sampler_pixel"),
                mag_filter: FilterMode::Nearest,
                min_filter: FilterMode::Nearest,
                mipmap_filter: FilterMode::Linear,
                ..default()
            },
            Self::Smooth => SamplerDescriptor {
                label: Some("voxel_sampler_smooth"),
                mag_filter: FilterMode::Linear,
                min_filter: FilterMode::Linear,
                mipmap_filter: FilterMode::Linear,
                anisotropy_clamp: 16,
                ..default()
            },
        }
    }
}

fn resolve_voxel_sampling_mode() -> VoxelSamplingMode {
    let Ok(raw) = std::env::var(VOXEL_SAMPLING_ENV) else {
        return VoxelSamplingMode::Pixel;
    };
    let Some(mode) = VoxelSamplingMode::parse(&raw) else {
        log::warn!(
            "{VOXEL_SAMPLING_ENV}={raw:?} 无效；可选值为 \"pixel\" / \"smooth\"，已回退到 \"pixel\""
        );
        return VoxelSamplingMode::Pixel;
    };
    mode
}

fn init_voxel_render_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    asset_server: Res<AssetServer>,
    pipeline_cache: Res<PipelineCache>,
) {
    let view_layout = BindGroupLayoutDescriptor::new(
        "voxel_view_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX,
            (uniform_buffer::<ViewUniform>(true),),
        ),
    );

    let voxel_layout = BindGroupLayoutDescriptor::new(
        "voxel_voxel_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX_FRAGMENT,
            (
                // chunk 元数据表（stride=16*u32）。
                storage_buffer_read_only::<u32>(false),
                storage_buffer_read_only::<u32>(false),
                // quad buffer：array<u32>，按 2 u32 / quad。
                storage_buffer_read_only::<u32>(false),
                // material_id -> six faces x five channels 的运行时材质表。
                storage_buffer_read_only::<u32>(false),
                texture_2d_array(TextureSampleType::Float { filterable: true }),
                texture_2d_array(TextureSampleType::Float { filterable: true }),
                texture_2d_array(TextureSampleType::Float { filterable: true }),
                texture_2d_array(TextureSampleType::Float { filterable: true }),
                texture_2d_array(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
            ),
        ),
    );

    let culling_layout = BindGroupLayoutDescriptor::new(
        "voxel_culling_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                uniform_buffer::<VoxelCullingUniform>(false),
                storage_buffer_read_only::<u32>(false),
                storage_buffer::<u32>(false),
                texture_2d(TextureSampleType::Float { filterable: false }),
            ),
        ),
    );

    let sampling_mode = resolve_voxel_sampling_mode();
    let sampler = render_device.create_sampler(&sampling_mode.sampler_descriptor());
    log::info!(
        "Voxel 纹理采样模式：{}（env: {VOXEL_SAMPLING_ENV}）",
        sampling_mode.as_str()
    );

    let shader = asset_server.load(SHADER_ASSET_PATH);
    let culling_shader = asset_server.load(CULL_SHADER_ASSET_PATH);
    let culling_fallback_hzb = render_device
        .create_texture(&TextureDescriptor {
            label: Some("voxel_culling_fallback_hzb"),
            size: Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::R32Float,
            usage: TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&TextureViewDescriptor::default());
    let culling_pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("voxel_chunk_culling_pipeline".into()),
        layout: vec![culling_layout.clone()],
        shader: culling_shader.clone(),
        shader_defs: vec![],
        entry_point: Some("cull".into()),
        push_constant_ranges: vec![],
        zero_initialize_workgroup_memory: true,
    });

    // wgpu 27 已移除 MULTI_DRAW_INDIRECT 特性位；当前后端统一支持（必要时由驱动/后端模拟）。
    let supports_multi_draw_indirect = true;

    commands.insert_resource(VoxelRenderPipeline {
        view_layout,
        voxel_layout,
        culling_layout,
        sampler,
        culling_fallback_hzb,
        shader,
        culling_pipeline,
        supports_multi_draw_indirect,
        pipelines: HashMap::new(),
    });
}

#[expect(
    clippy::too_many_arguments,
    reason = "RenderApp prepare 系统需要同时访问 view、buffer、pipeline 与 GPU 资源"
)]
fn prepare_voxel_gpu_buffers(
    views: Query<(
        Entity,
        &ExtractedView,
        &ViewTarget,
        &Msaa,
        Option<&ViewDepthPyramid>,
    )>,
    quads: Res<VoxelQuadUploadQueue>,
    material_table: Option<Res<ExtractedVoxelMaterialTable>>,
    chunks: Query<(&ChunkKey, &ChunkBounds, &ChunkDrawRange)>,
    mut buffers: ResMut<VoxelGpuBuffers>,
    mut view_states: ResMut<VoxelViewStates>,
    mut pipeline: ResMut<VoxelRenderPipeline>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    pipeline_cache: Res<PipelineCache>,
) {
    upload_quads_if_needed(&quads, &mut buffers, &render_device, &render_queue);
    upload_material_table_if_needed(
        material_table.as_deref(),
        &mut buffers,
        &render_device,
        &render_queue,
    );

    let entries = collect_chunk_meta_entries(chunks.iter());
    sync_chunk_meta_buffer(&entries, &mut buffers, &render_device, &render_queue);
    sync_draw_records_buffer(&entries, &mut buffers, &render_device, &render_queue);
    let chunk_count = buffers.chunk_count();

    let active_views: Vec<_> = views.iter().map(|(entity, ..)| entity).collect();
    view_states.sync_active_views(active_views.iter().copied());

    let compute_pipeline_ready = pipeline_cache
        .get_compute_pipeline(pipeline.culling_pipeline)
        .is_some();

    for (view_entity, view, view_target, msaa, view_depth_pyramid) in &views {
        let clip_from_world = view
            .clip_from_world
            .unwrap_or_else(|| view.clip_from_view * view.world_from_view.to_matrix().inverse());

        let view_state = view_states.view_state_mut(view_entity);

        let (hzb_mip_count, hzb_enabled, hzb_size) = match view_depth_pyramid {
            Some(depth_pyramid) if depth_pyramid.mip_count > 0 => {
                let viewport = view.viewport.zw();
                let mip0 =
                    UVec2::new(viewport.x.div_ceil(2), viewport.y.div_ceil(2)).max(UVec2::ONE);
                (depth_pyramid.mip_count, 1, mip0)
            }
            _ => (0, 0, UVec2::ONE),
        };
        view_state.culling_uniform.set(VoxelCullingUniform {
            clip_from_world,
            chunk_count,
            hzb_mip_count,
            hzb_enabled,
            _pad0: 0,
            hzb_size,
            _pad1: UVec2::ZERO,
        });
        if chunk_count > 0 {
            view_state
                .culling_uniform
                .write_buffer(&render_device, &render_queue);
        }

        {
            let indirect_u32 = view_state.indirect_u32_mut(VoxelDrawLayer::Opaque);
            ensure_indirect_capacity(chunk_count, indirect_u32, &render_device, &render_queue);

            // pipeline 热身阶段 fallback：compute pipeline 未就绪时用 CPU 先填 indirect。
            if !compute_pipeline_ready {
                build_indirect_cpu_fallback(
                    &entries,
                    buffers.draw_records_u32.values().as_slice(),
                    clip_from_world,
                    indirect_u32,
                    &render_device,
                    &render_queue,
                );
            }
        }

        // pipeline：按 view format + msaa samples 做最小特化。
        let format = view_target.main_texture_format();
        let samples = msaa.samples();

        if !pipeline.pipelines.contains_key(&(format, samples)) {
            let view_layout = pipeline.view_layout.clone();
            let voxel_layout = pipeline.voxel_layout.clone();
            let shader = pipeline.shader.clone();
            let id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
                label: Some("voxel_quads_pipeline".into()),
                layout: vec![view_layout, voxel_layout],
                vertex: VertexState {
                    shader: shader.clone(),
                    entry_point: Some("vertex".into()),
                    shader_defs: vec![],
                    buffers: vec![],
                },
                fragment: Some(FragmentState {
                    shader,
                    entry_point: Some("fragment".into()),
                    shader_defs: vec![],
                    targets: vec![Some(ColorTargetState {
                        format,
                        blend: None,
                        write_mask: ColorWrites::ALL,
                    })],
                }),
                primitive: PrimitiveState {
                    topology: PrimitiveTopology::TriangleList,
                    cull_mode: Some(Face::Back),
                    ..default()
                },
                depth_stencil: Some(DepthStencilState {
                    format: CORE_3D_DEPTH_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: CompareFunction::GreaterEqual,
                    stencil: StencilState::default(),
                    bias: DepthBiasState::default(),
                }),
                multisample: MultisampleState {
                    count: samples,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                push_constant_ranges: vec![],
                zero_initialize_workgroup_memory: true,
            });
            pipeline.pipelines.insert((format, samples), id);
        }
    }
}

fn collect_chunk_meta_entries<'a>(
    chunks: impl IntoIterator<Item = (&'a ChunkKey, &'a ChunkBounds, &'a ChunkDrawRange)>,
) -> Vec<(ChunkKey, ChunkMetaV2)> {
    let mut out = Vec::new();
    for (key, bounds, range) in chunks {
        if range.opaque_len == 0 && range.cutout_len == 0 {
            continue;
        }
        out.push((
            *key,
            ChunkMetaV2 {
                origin: key.min_world_voxel(),
                min: bounds.min,
                max: bounds.max,
                opaque_offset: range.opaque_offset,
                opaque_len: range.opaque_len,
                cutout_offset: range.cutout_offset,
                cutout_len: range.cutout_len,
                flags: 0,
                reserved0: 0,
                reserved1: 0,
            },
        ));
    }
    out.sort_by_key(|(key, _)| *key);
    out
}

fn pack_chunk_meta_u32(entries: &[(ChunkKey, ChunkMetaV2)]) -> Vec<u32> {
    let mut packed = Vec::with_capacity(entries.len() * CHUNK_META_V2_STRIDE_U32);
    for (_, entry) in entries {
        packed.extend_from_slice(&entry.to_words());
    }
    packed
}

fn build_draw_records(entries: &[(ChunkKey, ChunkMetaV2)]) -> Vec<VoxelDrawRecord> {
    let mut draw_records = Vec::with_capacity(entries.len());
    for (chunk_meta_index, (_, entry)) in entries.iter().enumerate() {
        draw_records.push(VoxelDrawRecord {
            chunk_meta_index: chunk_meta_index as u32,
            first_instance: entry.opaque_offset,
            layer_mask: 0b01,
            reserved: 0,
        });
    }
    draw_records
}

fn pack_draw_records_u32(draw_records: &[VoxelDrawRecord]) -> Vec<u32> {
    let mut packed = Vec::with_capacity(draw_records.len() * VOXEL_DRAW_RECORD_STRIDE_U32);
    for record in draw_records {
        packed.extend_from_slice(&record.to_words());
    }
    packed
}

fn sync_chunk_meta_buffer(
    entries: &[(ChunkKey, ChunkMetaV2)],
    buffers: &mut VoxelGpuBuffers,
    render_device: &RenderDevice,
    render_queue: &RenderQueue,
) {
    let packed = pack_chunk_meta_u32(entries);
    if buffers.chunk_meta_u32.values().as_slice() == packed.as_slice() {
        return;
    }
    *buffers.chunk_meta_u32.values_mut() = packed;
    if !buffers.chunk_meta_u32.values().is_empty() {
        buffers
            .chunk_meta_u32
            .write_buffer(render_device, render_queue);
    }
}

fn sync_draw_records_buffer(
    entries: &[(ChunkKey, ChunkMetaV2)],
    buffers: &mut VoxelGpuBuffers,
    render_device: &RenderDevice,
    render_queue: &RenderQueue,
) {
    let packed = pack_draw_records_u32(&build_draw_records(entries));
    if buffers.draw_records_u32.values().as_slice() == packed.as_slice() {
        return;
    }
    *buffers.draw_records_u32.values_mut() = packed;
    if !buffers.draw_records_u32.values().is_empty() {
        buffers
            .draw_records_u32
            .write_buffer(render_device, render_queue);
    }
}

fn ensure_indirect_capacity(
    chunk_count: u32,
    indirect_u32: &mut RawBufferVec<u32>,
    render_device: &RenderDevice,
    render_queue: &RenderQueue,
) {
    let target_len = (chunk_count as usize) * VOXEL_DRAW_RECORD_STRIDE_U32;
    if !resize_indirect_buffer(indirect_u32, chunk_count) {
        return;
    }
    if target_len > 0 {
        // 只做容量保障；命令内容由 compute pass 覆写。
        indirect_u32.write_buffer(render_device, render_queue);
    }
}

fn resize_indirect_buffer(indirect_u32: &mut RawBufferVec<u32>, chunk_count: u32) -> bool {
    let target_len = (chunk_count as usize) * VOXEL_DRAW_RECORD_STRIDE_U32;
    if indirect_u32.len() == target_len {
        return false;
    }

    indirect_u32.values_mut().resize(target_len, 0);
    true
}

fn build_indirect_cpu_fallback(
    entries: &[(ChunkKey, ChunkMetaV2)],
    draw_records: &[u32],
    clip_from_world: Mat4,
    indirect_u32: &mut RawBufferVec<u32>,
    render_device: &RenderDevice,
    render_queue: &RenderQueue,
) {
    let values = indirect_u32.values_mut();
    fill_indirect_cpu_fallback(entries, draw_records, clip_from_world, values);
    if !values.is_empty() {
        indirect_u32.write_buffer(render_device, render_queue);
    }
}

fn fill_indirect_cpu_fallback(
    entries: &[(ChunkKey, ChunkMetaV2)],
    draw_records: &[u32],
    clip_from_world: Mat4,
    values: &mut Vec<u32>,
) {
    let draw_record_count = draw_records.len() / VOXEL_DRAW_RECORD_STRIDE_U32;
    values.resize(draw_record_count * VOXEL_DRAW_RECORD_STRIDE_U32, 0);
    for draw_record_index in 0..draw_record_count {
        let base = draw_record_index * VOXEL_DRAW_RECORD_STRIDE_U32;
        let chunk_meta_index = draw_records[base + VoxelDrawRecord::CHUNK_META_INDEX_OFFSET];
        let Some((_, entry)) = entries.get(chunk_meta_index as usize) else {
            continue;
        };
        let visible = aabb_visible(clip_from_world, entry.min.as_vec3(), entry.max.as_vec3());
        let draw = VoxelIndirectArgs {
            vertex_count: 6,
            instance_count: if visible { entry.opaque_len } else { 0 },
            first_vertex: (draw_record_index as u32) * 6,
            first_instance: draw_records[base + VoxelDrawRecord::FIRST_INSTANCE_OFFSET],
        };
        let words = draw.to_words();
        values[base + VoxelIndirectArgs::VERTEX_COUNT_OFFSET] =
            words[VoxelIndirectArgs::VERTEX_COUNT_OFFSET];
        values[base + VoxelIndirectArgs::INSTANCE_COUNT_OFFSET] =
            words[VoxelIndirectArgs::INSTANCE_COUNT_OFFSET];
        values[base + VoxelIndirectArgs::FIRST_VERTEX_OFFSET] =
            words[VoxelIndirectArgs::FIRST_VERTEX_OFFSET];
        values[base + VoxelIndirectArgs::FIRST_INSTANCE_OFFSET] =
            words[VoxelIndirectArgs::FIRST_INSTANCE_OFFSET];
    }
}

fn upload_quads_if_needed(
    quads: &VoxelQuadUploadQueue,
    buffers: &mut VoxelGpuBuffers,
    render_device: &RenderDevice,
    render_queue: &RenderQueue,
) {
    if quads.epoch == buffers.uploaded_epoch {
        return;
    }

    let target_len_u32 = (quads.quad_capacity as usize) * 2;

    if let Some(full) = &quads.full {
        buffers.quads_u32.clear();
        for &q in full.iter() {
            buffers.quads_u32.push(q as u32);
            buffers.quads_u32.push((q >> 32) as u32);
        }
        if buffers.quads_u32.len() < target_len_u32 {
            buffers.quads_u32.values_mut().resize(target_len_u32, 0);
        }
        buffers.quads_u32.write_buffer(render_device, render_queue);
        buffers.uploaded_epoch = quads.epoch;
        return;
    }

    if buffers.quads_u32.len() != target_len_u32 {
        buffers.quads_u32.values_mut().resize(target_len_u32, 0);
        if !buffers.quads_u32.values().is_empty() {
            buffers.quads_u32.write_buffer(render_device, render_queue);
        }
    }

    let mut requires_full_write = target_len_u32 > buffers.quads_u32.capacity();

    for update in quads.updates.iter() {
        let start = (update.offset as usize) * 2;
        let end = start + update.data.len() * 2;
        if end > buffers.quads_u32.values().len() {
            log::warn!(
                "voxel 增量上传越界：start={} end={} len={}",
                start,
                end,
                buffers.quads_u32.values().len()
            );
            requires_full_write = true;
            continue;
        }

        {
            let values = buffers.quads_u32.values_mut();
            for (i, &q) in update.data.iter().enumerate() {
                let dst = start + i * 2;
                values[dst] = q as u32;
                values[dst + 1] = (q >> 32) as u32;
            }
        }

        if requires_full_write {
            continue;
        }

        if let Err(err) = buffers
            .quads_u32
            .write_buffer_range(render_queue, start..end)
        {
            log::debug!("voxel 局部上传失败，回退全量写入：{err:?}");
            requires_full_write = true;
        }
    }

    if requires_full_write && !buffers.quads_u32.values().is_empty() {
        buffers.quads_u32.write_buffer(render_device, render_queue);
    }

    buffers.uploaded_epoch = quads.epoch;
}

fn upload_material_table_if_needed(
    material_table: Option<&ExtractedVoxelMaterialTable>,
    buffers: &mut VoxelGpuBuffers,
    render_device: &RenderDevice,
    render_queue: &RenderQueue,
) {
    let table = build_material_table_u32(material_table);
    if table == buffers.last_material_table {
        return;
    }

    buffers.last_material_table = table.clone();
    buffers.material_table_u32.clear();
    for value in table {
        buffers.material_table_u32.push(value);
    }
    buffers
        .material_table_u32
        .write_buffer(render_device, render_queue);
}

fn aabb_visible(clip_from_world: Mat4, min: Vec3, max: Vec3) -> bool {
    let corners = [
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(min.x, max.y, min.z),
        Vec3::new(max.x, max.y, min.z),
        Vec3::new(min.x, min.y, max.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(min.x, max.y, max.z),
        Vec3::new(max.x, max.y, max.z),
    ];

    let mut all_left = true;
    let mut all_right = true;
    let mut all_bottom = true;
    let mut all_top = true;
    let mut all_near = true;
    let mut all_far = true;

    for corner in corners {
        let p = clip_from_world * corner.extend(1.0);
        all_left &= p.x < -p.w;
        all_right &= p.x > p.w;
        all_bottom &= p.y < -p.w;
        all_top &= p.y > p.w;
        all_near &= p.z > p.w;
        all_far &= p.z < 0.0;
    }

    !(all_left || all_right || all_bottom || all_top || all_near || all_far)
}

#[expect(
    clippy::too_many_arguments,
    reason = "绑定组准备阶段需要多类 render 资源，拆分会引入额外状态同步"
)]
fn prepare_voxel_bind_groups(
    pipeline: Res<VoxelRenderPipeline>,
    buffers: Res<VoxelGpuBuffers>,
    view_uniforms: Res<ViewUniforms>,
    views: Query<(Entity, Option<&ViewDepthPyramid>), With<ExtractedView>>,
    runtime_packs: Option<Res<ExtractedTextureRuntimePacks>>,
    gpu_images: Res<bevy::render::render_asset::RenderAssets<GpuImage>>,
    render_device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    mut bind_groups: ResMut<VoxelRenderBindGroups>,
    mut view_states: ResMut<VoxelViewStates>,
) {
    let active_views: Vec<_> = views.iter().map(|(entity, _)| entity).collect();
    view_states.sync_active_views(active_views.iter().copied());

    let Some(view_binding) = view_uniforms.uniforms.binding() else {
        view_states.clear_bind_groups();
        return;
    };

    let Some(chunk_meta_binding) = buffers.chunk_meta_u32.binding() else {
        bind_groups.voxel = None;
        view_states.clear_bind_groups();
        return;
    };
    let Some(draw_records_binding) = buffers.draw_records_u32.binding() else {
        bind_groups.voxel = None;
        view_states.clear_bind_groups();
        return;
    };
    let Some(quad_binding) = buffers.quads_u32.binding() else {
        bind_groups.voxel = None;
        view_states.clear_bind_groups();
        return;
    };
    let Some(material_table_binding) = buffers.material_table_u32.binding() else {
        bind_groups.voxel = None;
        view_states.clear_bind_groups();
        return;
    };
    let Some(runtime_packs) = runtime_packs else {
        bind_groups.voxel = None;
        view_states.clear_bind_groups();
        return;
    };
    let Some(pack) = runtime_packs.0.packs.first() else {
        bind_groups.voxel = None;
        view_states.clear_bind_groups();
        return;
    };
    let Some(albedo_image) = gpu_images.get(&pack.albedo) else {
        bind_groups.voxel = None;
        view_states.clear_bind_groups();
        return;
    };
    let Some(normal_image) = gpu_images.get(&pack.normal) else {
        bind_groups.voxel = None;
        view_states.clear_bind_groups();
        return;
    };
    let Some(orm_image) = gpu_images.get(&pack.orm) else {
        bind_groups.voxel = None;
        view_states.clear_bind_groups();
        return;
    };
    let Some(emissive_image) = gpu_images.get(&pack.emissive) else {
        bind_groups.voxel = None;
        view_states.clear_bind_groups();
        return;
    };
    let Some(height_image) = gpu_images.get(&pack.height) else {
        bind_groups.voxel = None;
        view_states.clear_bind_groups();
        return;
    };

    bind_groups.voxel = Some(render_device.create_bind_group(
        "voxel_voxel_bind_group",
        &pipeline_cache.get_bind_group_layout(&pipeline.voxel_layout),
        &BindGroupEntries::sequential((
            chunk_meta_binding.clone(),
            draw_records_binding,
            quad_binding.clone(),
            material_table_binding.clone(),
            &albedo_image.texture_view,
            &normal_image.texture_view,
            &orm_image.texture_view,
            &emissive_image.texture_view,
            &height_image.texture_view,
            &pipeline.sampler,
        )),
    ));

    for (view_entity, view_depth_pyramid) in &views {
        let view_state = view_states.view_state_mut(view_entity);
        view_state.bind_groups.view = Some(render_device.create_bind_group(
            "voxel_view_bind_group",
            &pipeline_cache.get_bind_group_layout(&pipeline.view_layout),
            &BindGroupEntries::sequential((view_binding.clone(),)),
        ));

        let Some(culling_uniform_binding) = view_state.culling_uniform.binding() else {
            view_state.bind_groups.culling = None;
            continue;
        };
        let Some(indirect_binding) = view_state
            .indirect_u32(VoxelDrawLayer::Opaque)
            .and_then(RawBufferVec::binding)
        else {
            view_state.bind_groups.culling = None;
            continue;
        };
        let depth_pyramid_view = match view_depth_pyramid {
            Some(depth_pyramid) => &depth_pyramid.all_mips,
            None => &pipeline.culling_fallback_hzb,
        };
        view_state.bind_groups.culling = Some(render_device.create_bind_group(
            "voxel_culling_bind_group",
            &pipeline_cache.get_bind_group_layout(&pipeline.culling_layout),
            &BindGroupEntries::sequential((
                culling_uniform_binding.clone(),
                chunk_meta_binding.clone(),
                indirect_binding,
                depth_pyramid_view,
            )),
        ));
    }
}

fn build_material_table_u32(materials: Option<&ExtractedVoxelMaterialTable>) -> Vec<u32> {
    let mut table = vec![0; MAX_MATERIAL_KEYS * MATERIAL_TABLE_STRIDE_U32];
    let Some(materials) = materials else {
        return table;
    };

    for (index, material) in materials.0.entries.iter().enumerate() {
        if index >= MAX_MATERIAL_KEYS {
            break;
        }
        let base = index * MATERIAL_TABLE_STRIDE_U32;
        table[base] = material.alpha_mode;
        table[base + 1] = material.cutout_threshold.to_bits();
        let mut write_face = |offset: usize, face: crate::world::VoxelMaterialFaceLayers| {
            table[base + offset] = face.albedo as u32;
            table[base + offset + 1] = face.normal as u32;
            table[base + offset + 2] = face.orm as u32;
            table[base + offset + 3] = face.emissive as u32;
            table[base + offset + 4] = face.height as u32;
        };
        write_face(2, material.top);
        write_face(7, material.bottom);
        write_face(12, material.north);
        write_face(17, material.south);
        write_face(22, material.east);
        write_face(27, material.west);
    }
    table
}

#[derive(Default)]
struct VoxelOpaquePassNode;

impl ViewNode for VoxelOpaquePassNode {
    type ViewQuery = (
        Entity,
        &'static ViewTarget,
        &'static ViewDepthTexture,
        &'static ViewUniformOffset,
        &'static Msaa,
    );

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (view_entity, target, depth, view_uniform_offset, msaa): QueryItem<'w, '_, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        let pipeline = world.resource::<VoxelRenderPipeline>();
        let bind_groups = world.resource::<VoxelRenderBindGroups>();
        let view_states = world.resource::<VoxelViewStates>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let buffers = world.resource::<VoxelGpuBuffers>();
        let chunk_count = buffers.chunk_count();
        if chunk_count == 0 {
            return Ok(());
        }

        let Some(view_state) = view_states.view_state(view_entity) else {
            return Ok(());
        };

        let Some(view_bg) = &view_state.bind_groups.view else {
            return Ok(());
        };
        let Some(voxel_bg) = &bind_groups.voxel else {
            return Ok(());
        };

        let format = target.main_texture_format();
        let samples = msaa.samples();
        let Some(pipeline_id) = pipeline.pipelines.get(&(format, samples)).copied() else {
            return Ok(());
        };
        let Some(rp) = pipeline_cache.get_render_pipeline(pipeline_id) else {
            return Ok(());
        };

        if let (Some(culling_bg), Some(cp)) = (
            view_state.bind_groups.culling.as_ref(),
            pipeline_cache.get_compute_pipeline(pipeline.culling_pipeline),
        ) {
            let mut cull_pass =
                render_context
                    .command_encoder()
                    .begin_compute_pass(&ComputePassDescriptor {
                        label: Some("voxel_culling_pass"),
                        timestamp_writes: None,
                    });
            cull_pass.set_pipeline(cp);
            cull_pass.set_bind_group(0, culling_bg, &[]);
            let workgroups = chunk_count.div_ceil(CULL_WORKGROUP_SIZE);
            cull_pass.dispatch_workgroups(workgroups, 1, 1);
        }

        let mut pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("voxel_opaque_pass"),
            color_attachments: &[Some(target.get_color_attachment())],
            depth_stencil_attachment: Some(depth.get_attachment(StoreOp::Store)),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        pass.set_render_pipeline(rp);
        pass.set_bind_group(0, view_bg, &[view_uniform_offset.offset]);
        pass.set_bind_group(1, voxel_bg, &[]);

        let Some(indirect_buffer) = view_state
            .indirect_u32(VoxelDrawLayer::Opaque)
            .and_then(RawBufferVec::buffer)
        else {
            return Ok(());
        };

        if pipeline.supports_multi_draw_indirect {
            pass.multi_draw_indirect(indirect_buffer, 0, chunk_count);
        } else {
            for i in 0..chunk_count {
                pass.draw_indirect(indirect_buffer, (i as u64) * 16);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_table_defaults_to_zero() {
        let table = build_material_table_u32(None);
        assert_eq!(table.len(), MAX_MATERIAL_KEYS * MATERIAL_TABLE_STRIDE_U32);
        assert!(table.iter().all(|value| *value == 0));
    }

    #[test]
    fn material_table_packs_face_layers() {
        let materials = ExtractedVoxelMaterialTable(VoxelMaterialTable {
            entries: vec![crate::world::VoxelMaterialEntry {
                alpha_mode: 1,
                cutout_threshold: 0.5,
                top: crate::world::VoxelMaterialFaceLayers {
                    albedo: 11,
                    normal: 12,
                    orm: 13,
                    emissive: 14,
                    height: 15,
                },
                ..Default::default()
            }],
        });

        let table = build_material_table_u32(Some(&materials));
        let base = 0usize;
        assert_eq!(table[base], 1);
        assert_eq!(f32::from_bits(table[base + 1]), 0.5);
        assert_eq!(table[base + 2], 11);
        assert_eq!(table[base + 3], 12);
        assert_eq!(table[base + 4], 13);
        assert_eq!(table[base + 5], 14);
        assert_eq!(table[base + 6], 15);
    }

    #[test]
    fn chunk_meta_v2_layout() {
        let entries = vec![(
            ChunkKey::new(1, -2, 3),
            ChunkMetaV2 {
                origin: IVec3::new(32, -64, 96),
                min: IVec3::new(32, -64, 96),
                max: IVec3::new(64, -32, 128),
                opaque_offset: 17,
                opaque_len: 29,
                cutout_offset: 31,
                cutout_len: 37,
                flags: 0xA5A5_0001,
                reserved0: 0x1111_2222,
                reserved1: 0x3333_4444,
            },
        )];
        let packed = pack_chunk_meta_u32(&entries);
        assert_eq!(packed.len(), CHUNK_META_V2_STRIDE_U32);
        assert_eq!(packed[ChunkMetaV2::ORIGIN_X_OFFSET], 32i32 as u32);
        assert_eq!(packed[ChunkMetaV2::ORIGIN_Y_OFFSET], (-64i32) as u32);
        assert_eq!(packed[ChunkMetaV2::ORIGIN_Z_OFFSET], 96i32 as u32);
        assert_eq!(packed[ChunkMetaV2::OPAQUE_OFFSET_OFFSET], 17);
        assert_eq!(packed[ChunkMetaV2::MIN_X_OFFSET], 32i32 as u32);
        assert_eq!(packed[ChunkMetaV2::MIN_Y_OFFSET], (-64i32) as u32);
        assert_eq!(packed[ChunkMetaV2::MIN_Z_OFFSET], 96i32 as u32);
        assert_eq!(packed[ChunkMetaV2::OPAQUE_LEN_OFFSET], 29);
        assert_eq!(packed[ChunkMetaV2::MAX_X_OFFSET], 64i32 as u32);
        assert_eq!(packed[ChunkMetaV2::MAX_Y_OFFSET], (-32i32) as u32);
        assert_eq!(packed[ChunkMetaV2::MAX_Z_OFFSET], 128i32 as u32);
        assert_eq!(packed[ChunkMetaV2::CUTOUT_OFFSET_OFFSET], 31);
        assert_eq!(packed[ChunkMetaV2::CUTOUT_LEN_OFFSET], 37);
        assert_eq!(packed[ChunkMetaV2::FLAGS_OFFSET], 0xA5A5_0001);
        assert_eq!(packed[ChunkMetaV2::RESERVED0_OFFSET], 0x1111_2222);
        assert_eq!(packed[ChunkMetaV2::RESERVED1_OFFSET], 0x3333_4444);
    }

    #[test]
    fn voxel_draw_record_layout() {
        let record = VoxelDrawRecord {
            chunk_meta_index: 9,
            first_instance: 42,
            layer_mask: 0b01,
            reserved: 0xABCD_EF01,
        };
        let words = record.to_words();
        assert_eq!(VOXEL_DRAW_RECORD_STRIDE_U32, 4);
        assert_eq!(words.len(), VOXEL_DRAW_RECORD_STRIDE_U32);
        assert_eq!(words[VoxelDrawRecord::CHUNK_META_INDEX_OFFSET], 9);
        assert_eq!(words[VoxelDrawRecord::FIRST_INSTANCE_OFFSET], 42);
        assert_eq!(words[VoxelDrawRecord::LAYER_MASK_OFFSET], 0b01);
        assert_eq!(words[VoxelDrawRecord::RESERVED_OFFSET], 0xABCD_EF01);
    }

    #[test]
    fn draw_record_identity_is_explicit() {
        let key_a = ChunkKey::new(0, 0, 0);
        let key_b = ChunkKey::new(1, 0, 0);
        let bounds_a = ChunkBounds::from_key(key_a);
        let bounds_b = ChunkBounds::from_key(key_b);
        let range_a = ChunkDrawRange {
            opaque_offset: 3,
            opaque_len: 2,
            ..Default::default()
        };
        let range_b = ChunkDrawRange {
            opaque_offset: 17,
            opaque_len: 4,
            ..Default::default()
        };

        let entries = collect_chunk_meta_entries([
            (&key_b, &bounds_b, &range_b),
            (&key_a, &bounds_a, &range_a),
        ]);
        let draw_records = build_draw_records(&entries);

        assert_eq!(draw_records.len(), 2);
        assert_eq!(draw_records[0].chunk_meta_index, 0);
        assert_eq!(draw_records[0].first_instance, 3);
        assert_eq!(draw_records[1].chunk_meta_index, 1);
        assert_eq!(draw_records[1].first_instance, 17);
    }

    #[test]
    fn indirect_first_instance_matches_quad_offset() {
        let key = ChunkKey::new(0, 0, 0);
        let bounds = ChunkBounds::from_key(key);
        let range = ChunkDrawRange {
            opaque_offset: 21,
            opaque_len: 5,
            ..Default::default()
        };
        let entries = collect_chunk_meta_entries([(&key, &bounds, &range)]);
        let draw_records = pack_draw_records_u32(&build_draw_records(&entries));

        let mut indirect_words = Vec::new();
        fill_indirect_cpu_fallback(
            &entries,
            draw_records.as_slice(),
            Mat4::IDENTITY,
            &mut indirect_words,
        );

        assert_eq!(indirect_words.len(), VOXEL_DRAW_RECORD_STRIDE_U32);
        assert_eq!(
            indirect_words[VoxelIndirectArgs::FIRST_INSTANCE_OFFSET],
            range.opaque_offset
        );
    }

    #[test]
    fn draw_record_identity_survives_resident_reorder() {
        let key_a = ChunkKey::new(-1, 0, 0);
        let key_b = ChunkKey::new(0, 0, 0);
        let key_c = ChunkKey::new(1, 0, 0);
        let bounds_a = ChunkBounds::from_key(key_a);
        let bounds_b = ChunkBounds::from_key(key_b);
        let bounds_c = ChunkBounds::from_key(key_c);
        let range_a = ChunkDrawRange {
            opaque_offset: 1,
            opaque_len: 1,
            ..Default::default()
        };
        let range_b = ChunkDrawRange {
            opaque_offset: 8,
            opaque_len: 1,
            ..Default::default()
        };
        let range_c = ChunkDrawRange {
            opaque_offset: 13,
            opaque_len: 1,
            ..Default::default()
        };

        let entries_abc = collect_chunk_meta_entries([
            (&key_a, &bounds_a, &range_a),
            (&key_b, &bounds_b, &range_b),
            (&key_c, &bounds_c, &range_c),
        ]);
        let entries_cba = collect_chunk_meta_entries([
            (&key_c, &bounds_c, &range_c),
            (&key_b, &bounds_b, &range_b),
            (&key_a, &bounds_a, &range_a),
        ]);

        let records_abc = build_draw_records(&entries_abc);
        let records_cba = build_draw_records(&entries_cba);

        assert_eq!(records_abc, records_cba);
        assert_eq!(records_abc[0].chunk_meta_index, 0);
        assert_eq!(records_abc[1].chunk_meta_index, 1);
        assert_eq!(records_abc[2].chunk_meta_index, 2);
    }

    #[test]
    fn visible_chunk_record_layout() {
        let record = VisibleChunkRecord {
            chunk_index: 9,
            layer_mask: 0b11,
            draw_record_base: 24,
            reserved: 0xDEAD_BEEF,
        };
        let words = record.to_words();
        assert_eq!(VISIBLE_CHUNK_RECORD_STRIDE_U32, 4);
        assert_eq!(words.len(), VISIBLE_CHUNK_RECORD_STRIDE_U32);
        assert_eq!(words[VisibleChunkRecord::CHUNK_INDEX_OFFSET], 9);
        assert_eq!(words[VisibleChunkRecord::LAYER_MASK_OFFSET], 0b11);
        assert_eq!(words[VisibleChunkRecord::DRAW_RECORD_BASE_OFFSET], 24);
        assert_eq!(words[VisibleChunkRecord::RESERVED_OFFSET], 0xDEAD_BEEF);

        assert_eq!(PER_VIEW_COUNTERS_STRIDE_U32, 4);
        assert_eq!(per_view_counters_layout::VISIBLE_CHUNK_COUNT_OFFSET, 0);
        assert_eq!(per_view_counters_layout::FLAGS_OFFSET, 1);
        assert_eq!(per_view_counters_layout::RESERVED0_OFFSET, 2);
        assert_eq!(per_view_counters_layout::RESERVED1_OFFSET, 3);
    }

    #[test]
    fn collect_chunk_meta_includes_cutout_only_chunks() {
        let opaque_key = ChunkKey::new(0, 0, 0);
        let cutout_only_key = ChunkKey::new(1, 0, 0);
        let empty_key = ChunkKey::new(2, 0, 0);
        let transparent_only_key = ChunkKey::new(3, 0, 0);

        let opaque_bounds = ChunkBounds::from_key(opaque_key);
        let cutout_only_bounds = ChunkBounds::from_key(cutout_only_key);
        let empty_bounds = ChunkBounds::from_key(empty_key);
        let transparent_only_bounds = ChunkBounds::from_key(transparent_only_key);

        let opaque_range = ChunkDrawRange {
            opaque_offset: 4,
            opaque_len: 2,
            ..Default::default()
        };
        let cutout_only_range = ChunkDrawRange {
            cutout_offset: 8,
            cutout_len: 5,
            ..Default::default()
        };
        let empty_range = ChunkDrawRange::default();
        let transparent_only_range = ChunkDrawRange {
            transparent_offset: 13,
            transparent_len: 7,
            ..Default::default()
        };

        let entries = collect_chunk_meta_entries([
            (&empty_key, &empty_bounds, &empty_range),
            (&cutout_only_key, &cutout_only_bounds, &cutout_only_range),
            (
                &transparent_only_key,
                &transparent_only_bounds,
                &transparent_only_range,
            ),
            (&opaque_key, &opaque_bounds, &opaque_range),
        ]);

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].0, opaque_key);
        assert_eq!(entries[0].1.opaque_offset, 4);
        assert_eq!(entries[0].1.opaque_len, 2);
        assert_eq!(entries[0].1.cutout_offset, 0);
        assert_eq!(entries[0].1.cutout_len, 0);

        assert_eq!(entries[1].0, cutout_only_key);
        assert_eq!(entries[1].1.opaque_len, 0);
        assert_eq!(entries[1].1.cutout_offset, 8);
        assert_eq!(entries[1].1.cutout_len, 5);
    }

    #[test]
    fn chunk_meta_sorting_is_stable_by_chunk_key() {
        let key_a = ChunkKey::new(-2, 1, 7);
        let key_b = ChunkKey::new(0, 0, 0);
        let key_c = ChunkKey::new(0, 1, -1);

        let bounds_a = ChunkBounds::from_key(key_a);
        let bounds_b = ChunkBounds::from_key(key_b);
        let bounds_c = ChunkBounds::from_key(key_c);

        let range_a = ChunkDrawRange {
            cutout_offset: 30,
            cutout_len: 1,
            ..Default::default()
        };
        let range_b = ChunkDrawRange {
            opaque_offset: 10,
            opaque_len: 3,
            ..Default::default()
        };
        let range_c = ChunkDrawRange {
            opaque_offset: 20,
            opaque_len: 4,
            cutout_offset: 24,
            cutout_len: 2,
            ..Default::default()
        };

        let entries = collect_chunk_meta_entries([
            (&key_c, &bounds_c, &range_c),
            (&key_b, &bounds_b, &range_b),
            (&key_a, &bounds_a, &range_a),
        ]);

        let keys: Vec<_> = entries.iter().map(|(key, _)| *key).collect();
        assert_eq!(keys, vec![key_a, key_b, key_c]);
        assert_eq!(entries[0].1.origin, key_a.min_world_voxel());
        assert_eq!(entries[0].1.cutout_offset, 30);
        assert_eq!(entries[1].1.origin, key_b.min_world_voxel());
        assert_eq!(entries[1].1.opaque_offset, 10);
        assert_eq!(entries[2].1.origin, key_c.min_world_voxel());
        assert_eq!(entries[2].1.opaque_offset, 20);
        assert_eq!(entries[2].1.cutout_offset, 24);
    }

    #[test]
    fn per_view_state() {
        let view_a = Entity::from_bits(1);
        let view_b = Entity::from_bits(2);
        let view_c = Entity::from_bits(3);
        let mut view_states = VoxelViewStates::default();

        view_states.sync_active_views([view_a, view_b]);
        assert_eq!(view_states.by_entity.len(), 2);
        assert!(view_states.view_state(view_a).is_some());
        assert!(view_states.view_state(view_b).is_some());

        let preserved_state = view_states.view_state_mut(view_b);
        assert!(resize_indirect_buffer(
            preserved_state.indirect_u32_mut(VoxelDrawLayer::Opaque),
            2,
        ));
        preserved_state
            .indirect_u32_mut(VoxelDrawLayer::Opaque)
            .values_mut()
            .copy_from_slice(&[6, 7, 8, 9, 10, 11, 12, 13]);

        view_states.sync_active_views([view_b, view_c]);

        assert_eq!(view_states.by_entity.len(), 2);
        assert!(view_states.view_state(view_a).is_none());

        let preserved_state = view_states.view_state(view_b).expect("view_b kept");
        assert_eq!(
            preserved_state
                .indirect_u32(VoxelDrawLayer::Opaque)
                .expect("opaque buffer exists")
                .values(),
            &[6, 7, 8, 9, 10, 11, 12, 13]
        );

        let fresh_state = view_states.view_state(view_c).expect("view_c created");
        assert!(fresh_state.bind_groups.view.is_none());
        assert!(fresh_state.bind_groups.culling.is_none());
        assert!(fresh_state.previous_depth_texture.is_none());
        assert!(fresh_state.previous_hzb_texture.is_none());
        assert!(fresh_state.visible_chunk_list.is_none());
        assert!(fresh_state.visible_chunk_counter.is_none());
        assert!(fresh_state.indirect_u32(VoxelDrawLayer::Opaque).is_some());
        assert!(fresh_state.indirect_u32(VoxelDrawLayer::Cutout).is_none());
    }

    #[test]
    fn resize_reallocates_only_target_view() {
        let view_a = Entity::from_bits(11);
        let view_b = Entity::from_bits(22);
        let mut view_states = VoxelViewStates::default();
        view_states.sync_active_views([view_a, view_b]);

        {
            let state_a = view_states.view_state_mut(view_a);
            assert!(resize_indirect_buffer(
                state_a.indirect_u32_mut(VoxelDrawLayer::Opaque),
                1,
            ));
        }

        {
            let state_b = view_states.view_state_mut(view_b);
            assert!(resize_indirect_buffer(
                state_b.indirect_u32_mut(VoxelDrawLayer::Opaque),
                2,
            ));
            state_b
                .indirect_u32_mut(VoxelDrawLayer::Opaque)
                .values_mut()
                .copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
        }

        let previous_view_b = view_states
            .view_state(view_b)
            .expect("view_b exists")
            .indirect_u32(VoxelDrawLayer::Opaque)
            .expect("opaque buffer exists")
            .values()
            .to_vec();

        let reallocated = resize_indirect_buffer(
            view_states
                .view_state_mut(view_a)
                .indirect_u32_mut(VoxelDrawLayer::Opaque),
            5,
        );

        assert!(reallocated);
        assert_eq!(
            view_states
                .view_state(view_a)
                .expect("view_a exists")
                .indirect_u32(VoxelDrawLayer::Opaque)
                .expect("opaque buffer exists")
                .len(),
            20
        );
        assert_eq!(
            view_states
                .view_state(view_b)
                .expect("view_b exists")
                .indirect_u32(VoxelDrawLayer::Opaque)
                .expect("opaque buffer exists")
                .values(),
            previous_view_b.as_slice()
        );
    }
}
