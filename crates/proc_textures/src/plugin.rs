//! 程序化贴图（GPU compute / texture array）生成服务。
//!
//! 设计目标：
//! - 纯服务：不默认 spawn 预览实体（相机/灯光/方块）
//! - RenderApp 内 dispatch compute，一次性生成 texture array
//! - 主世界通过 channel 收到 ready 信号，用于 BootLoading 聚合

use std::borrow::Cow;
use std::sync::{mpsc, Mutex};
use std::time::Instant;

use bevy::{
    asset::RenderAssetUsages,
    asset::{io::Reader, LoadContext},
    image::{TextureFormatPixelInfo, Volume},
    prelude::*,
    reflect::TypePath,
    render::{
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_asset::RenderAssets,
        render_graph::{self, RenderGraph, RenderLabel},
        render_resource::{
            binding_types::{texture_storage_2d_array, uniform_buffer},
            *,
        },
        renderer::{RenderContext, RenderDevice, RenderQueue},
        texture::GpuImage,
        Render, RenderApp, RenderStartup, RenderSystems,
    },
    shader::PipelineCacheError,
    shader::ShaderRef,
};

use cruft_game_flow::{BootReadiness, BootReady};

use serde::Deserialize;

const SHADER_ASSET_PATH: &str = "shaders/procedural_texture.wgsl";
const WORKGROUP_SIZE: u32 = 8;
const CANONICAL_TEXTURE_SIZE: u32 = 64;
const MATERIAL_SHADER_ASSET_PATH: &str = "shaders/procedural_array_material.wgsl";
const MAX_LAYERS: usize = 256;
const TEXTURE_DATA_PATH: &str = "texture_data/blocks.texture.json";

/// 程序化纹理 array 的句柄（未来供 voxel/material 使用）。
#[derive(Resource, Clone)]
pub struct BlockTextureArray(pub Handle<Image>);

/// 程序化贴图是否已准备完成（BootLoading 聚合用）。
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct ProcTexturesReady(pub bool);

#[derive(Resource)]
struct ProcTexturesReadyRx(Mutex<mpsc::Receiver<()>>);

#[derive(Resource, Clone)]
struct ProcTexturesReadyTx(mpsc::Sender<()>);

pub struct ProcTexturesPlugin;

impl Plugin for ProcTexturesPlugin {
    fn build(&self, app: &mut App) {
        let (tx, rx) = mpsc::channel::<()>();

        app.init_resource::<ProcTexturesReady>()
            .insert_resource(ProcTexturesReadyRx(Mutex::new(rx)))
            .add_plugins(MaterialPlugin::<ProceduralArrayMaterial>::default())
            .add_plugins((
                ExtractResourcePlugin::<ProceduralTextureImages>::default(),
                ExtractResourcePlugin::<ProceduralTextureArrayParams>::default(),
                ExtractResourcePlugin::<ProceduralTextureGenerationMetrics>::default(),
            ))
            .init_asset::<TextureDataAsset>()
            .init_asset_loader::<TextureDataLoader>()
            .add_systems(Startup, load_texture_data_handle)
            .add_systems(
                Update,
                (setup_procedural_textures_from_data, poll_ready_signal),
            );

        let render_app = app.sub_app_mut(RenderApp);
        render_app.insert_resource(ProcTexturesReadyTx(tx));
        render_app
            .add_systems(RenderStartup, init_procedural_texture_pipeline)
            .add_systems(
                Render,
                prepare_procedural_texture_bind_group.in_set(RenderSystems::PrepareBindGroups),
            );

        let mut render_graph = render_app.world_mut().resource_mut::<RenderGraph>();
        render_graph.add_node(ProceduralTextureLabel, ProceduralTextureNode::default());
        render_graph.add_node_edge(
            ProceduralTextureLabel,
            bevy::render::graph::CameraDriverLabel,
        );
    }
}

fn load_texture_data_handle(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(TextureDataHandle(asset_server.load(TEXTURE_DATA_PATH)));
}

fn poll_ready_signal(
    mut ready: ResMut<ProcTexturesReady>,
    rx: Res<ProcTexturesReadyRx>,
    mut boot: ResMut<BootReadiness>,
) {
    if ready.0 {
        return;
    }

    // drain
    let Ok(guard) = rx.0.lock() else {
        return;
    };
    while guard.try_recv().is_ok() {
        ready.0 = true;
        boot.0.insert(BootReady::PROC_TEXTURES);
    }
}

fn setup_procedural_textures_from_data(
    mut commands: Commands,
    data_handle: Option<Res<TextureDataHandle>>,
    data_assets: Res<Assets<TextureDataAsset>>,
    mut images: ResMut<Assets<Image>>,
    already_initialized: Option<Res<ProceduralTextureInitialized>>,
) {
    if already_initialized.is_some() {
        return;
    }

    let Some(data_handle) = data_handle else {
        return;
    };
    let Some(data) = data_assets.get(&data_handle.0) else {
        return;
    };

    let specs = &data.specs;
    if specs.is_empty() {
        panic!("{TEXTURE_DATA_PATH} must contain at least one texture spec");
    }
    if specs.len() > MAX_LAYERS {
        panic!(
            "Too many texture specs in {TEXTURE_DATA_PATH}: got {}, MAX_LAYERS={MAX_LAYERS}",
            specs.len()
        );
    }

    let layer_count = u32::try_from(specs.len()).expect("Too many texture layers");

    let mut image = Image::new_target_texture(
        CANONICAL_TEXTURE_SIZE,
        CANONICAL_TEXTURE_SIZE,
        TextureFormat::Rgba8Unorm,
        None,
    );
    image.asset_usage = RenderAssetUsages::RENDER_WORLD;
    image.texture_descriptor.size.depth_or_array_layers = layer_count;
    image.texture_descriptor.usage =
        TextureUsages::COPY_DST | TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING;
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::D2Array),
        ..default()
    });
    if let Some(data) = image.data.as_mut() {
        let pixel_size = image
            .texture_descriptor
            .format
            .pixel_size()
            .expect("ProceduralTexture output image format must have a known pixel size");
        let byte_len = pixel_size * image.texture_descriptor.size.volume() as usize;
        data.resize(byte_len, 0);
    }

    commands.insert_resource(ProceduralTextureGenerationMetrics {
        started_at: Instant::now(),
        texture_size: image.texture_descriptor.size,
        texture_format: image.texture_descriptor.format,
        mip_level_count: image.texture_descriptor.mip_level_count.max(1),
        texture_dimension: image.texture_descriptor.dimension,
    });

    let texture = images.add(image);

    commands.insert_resource(ProceduralTextureImages {
        array: texture.clone(),
    });
    commands.insert_resource(ProceduralTextureArrayParams::from_specs(specs));
    commands.insert_resource(BlockTextureArray(texture));
    commands.insert_resource(ProceduralTextureInitialized);
}

#[derive(Resource)]
struct ProceduralTextureInitialized;

#[derive(Resource)]
struct TextureDataHandle(Handle<TextureDataAsset>);

#[derive(Resource, Clone, ExtractResource)]
struct ProceduralTextureImages {
    array: Handle<Image>,
}

#[derive(Resource, Clone, ExtractResource)]
struct ProceduralTextureGenerationMetrics {
    started_at: Instant,
    texture_size: Extent3d,
    texture_format: TextureFormat,
    mip_level_count: u32,
    texture_dimension: TextureDimension,
}

#[derive(Resource, Clone, ExtractResource, ShaderType)]
struct ProceduralTextureLayerParams {
    seed: u32,
    octaves: u32,
    _pad0: UVec2,
    noise_scale: f32,
    warp_strength: f32,
    _pad1: Vec2,
    palette: [Vec4; 4],
}

#[derive(Resource, Clone, ExtractResource, ShaderType)]
struct ProceduralTextureArrayParams {
    layer_count: u32,
    _pad0: UVec3,
    layers: [ProceduralTextureLayerParams; MAX_LAYERS],
}

impl ProceduralTextureArrayParams {
    fn from_specs(specs: &[TextureSpec]) -> Self {
        let layer_count = u32::try_from(specs.len()).expect("Too many layers");

        let mut layers = std::array::from_fn(|_| ProceduralTextureLayerParams {
            seed: 0,
            octaves: 1,
            _pad0: UVec2::ZERO,
            noise_scale: 1.0,
            warp_strength: 0.0,
            _pad1: Vec2::ZERO,
            palette: [Vec4::ZERO; 4],
        });

        for (i, spec) in specs.iter().enumerate() {
            layers[i] = spec.to_layer_params();
        }

        Self {
            layer_count,
            _pad0: UVec3::ZERO,
            layers,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct TextureSpec {
    #[serde(rename = "name")]
    _name: String,
    size: u32,
    seed: u32,
    octaves: u32,
    noise_scale: f32,
    warp_strength: f32,
    palette: [[u8; 3]; 4],
}

impl TextureSpec {
    fn to_layer_params(&self) -> ProceduralTextureLayerParams {
        let requested_size = self.size.max(1);
        let ratio = CANONICAL_TEXTURE_SIZE as f32 / requested_size as f32;

        // 归一到 64×64 的“语义缩放”策略：
        // - noise_scale：按分辨率比例缩放，让噪声周期（以像素计）保持接近不变
        // - warp_strength：按反比例缩放，让 warp 的像素位移保持接近不变
        let noise_scale = (self.noise_scale * ratio).max(1e-6);
        let warp_strength = (self.warp_strength / ratio).max(0.0);

        ProceduralTextureLayerParams {
            seed: self.seed,
            octaves: self.octaves.max(1),
            _pad0: UVec2::ZERO,
            noise_scale,
            warp_strength,
            _pad1: Vec2::ZERO,
            palette: self.palette.map(|rgb| {
                Vec4::new(
                    srgb_u8_to_linear(rgb[0]),
                    srgb_u8_to_linear(rgb[1]),
                    srgb_u8_to_linear(rgb[2]),
                    1.0,
                )
            }),
        }
    }
}

fn srgb_u8_to_linear(v: u8) -> f32 {
    let x = v as f32 / 255.0;
    if x <= 0.04045 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}

#[derive(Asset, TypePath, Debug, Clone)]
struct TextureDataAsset {
    specs: Vec<TextureSpec>,
}

#[derive(Default, TypePath)]
struct TextureDataLoader;

#[derive(Debug)]
struct TextureDataLoadError {
    message: String,
}

impl std::fmt::Display for TextureDataLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for TextureDataLoadError {}

impl bevy::asset::AssetLoader for TextureDataLoader {
    type Asset = TextureDataAsset;
    type Settings = ();
    type Error = TextureDataLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .await
            .map_err(|e| TextureDataLoadError {
                message: e.to_string(),
            })?;
        let specs: Vec<TextureSpec> =
            serde_json::from_slice(&bytes).map_err(|e| TextureDataLoadError {
                message: e.to_string(),
            })?;
        Ok(TextureDataAsset { specs })
    }

    fn extensions(&self) -> &[&str] {
        &["texture.json"]
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct ProceduralArrayMaterial {
    #[texture(0, dimension = "2d_array")]
    #[sampler(1)]
    array_texture: Handle<Image>,
}

impl Material for ProceduralArrayMaterial {
    fn fragment_shader() -> ShaderRef {
        MATERIAL_SHADER_ASSET_PATH.into()
    }
}

#[derive(Resource)]
struct ProceduralTextureBindGroup(BindGroup);

#[derive(Resource)]
struct ProceduralTextureDispatchThisFrame(bool);

#[derive(Resource)]
struct ProceduralTexturePipeline {
    bind_group_layout: BindGroupLayoutDescriptor,
    pipeline: CachedComputePipelineId,
}

fn init_procedural_texture_pipeline(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    pipeline_cache: Res<PipelineCache>,
) {
    let bind_group_layout = BindGroupLayoutDescriptor::new(
        "ProceduralTextureBindGroup",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                texture_storage_2d_array(
                    TextureFormat::Rgba8Unorm,
                    StorageTextureAccess::WriteOnly,
                ),
                uniform_buffer::<ProceduralTextureArrayParams>(false),
            ),
        ),
    );

    let shader = asset_server.load(SHADER_ASSET_PATH);
    let pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some(Cow::Borrowed("procedural_texture_pipeline")),
        layout: vec![bind_group_layout.clone()],
        shader,
        entry_point: Some(Cow::Borrowed("main")),
        ..default()
    });

    commands.insert_resource(ProceduralTexturePipeline {
        bind_group_layout,
        pipeline,
    });
    commands.insert_resource(ProceduralTextureDispatchThisFrame(false));
}

fn prepare_procedural_texture_bind_group(
    mut commands: Commands,
    pipeline: Res<ProceduralTexturePipeline>,
    pipeline_cache: Res<PipelineCache>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    procedural_images: Option<Res<ProceduralTextureImages>>,
    params: Option<Res<ProceduralTextureArrayParams>>,
    render_device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    existing: Option<Res<ProceduralTextureBindGroup>>,
) {
    if existing.is_some() {
        return;
    }

    let (Some(procedural_images), Some(params)) = (procedural_images, params) else {
        return;
    };

    let Some(gpu_image) = gpu_images.get(&procedural_images.array) else {
        return;
    };

    let mut uniform_buffer = UniformBuffer::from(params.into_inner());
    uniform_buffer.write_buffer(&render_device, &queue);

    let bind_group = render_device.create_bind_group(
        None,
        &pipeline_cache.get_bind_group_layout(&pipeline.bind_group_layout),
        &BindGroupEntries::sequential((&gpu_image.texture_view, &uniform_buffer)),
    );
    commands.insert_resource(ProceduralTextureBindGroup(bind_group));
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct ProceduralTextureLabel;

enum ProceduralTextureNodeState {
    Loading,
    Dispatching,
    Done,
}

struct ProceduralTextureNode {
    state: ProceduralTextureNodeState,
}

impl Default for ProceduralTextureNode {
    fn default() -> Self {
        Self {
            state: ProceduralTextureNodeState::Loading,
        }
    }
}

impl render_graph::Node for ProceduralTextureNode {
    fn update(&mut self, world: &mut World) {
        let mut dispatch = false;

        if !world.contains_resource::<ProceduralTextureBindGroup>() {
            world.insert_resource(ProceduralTextureDispatchThisFrame(false));
            return;
        }

        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = world.resource::<ProceduralTexturePipeline>();

        match self.state {
            ProceduralTextureNodeState::Loading => {
                match pipeline_cache.get_compute_pipeline_state(pipeline.pipeline) {
                    CachedPipelineState::Ok(_) => {
                        dispatch = true;
                        self.state = ProceduralTextureNodeState::Dispatching;
                    }
                    CachedPipelineState::Err(PipelineCacheError::ShaderNotLoaded(_)) => {}
                    CachedPipelineState::Err(err) => {
                        panic!("Initializing assets/{SHADER_ASSET_PATH}:\n{err}")
                    }
                    _ => {}
                }
            }
            ProceduralTextureNodeState::Dispatching => {
                if let Some(metrics) = world.get_resource::<ProceduralTextureGenerationMetrics>() {
                    let elapsed = metrics.started_at.elapsed();
                    let bytes = estimate_texture_vram_bytes(
                        metrics.texture_size,
                        metrics.texture_dimension,
                        metrics.texture_format,
                        metrics.mip_level_count,
                    );
                    log::info!(
                        "程序化纹理生成完成：耗时={elapsed:?}，显存占用≈{}（{} bytes），纹理={}×{}@{} mip={} format={:?}",
                        format_bytes(bytes),
                        bytes,
                        metrics.texture_size.width,
                        metrics.texture_size.height,
                        metrics.texture_size.depth_or_array_layers,
                        metrics.mip_level_count,
                        metrics.texture_format,
                    );
                } else {
                    log::warn!(
                        "程序化纹理生成完成，但缺少统计信息（ProceduralTextureGenerationMetrics）"
                    );
                }

                if let Some(tx) = world.get_resource::<ProcTexturesReadyTx>() {
                    let _ = tx.0.send(());
                }

                self.state = ProceduralTextureNodeState::Done;
            }
            ProceduralTextureNodeState::Done => {}
        }

        if let Some(mut flag) = world.get_resource_mut::<ProceduralTextureDispatchThisFrame>() {
            flag.0 = dispatch;
        } else {
            world.insert_resource(ProceduralTextureDispatchThisFrame(dispatch));
        }
    }

    fn run(
        &self,
        _graph: &mut render_graph::RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), render_graph::NodeRunError> {
        let should_dispatch = world
            .get_resource::<ProceduralTextureDispatchThisFrame>()
            .is_some_and(|v| v.0);
        if !should_dispatch {
            return Ok(());
        }

        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = world.resource::<ProceduralTexturePipeline>();
        let bind_group = &world.resource::<ProceduralTextureBindGroup>().0;

        let gpu_images = world.resource::<RenderAssets<GpuImage>>();
        let images = world.resource::<ProceduralTextureImages>();
        let gpu_image = gpu_images
            .get(&images.array)
            .expect("ProceduralTexture output image wasn't prepared to GPU yet");

        let x_workgroups = (gpu_image.size.width + WORKGROUP_SIZE - 1) / WORKGROUP_SIZE;
        let y_workgroups = (gpu_image.size.height + WORKGROUP_SIZE - 1) / WORKGROUP_SIZE;
        let layer_workgroups = gpu_image.size.depth_or_array_layers;

        let compute_pipeline = pipeline_cache
            .get_compute_pipeline(pipeline.pipeline)
            .expect("ProceduralTexture compute pipeline wasn't ready");

        let mut pass = render_context
            .command_encoder()
            .begin_compute_pass(&ComputePassDescriptor::default());
        pass.set_pipeline(compute_pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.dispatch_workgroups(x_workgroups, y_workgroups, layer_workgroups);

        Ok(())
    }
}

fn estimate_texture_vram_bytes(
    size: Extent3d,
    dimension: TextureDimension,
    format: TextureFormat,
    mip_level_count: u32,
) -> u64 {
    let pixel_size = match format.pixel_size() {
        Ok(pixel_size) => pixel_size,
        Err(_) => return 0,
    };

    let mut width = size.width.max(1);
    let mut height = size.height.max(1);
    let mut depth_or_layers = size.depth_or_array_layers.max(1);

    let mut total = 0u64;
    let mip_count = mip_level_count.max(1);
    for _ in 0..mip_count {
        total = total.saturating_add(
            u64::from(width) * u64::from(height) * u64::from(depth_or_layers) * pixel_size as u64,
        );

        width = (width / 2).max(1);
        height = (height / 2).max(1);
        if matches!(dimension, TextureDimension::D3) {
            depth_or_layers = (depth_or_layers / 2).max(1);
        }
    }

    total
}

fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

    let b = bytes as f64;
    if b >= GIB {
        format!("{:.2} GiB", b / GIB)
    } else if b >= MIB {
        format!("{:.2} MiB", b / MIB)
    } else if b >= KIB {
        format!("{:.2} KiB", b / KIB)
    } else {
        format!("{bytes} B")
    }
}
