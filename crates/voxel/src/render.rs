//! 体素渲染（MVP）：Packed-Quad + vertex pulling（无 Mesh 资产路径）。
//!
//! 说明：
//! - 本阶段先打通“能看到地形”的最小链路：按 chunk 逐个 draw（CPU loop），shader 侧从 quad storage buffer
//!   拉取顶点（vertex pulling）。
//! - `docs/voxel/rendering.md` 规定最终形态是 MDI + GPU culling + HZB；这里先保持数据形态一致，
//!   后续再硬切换到 IndirectDrawBuffer / VisibleChunkList 等完整管线。

use bevy::{
    core_pipeline::core_3d::{graph::Core3d, graph::Node3d, CORE_3D_DEPTH_FORMAT},
    ecs::query::QueryItem,
    prelude::*,
    render::{
        extract_component::ExtractComponentPlugin,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_graph::{
            NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner,
        },
        render_resource::{
            binding_types::{sampler, storage_buffer_read_only, texture_2d_array, uniform_buffer},
            *,
        },
        renderer::{RenderContext, RenderDevice, RenderQueue},
        texture::GpuImage,
        view::{Msaa, ViewDepthTexture, ViewTarget, ViewUniform, ViewUniformOffset, ViewUniforms},
        Render, RenderApp, RenderStartup, RenderSystems,
    },
};

use cruft_proc_textures::BlockTextureArray;

use crate::coords::ChunkKey;
use crate::world::{ChunkDrawRange, VoxelQuadStore};

const SHADER_ASSET_PATH: &str = "shaders/voxel_quads.wgsl";

#[derive(Resource, Clone)]
struct ExtractedBlockTextureArray(pub Handle<Image>);

impl ExtractResource for ExtractedBlockTextureArray {
    type Source = BlockTextureArray;

    fn extract_resource(source: &Self::Source) -> Self {
        Self(source.0.clone())
    }
}

#[derive(Resource)]
struct VoxelGpuBuffers {
    /// 以 `u32` 序列上传：每个 quad 占 2 个 u32（low/high）。
    quads_u32: RawBufferVec<u32>,
    chunk_uniforms: DynamicUniformBuffer<ChunkUniform>,
}

impl Default for VoxelGpuBuffers {
    fn default() -> Self {
        let mut quads_u32 = RawBufferVec::new(BufferUsages::STORAGE);
        quads_u32.set_label(Some("voxel_quads_u32"));
        Self {
            quads_u32,
            chunk_uniforms: DynamicUniformBuffer::default(),
        }
    }
}

#[derive(Clone, Copy, ShaderType)]
struct ChunkUniform {
    origin: Vec3,
    quad_base: u32,
}

#[derive(Resource, Default)]
struct VoxelChunkDrawList {
    entries: Vec<VoxelChunkDraw>,
}

#[derive(Debug, Clone, Copy)]
struct VoxelChunkDraw {
    chunk_uniform_offset: u32,
    instance_count: u32,
}

#[derive(Resource)]
struct VoxelRenderPipeline {
    view_layout: BindGroupLayoutDescriptor,
    voxel_layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    shader: Handle<Shader>,
    pipelines: std::collections::HashMap<(TextureFormat, u32), CachedRenderPipelineId>,
}

#[derive(Resource, Default)]
struct VoxelRenderBindGroups {
    view: Option<BindGroup>,
    voxel: Option<BindGroup>,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct VoxelOpaquePassLabel;

pub struct VoxelRenderPlugin;

impl Plugin for VoxelRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ExtractResourcePlugin::<VoxelQuadStore>::default(),
            ExtractResourcePlugin::<ExtractedBlockTextureArray>::default(),
            ExtractComponentPlugin::<ChunkKey>::default(),
            ExtractComponentPlugin::<ChunkDrawRange>::default(),
        ));

        let render_app = app.sub_app_mut(RenderApp);
        render_app
            .init_resource::<VoxelGpuBuffers>()
            .init_resource::<VoxelChunkDrawList>()
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

fn init_voxel_render_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    asset_server: Res<AssetServer>,
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
                uniform_buffer::<ChunkUniform>(true),
                // quad buffer：array<u32>，按 2 u32 / quad。
                storage_buffer_read_only::<u32>(false),
                texture_2d_array(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
            ),
        ),
    );

    let sampler = render_device.create_sampler(&SamplerDescriptor {
        mag_filter: FilterMode::Nearest,
        min_filter: FilterMode::Nearest,
        mipmap_filter: FilterMode::Nearest,
        ..default()
    });

    let shader = asset_server.load(SHADER_ASSET_PATH);

    commands.insert_resource(VoxelRenderPipeline {
        view_layout,
        voxel_layout,
        sampler,
        shader,
        pipelines: std::collections::HashMap::new(),
    });
}

fn prepare_voxel_gpu_buffers(
    views: Query<(&ViewTarget, &Msaa)>,
    quads: Res<VoxelQuadStore>,
    chunks: Query<(&ChunkKey, &ChunkDrawRange)>,
    mut buffers: ResMut<VoxelGpuBuffers>,
    mut draw_list: ResMut<VoxelChunkDrawList>,
    mut pipeline: ResMut<VoxelRenderPipeline>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    pipeline_cache: Res<PipelineCache>,
) {
    buffers.quads_u32.clear();
    for &q in quads.data.iter() {
        buffers.quads_u32.push(q as u32);
        buffers.quads_u32.push((q >> 32) as u32);
    }
    buffers
        .quads_u32
        .write_buffer(&render_device, &render_queue);

    buffers.chunk_uniforms.clear();
    draw_list.entries.clear();
    for (key, range) in &chunks {
        if range.opaque_len == 0 {
            continue;
        }
        let origin = key.min_world_voxel().as_vec3();
        let offset = buffers.chunk_uniforms.push(&ChunkUniform {
            origin,
            quad_base: range.opaque_offset,
        });
        draw_list.entries.push(VoxelChunkDraw {
            chunk_uniform_offset: offset,
            instance_count: range.opaque_len,
        });
    }
    buffers
        .chunk_uniforms
        .write_buffer(&render_device, &render_queue);

    // pipeline：按 view format + msaa samples 做最小特化。
    let Ok((view_target, msaa)) = views.single() else {
        return;
    };
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
                cull_mode: None, // MVP：先禁用背面剔除，避免 face winding 问题
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

fn prepare_voxel_bind_groups(
    pipeline: Res<VoxelRenderPipeline>,
    buffers: Res<VoxelGpuBuffers>,
    view_uniforms: Res<ViewUniforms>,
    texture: Option<Res<ExtractedBlockTextureArray>>,
    gpu_images: Res<bevy::render::render_asset::RenderAssets<GpuImage>>,
    render_device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    mut bind_groups: ResMut<VoxelRenderBindGroups>,
) {
    let Some(view_binding) = view_uniforms.uniforms.binding() else {
        return;
    };
    let Some(chunk_binding) = buffers.chunk_uniforms.binding() else {
        return;
    };
    let Some(quad_binding) = buffers.quads_u32.binding() else {
        return;
    };

    let Some(texture) = texture else {
        return;
    };
    let Some(gpu_image) = gpu_images.get(&texture.0) else {
        return;
    };

    bind_groups.view = Some(render_device.create_bind_group(
        "voxel_view_bind_group",
        &pipeline_cache.get_bind_group_layout(&pipeline.view_layout),
        &BindGroupEntries::sequential((view_binding.clone(),)),
    ));

    bind_groups.voxel = Some(render_device.create_bind_group(
        "voxel_voxel_bind_group",
        &pipeline_cache.get_bind_group_layout(&pipeline.voxel_layout),
        &BindGroupEntries::sequential((
            chunk_binding.clone(),
            quad_binding.clone(),
            &gpu_image.texture_view,
            &pipeline.sampler,
        )),
    ));
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
        let draw_list = world.resource::<VoxelChunkDrawList>();
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

        let mut pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("voxel_opaque_pass"),
            color_attachments: &[Some(target.get_color_attachment())],
            depth_stencil_attachment: Some(depth.get_attachment(StoreOp::Store)),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        pass.set_render_pipeline(rp);
        pass.set_bind_group(0, view_bg, &[view_uniform_offset.offset]);

        for entry in &draw_list.entries {
            pass.set_bind_group(1, voxel_bg, &[entry.chunk_uniform_offset]);
            pass.draw(0..6, 0..entry.instance_count);
        }

        Ok(())
    }
}
