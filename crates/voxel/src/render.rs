//! 体素渲染：Packed-Quad + vertex pulling。
//!
//! 当前实现重点：
//! - CPU 侧增量上传 quad 数据（避免每帧全量重传）
//! - GPU frustum culling（compute 生成 indirect 命令）
//! - MDI/间接绘制提交（单次或少量 API 调用绘制所有 chunk）

use std::collections::HashMap;

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
const CHUNK_META_STRIDE_U32: usize = 12;
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
struct ChunkMetaCpu {
    origin: IVec3,
    min: IVec3,
    max: IVec3,
    opaque_offset: u32,
    opaque_len: u32,
}

#[derive(Resource)]
struct VoxelGpuBuffers {
    /// 以 `u32` 序列上传：每个 quad 占 2 个 u32（low/high）。
    quads_u32: RawBufferVec<u32>,
    /// 运行时材质表：按 material_id 索引，每项固定 32 个 u32。
    material_table_u32: RawBufferVec<u32>,
    /// chunk 元数据表，固定 stride=12*u32（见 `voxel_quads.wgsl` / `voxel_cull.wgsl`）。
    chunk_meta_u32: RawBufferVec<u32>,
    /// DrawIndirectArgs 序列（每 chunk 一条，4*u32）。
    indirect_u32: RawBufferVec<u32>,
    culling_uniform: UniformBuffer<VoxelCullingUniform>,
    uploaded_epoch: u64,
    last_material_table: Vec<u32>,
    chunk_count: u32,
}

impl Default for VoxelGpuBuffers {
    fn default() -> Self {
        let mut quads_u32 = RawBufferVec::new(BufferUsages::STORAGE);
        quads_u32.set_label(Some("voxel_quads_u32"));
        let mut material_table_u32 = RawBufferVec::new(BufferUsages::STORAGE);
        material_table_u32.set_label(Some("voxel_material_table_u32"));
        let mut chunk_meta_u32 = RawBufferVec::new(BufferUsages::STORAGE);
        chunk_meta_u32.set_label(Some("voxel_chunk_meta_u32"));
        let mut indirect_u32 = RawBufferVec::new(BufferUsages::INDIRECT | BufferUsages::STORAGE);
        indirect_u32.set_label(Some("voxel_indirect_u32"));
        Self {
            quads_u32,
            material_table_u32,
            chunk_meta_u32,
            indirect_u32,
            culling_uniform: UniformBuffer::default(),
            uploaded_epoch: 0,
            last_material_table: Vec::new(),
            chunk_count: 0,
        }
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
    view: Option<BindGroup>,
    voxel: Option<BindGroup>,
    culling: Option<BindGroup>,
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
                // chunk 元数据表（stride=12*u32）。
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
        &ExtractedView,
        &ViewTarget,
        &Msaa,
        Option<&ViewDepthPyramid>,
    )>,
    quads: Res<VoxelQuadUploadQueue>,
    material_table: Option<Res<ExtractedVoxelMaterialTable>>,
    chunks: Query<(&ChunkKey, &ChunkBounds, &ChunkDrawRange)>,
    mut buffers: ResMut<VoxelGpuBuffers>,
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

    let Ok((view, view_target, msaa, view_depth_pyramid)) = views.single() else {
        return;
    };
    let clip_from_world = view
        .clip_from_world
        .unwrap_or_else(|| view.clip_from_view * view.world_from_view.to_matrix().inverse());

    let mut entries = collect_chunk_meta_entries(&chunks);
    entries.sort_by_key(|(key, _)| *key);
    buffers.chunk_count = entries.len() as u32;

    sync_chunk_meta_buffer(&entries, &mut buffers, &render_device, &render_queue);
    ensure_indirect_capacity(
        buffers.chunk_count,
        &mut buffers.indirect_u32,
        &render_device,
        &render_queue,
    );

    let chunk_count = buffers.chunk_count;
    let (hzb_mip_count, hzb_enabled, hzb_size) = match view_depth_pyramid {
        Some(depth_pyramid) if depth_pyramid.mip_count > 0 => {
            let viewport = view.viewport.zw();
            let mip0 = UVec2::new(viewport.x.div_ceil(2), viewport.y.div_ceil(2)).max(UVec2::ONE);
            (depth_pyramid.mip_count, 1, mip0)
        }
        _ => (0, 0, UVec2::ONE),
    };
    buffers.culling_uniform.set(VoxelCullingUniform {
        clip_from_world,
        chunk_count,
        hzb_mip_count,
        hzb_enabled,
        _pad0: 0,
        hzb_size,
        _pad1: UVec2::ZERO,
    });
    if chunk_count > 0 {
        buffers
            .culling_uniform
            .write_buffer(&render_device, &render_queue);
    }

    // pipeline 热身阶段 fallback：compute pipeline 未就绪时用 CPU 先填 indirect。
    if pipeline_cache
        .get_compute_pipeline(pipeline.culling_pipeline)
        .is_none()
    {
        build_indirect_cpu_fallback(
            &entries,
            clip_from_world,
            &mut buffers.indirect_u32,
            &render_device,
            &render_queue,
        );
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

fn collect_chunk_meta_entries(
    chunks: &Query<(&ChunkKey, &ChunkBounds, &ChunkDrawRange)>,
) -> Vec<(ChunkKey, ChunkMetaCpu)> {
    let mut out = Vec::new();
    for (key, bounds, range) in chunks.iter() {
        if range.opaque_len == 0 {
            continue;
        }
        out.push((
            *key,
            ChunkMetaCpu {
                origin: key.min_world_voxel(),
                min: bounds.min,
                max: bounds.max,
                opaque_offset: range.opaque_offset,
                opaque_len: range.opaque_len,
            },
        ));
    }
    out
}

fn pack_chunk_meta_u32(entries: &[(ChunkKey, ChunkMetaCpu)]) -> Vec<u32> {
    let mut packed = Vec::with_capacity(entries.len() * CHUNK_META_STRIDE_U32);
    for (_, entry) in entries {
        packed.extend_from_slice(&[
            entry.origin.x as u32,
            entry.origin.y as u32,
            entry.origin.z as u32,
            entry.opaque_offset,
            entry.min.x as u32,
            entry.min.y as u32,
            entry.min.z as u32,
            entry.opaque_len,
            entry.max.x as u32,
            entry.max.y as u32,
            entry.max.z as u32,
            0,
        ]);
    }
    packed
}

fn sync_chunk_meta_buffer(
    entries: &[(ChunkKey, ChunkMetaCpu)],
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

fn ensure_indirect_capacity(
    chunk_count: u32,
    indirect_u32: &mut RawBufferVec<u32>,
    render_device: &RenderDevice,
    render_queue: &RenderQueue,
) {
    let target_len = (chunk_count as usize) * 4;
    if indirect_u32.len() == target_len {
        return;
    }

    indirect_u32.values_mut().resize(target_len, 0);
    if target_len > 0 {
        // 只做容量保障；命令内容由 compute pass 覆写。
        indirect_u32.write_buffer(render_device, render_queue);
    }
}

fn build_indirect_cpu_fallback(
    entries: &[(ChunkKey, ChunkMetaCpu)],
    clip_from_world: Mat4,
    indirect_u32: &mut RawBufferVec<u32>,
    render_device: &RenderDevice,
    render_queue: &RenderQueue,
) {
    let values = indirect_u32.values_mut();
    values.resize(entries.len() * 4, 0);
    for (i, (_, entry)) in entries.iter().enumerate() {
        let visible = aabb_visible(clip_from_world, entry.min.as_vec3(), entry.max.as_vec3());
        let base = i * 4;
        values[base] = 6;
        values[base + 1] = if visible { entry.opaque_len } else { 0 };
        values[base + 2] = (i as u32) * 6;
        values[base + 3] = 0;
    }
    if !values.is_empty() {
        indirect_u32.write_buffer(render_device, render_queue);
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
    views: Query<Option<&ViewDepthPyramid>, With<ExtractedView>>,
    runtime_packs: Option<Res<ExtractedTextureRuntimePacks>>,
    gpu_images: Res<bevy::render::render_asset::RenderAssets<GpuImage>>,
    render_device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    mut bind_groups: ResMut<VoxelRenderBindGroups>,
) {
    let Some(view_binding) = view_uniforms.uniforms.binding() else {
        bind_groups.view = None;
        return;
    };
    bind_groups.view = Some(render_device.create_bind_group(
        "voxel_view_bind_group",
        &pipeline_cache.get_bind_group_layout(&pipeline.view_layout),
        &BindGroupEntries::sequential((view_binding.clone(),)),
    ));

    let Some(chunk_meta_binding) = buffers.chunk_meta_u32.binding() else {
        bind_groups.voxel = None;
        bind_groups.culling = None;
        return;
    };
    let Some(quad_binding) = buffers.quads_u32.binding() else {
        bind_groups.voxel = None;
        bind_groups.culling = None;
        return;
    };
    let Some(material_table_binding) = buffers.material_table_u32.binding() else {
        bind_groups.voxel = None;
        bind_groups.culling = None;
        return;
    };
    let Some(runtime_packs) = runtime_packs else {
        bind_groups.voxel = None;
        bind_groups.culling = None;
        return;
    };
    let Some(pack) = runtime_packs.0.packs.first() else {
        bind_groups.voxel = None;
        bind_groups.culling = None;
        return;
    };
    let Some(albedo_image) = gpu_images.get(&pack.albedo) else {
        bind_groups.voxel = None;
        bind_groups.culling = None;
        return;
    };
    let Some(normal_image) = gpu_images.get(&pack.normal) else {
        bind_groups.voxel = None;
        bind_groups.culling = None;
        return;
    };
    let Some(orm_image) = gpu_images.get(&pack.orm) else {
        bind_groups.voxel = None;
        bind_groups.culling = None;
        return;
    };
    let Some(emissive_image) = gpu_images.get(&pack.emissive) else {
        bind_groups.voxel = None;
        bind_groups.culling = None;
        return;
    };
    let Some(height_image) = gpu_images.get(&pack.height) else {
        bind_groups.voxel = None;
        bind_groups.culling = None;
        return;
    };

    bind_groups.voxel = Some(render_device.create_bind_group(
        "voxel_voxel_bind_group",
        &pipeline_cache.get_bind_group_layout(&pipeline.voxel_layout),
        &BindGroupEntries::sequential((
            chunk_meta_binding.clone(),
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

    let Some(culling_uniform_binding) = buffers.culling_uniform.binding() else {
        bind_groups.culling = None;
        return;
    };
    let Some(indirect_binding) = buffers.indirect_u32.binding() else {
        bind_groups.culling = None;
        return;
    };
    let depth_pyramid_view = match views.single() {
        Ok(Some(depth_pyramid)) => &depth_pyramid.all_mips,
        Ok(None) | Err(_) => &pipeline.culling_fallback_hzb,
    };
    bind_groups.culling = Some(render_device.create_bind_group(
        "voxel_culling_bind_group",
        &pipeline_cache.get_bind_group_layout(&pipeline.culling_layout),
        &BindGroupEntries::sequential((
            culling_uniform_binding.clone(),
            chunk_meta_binding.clone(),
            indirect_binding.clone(),
            depth_pyramid_view,
        )),
    ));
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
        &'static ViewTarget,
        &'static ViewDepthTexture,
        &'static ViewUniformOffset,
        &'static Msaa,
    );

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (target, depth, view_uniform_offset, msaa): QueryItem<'w, '_, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        let pipeline = world.resource::<VoxelRenderPipeline>();
        let bind_groups = world.resource::<VoxelRenderBindGroups>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let buffers = world.resource::<VoxelGpuBuffers>();
        if buffers.chunk_count == 0 {
            return Ok(());
        }

        let Some(view_bg) = &bind_groups.view else {
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
            bind_groups.culling.as_ref(),
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
            let workgroups = buffers.chunk_count.div_ceil(CULL_WORKGROUP_SIZE);
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

        let Some(indirect_buffer) = buffers.indirect_u32.buffer() else {
            return Ok(());
        };

        if pipeline.supports_multi_draw_indirect {
            pass.multi_draw_indirect(indirect_buffer, 0, buffers.chunk_count);
        } else {
            for i in 0..buffers.chunk_count {
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
    fn chunk_meta_packing_keeps_expected_stride_and_fields() {
        let entries = vec![(
            ChunkKey::new(1, -2, 3),
            ChunkMetaCpu {
                origin: IVec3::new(32, -64, 96),
                min: IVec3::new(32, -64, 96),
                max: IVec3::new(64, -32, 128),
                opaque_offset: 17,
                opaque_len: 29,
            },
        )];
        let packed = pack_chunk_meta_u32(&entries);
        assert_eq!(packed.len(), CHUNK_META_STRIDE_U32);
        assert_eq!(packed[0], 32i32 as u32);
        assert_eq!(packed[1], (-64i32) as u32);
        assert_eq!(packed[2], 96i32 as u32);
        assert_eq!(packed[3], 17);
        assert_eq!(packed[4], 32i32 as u32);
        assert_eq!(packed[5], (-64i32) as u32);
        assert_eq!(packed[6], 96i32 as u32);
        assert_eq!(packed[7], 29);
        assert_eq!(packed[8], 64i32 as u32);
        assert_eq!(packed[9], (-32i32) as u32);
        assert_eq!(packed[10], 128i32 as u32);
        assert_eq!(packed[11], 0);
    }
}
