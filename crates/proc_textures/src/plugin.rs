//! 程序化贴图（GPU compute / texture array）生成服务。
//!
//! 设计目标：
//! - 纯服务：不默认 spawn 预览实体（相机/灯光/方块）
//! - RenderApp 内 dispatch compute，一次性生成 texture array
//! - 主世界通过 channel 收到 ready 信号，用于 BootLoading 聚合

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{mpsc, Mutex};
use std::time::Instant;

use bevy::{
    asset::RenderAssetUsages,
    asset::{io::Reader, LoadContext},
    image::TextureFormatPixelInfo,
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
const MIN_NOISE_SCALE: f32 = 1.0;

/// 程序化纹理 array 的句柄（未来供 voxel/material 使用）。
#[derive(Resource, Clone)]
pub struct BlockTextureArray(pub Handle<Image>);

/// 程序化贴图服务状态。
#[derive(Resource, Debug, Clone, Default)]
pub enum ProcTexturesStatus {
    #[default]
    Loading,
    Ready,
    Failed(String),
}

#[derive(Debug, Clone)]
enum ProcTexturesSignal {
    Ready,
    Failed(String),
}

#[derive(Resource, Debug, Clone, Default)]
pub struct ProcTexturesReady(pub bool);

#[derive(Resource, Debug, Clone, Default)]
pub struct TextureRegistry {
    name_to_layer: HashMap<String, u16>,
}

impl TextureRegistry {
    pub fn layer_index(&self, name: &str) -> Result<u16, String> {
        self.name_to_layer
            .get(name)
            .copied()
            .ok_or_else(|| format!("Texture registry missing entry: {name}"))
    }

    fn from_specs(specs: &[TextureSpec]) -> Result<Self, String> {
        let mut name_to_layer = HashMap::with_capacity(specs.len());
        for (i, spec) in specs.iter().enumerate() {
            let index = u16::try_from(i)
                .map_err(|_| format!("Texture layer index overflows u16: {i}"))?;
            if name_to_layer.insert(spec.name.clone(), index).is_some() {
                return Err(format!("Duplicate texture name: {}", spec.name));
            }
        }
        Ok(Self { name_to_layer })
    }
}

#[derive(Resource)]
struct ProcTexturesReadyRx(Mutex<mpsc::Receiver<ProcTexturesSignal>>);

#[derive(Resource, Clone)]
struct ProcTexturesReadyTx(mpsc::Sender<ProcTexturesSignal>);

pub struct ProcTexturesPlugin;

impl Plugin for ProcTexturesPlugin {
    fn build(&self, app: &mut App) {
        let (tx, rx) = mpsc::channel::<ProcTexturesSignal>();

        app.init_resource::<ProcTexturesReady>()
            .init_resource::<ProcTexturesStatus>()
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
                prepare_procedural_texture_params_buffer.in_set(RenderSystems::PrepareBindGroups),
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
    mut status: ResMut<ProcTexturesStatus>,
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
    while let Ok(signal) = guard.try_recv() {
        match signal {
            ProcTexturesSignal::Ready => {
                ready.0 = true;
                *status = ProcTexturesStatus::Ready;
                boot.0.insert(BootReady::PROC_TEXTURES);
            }
            ProcTexturesSignal::Failed(message) => {
                *status = ProcTexturesStatus::Failed(message.clone());
                log::error!("Procedural textures initialization failed: {message}");
            }
        }
    }
}

fn setup_procedural_textures_from_data(
    mut commands: Commands,
    data_handle: Option<Res<TextureDataHandle>>,
    data_assets: Res<Assets<TextureDataAsset>>,
    mut images: ResMut<Assets<Image>>,
    mut status: ResMut<ProcTexturesStatus>,
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

    let registry = match TextureRegistry::from_specs(specs) {
        Ok(registry) => registry,
        Err(message) => {
            *status = ProcTexturesStatus::Failed(message.clone());
            log::error!("Texture registry build failed: {message}");
            return;
        }
    };

    let layer_count = match u32::try_from(specs.len()) {
        Ok(layer_count) => layer_count,
        Err(_) => {
            let message = format!("Too many texture layers in {TEXTURE_DATA_PATH}: {}", specs.len());
            *status = ProcTexturesStatus::Failed(message.clone());
            log::error!("{message}");
            return;
        }
    };
    let mip_level_count = calculate_mip_level_count(CANONICAL_TEXTURE_SIZE);

    let mut image = Image::new_target_texture(
        CANONICAL_TEXTURE_SIZE,
        CANONICAL_TEXTURE_SIZE,
        TextureFormat::Rgba8Unorm,
        None,
    );
    image.asset_usage = RenderAssetUsages::RENDER_WORLD;
    image.texture_descriptor.size.depth_or_array_layers = layer_count;
    image.texture_descriptor.mip_level_count = mip_level_count;
    image.texture_descriptor.usage =
        TextureUsages::COPY_DST | TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING;
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::D2Array),
        ..default()
    });
    // 纹理内容完全由 RenderGraph compute pass 写入，不保留 CPU 像素副本。
    image.data = None;

    commands.insert_resource(ProceduralTextureGenerationMetrics {
        started_at: Instant::now(),
        texture_size: image.texture_descriptor.size,
        texture_format: image.texture_descriptor.format,
        mip_level_count,
        texture_dimension: image.texture_descriptor.dimension,
    });

    let texture = images.add(image);

    commands.insert_resource(ProceduralTextureImages {
        array: texture.clone(),
    });
    commands.insert_resource(ProceduralTextureArrayParams::from_specs(specs));
    commands.insert_resource(registry);
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
#[serde(rename_all = "snake_case")]
enum TextureStyle {
    MinecraftQuantized,
    HdPixelArt,
    HdRealistic,
    VectorToon,
}

impl Default for TextureStyle {
    fn default() -> Self {
        Self::MinecraftQuantized
    }
}

#[derive(Debug, Clone, Deserialize)]
struct TextureSpec {
    name: String,
    size: u32,
    seed: u32,
    octaves: u32,
    noise_scale: f32,
    warp_strength: f32,
    palette: [[u8; 3]; 4],
    #[serde(default)]
    style: TextureStyle,
}

impl TextureSpec {
    fn to_layer_params(&self) -> ProceduralTextureLayerParams {
        let requested_size = self.size.max(1);
        let ratio = CANONICAL_TEXTURE_SIZE as f32 / requested_size as f32;
        // 归一到 64×64 的“语义缩放”策略：
        // - noise_scale：按分辨率比例缩放，让噪声周期（以像素计）保持接近不变
        // - warp_strength：按反比例缩放，让 warp 的像素位移保持接近不变
        let noise_scale = (self.noise_scale * ratio).max(MIN_NOISE_SCALE);
        let warp_strength = (self.warp_strength / ratio).max(0.0);

        match self.style {
            TextureStyle::MinecraftQuantized => {}
            _ => {
                log::warn!(
                    "Texture '{}' requests style {:?}, fallback to minecraft_quantized",
                    self.name,
                    self.style
                );
            }
        }

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

#[derive(Debug, Clone, Deserialize)]
struct TextureDataV1 {
    schema_version: u32,
    textures: Vec<TextureSpec>,
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

fn validate_specs(specs: &[TextureSpec]) -> Result<(), TextureDataLoadError> {
    if specs.is_empty() {
        return Err(TextureDataLoadError {
            message: format!("{TEXTURE_DATA_PATH} must contain at least one texture spec"),
        });
    }
    if specs.len() > MAX_LAYERS {
        return Err(TextureDataLoadError {
            message: format!(
                "Too many texture specs in {TEXTURE_DATA_PATH}: got {}, MAX_LAYERS={MAX_LAYERS}",
                specs.len()
            ),
        });
    }

    let mut names = HashMap::new();
    for spec in specs {
        if names.insert(spec.name.clone(), true).is_some() {
            return Err(TextureDataLoadError {
                message: format!("Duplicate texture name in {TEXTURE_DATA_PATH}: {}", spec.name),
            });
        }
        if spec.size == 0 {
            return Err(TextureDataLoadError {
                message: format!("Texture '{}' has invalid size=0", spec.name),
            });
        }
        if spec.noise_scale <= 0.0 {
            return Err(TextureDataLoadError {
                message: format!("Texture '{}' has invalid noise_scale={}", spec.name, spec.noise_scale),
            });
        }
    }

    Ok(())
}

fn parse_texture_specs_from_bytes(bytes: &[u8]) -> Result<Vec<TextureSpec>, TextureDataLoadError> {
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| TextureDataLoadError {
        message: e.to_string(),
    })?;

    let specs = if value.is_array() {
        serde_json::from_value::<Vec<TextureSpec>>(value).map_err(|e| TextureDataLoadError {
            message: format!("Invalid legacy v1 payload: {e}"),
        })?
    } else {
        let wrapper = serde_json::from_slice::<TextureDataV1>(bytes).map_err(|e| TextureDataLoadError {
            message: format!("Invalid schema wrapper: {e}"),
        })?;
        if wrapper.schema_version != 1 {
            return Err(TextureDataLoadError {
                message: format!("Unsupported schema_version: {}", wrapper.schema_version),
            });
        }
        wrapper.textures
    };

    validate_specs(&specs)?;
    Ok(specs)
}

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

        let specs = parse_texture_specs_from_bytes(&bytes)?;
        Ok(TextureDataAsset { specs })
    }

    fn extensions(&self) -> &[&str] {
        &["texture.json"]
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_maps_names_to_layers() {
        let specs = vec![
            TextureSpec { name: "a".into(), size: 16, seed: 1, octaves: 1, noise_scale: 1.0, warp_strength: 0.0, palette: [[0,0,0];4], style: TextureStyle::MinecraftQuantized },
            TextureSpec { name: "b".into(), size: 16, seed: 2, octaves: 1, noise_scale: 1.0, warp_strength: 0.0, palette: [[0,0,0];4], style: TextureStyle::MinecraftQuantized },
        ];
        let registry = TextureRegistry::from_specs(&specs).expect("registry");
        assert_eq!(registry.layer_index("a").unwrap(), 0);
        assert_eq!(registry.layer_index("b").unwrap(), 1);
        assert!(registry.layer_index("missing").is_err());
    }

    #[test]
    fn duplicate_names_fail() {
        let specs = vec![
            TextureSpec { name: "dup".into(), size: 16, seed: 1, octaves: 1, noise_scale: 1.0, warp_strength: 0.0, palette: [[0,0,0];4], style: TextureStyle::MinecraftQuantized },
            TextureSpec { name: "dup".into(), size: 16, seed: 2, octaves: 1, noise_scale: 1.0, warp_strength: 0.0, palette: [[0,0,0];4], style: TextureStyle::MinecraftQuantized },
        ];
        assert!(TextureRegistry::from_specs(&specs).is_err());
    }

    #[test]
    fn schema_version_dispatch_works() {
        let v1_wrapped = br#"{"schema_version":1,"textures":[{"name":"ok","size":16,"seed":1,"octaves":1,"noise_scale":1.0,"warp_strength":0.0,"palette":[[0,0,0],[1,1,1],[2,2,2],[3,3,3]]}]}"#;
        assert!(parse_texture_specs_from_bytes(v1_wrapped).is_ok());

        let legacy = br#"[{"name":"ok","size":16,"seed":1,"octaves":1,"noise_scale":1.0,"warp_strength":0.0,"palette":[[0,0,0],[1,1,1],[2,2,2],[3,3,3]]}]"#;
        assert!(parse_texture_specs_from_bytes(legacy).is_ok());
    }

    #[test]
    fn invalid_payload_returns_error() {
        let bad = br#"{"schema_version":2,"textures":[]}"#;
        assert!(parse_texture_specs_from_bytes(bad).is_err());
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
struct ProceduralTextureParamsBuffer(UniformBuffer<ProceduralTextureArrayParams>);

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

fn prepare_procedural_texture_params_buffer(
    mut commands: Commands,
    params: Option<Res<ProceduralTextureArrayParams>>,
    render_device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    existing: Option<Res<ProceduralTextureParamsBuffer>>,
) {
    if existing.is_some() {
        return;
    }

    let Some(params) = params else {
        return;
    };

    let mut uniform_buffer = UniformBuffer::from(params.into_inner().clone());
    uniform_buffer.set_label(Some("procedural_texture_params"));
    uniform_buffer.write_buffer(&render_device, &queue);
    commands.insert_resource(ProceduralTextureParamsBuffer(uniform_buffer));
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

        if !is_procedural_texture_dispatch_ready(world) {
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
                        let message = format!("Initializing assets/{SHADER_ASSET_PATH}:\n{err}");
                        if let Some(tx) = world.get_resource::<ProcTexturesReadyTx>() {
                            let _ = tx.0.send(ProcTexturesSignal::Failed(message.clone()));
                        }
                        log::error!("{message}");
                        self.state = ProceduralTextureNodeState::Done;
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
                    let _ = tx.0.send(ProcTexturesSignal::Ready);
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
        let params_buffer = world.resource::<ProceduralTextureParamsBuffer>();
        let render_device = world.resource::<RenderDevice>();
        let bind_group_layout = pipeline_cache.get_bind_group_layout(&pipeline.bind_group_layout);

        let gpu_images = world.resource::<RenderAssets<GpuImage>>();
        let images = world.resource::<ProceduralTextureImages>();
        let gpu_image = gpu_images
            .get(&images.array)
            .expect("ProceduralTexture output image wasn't prepared to GPU yet");

        let compute_pipeline = pipeline_cache
            .get_compute_pipeline(pipeline.pipeline)
            .expect("ProceduralTexture compute pipeline wasn't ready");

        let mut pass = render_context
            .command_encoder()
            .begin_compute_pass(&ComputePassDescriptor::default());
        pass.set_pipeline(compute_pipeline);
        let mip_level_count = gpu_image.mip_level_count.max(1);
        let layer_workgroups = gpu_image.size.depth_or_array_layers.max(1);

        for mip_level in 0..mip_level_count {
            let mip_width = mip_extent(gpu_image.size.width, mip_level);
            let mip_height = mip_extent(gpu_image.size.height, mip_level);
            let mip_view = gpu_image.texture.create_view(&TextureViewDescriptor {
                format: Some(gpu_image.texture_format),
                dimension: Some(TextureViewDimension::D2Array),
                base_mip_level: mip_level,
                mip_level_count: Some(1),
                base_array_layer: 0,
                array_layer_count: Some(layer_workgroups),
                ..default()
            });
            let bind_group = render_device.create_bind_group(
                None,
                &bind_group_layout,
                &BindGroupEntries::sequential((&mip_view, &params_buffer.0)),
            );

            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(
                workgroup_count(mip_width),
                workgroup_count(mip_height),
                layer_workgroups,
            );
        }

        Ok(())
    }
}

fn is_procedural_texture_dispatch_ready(world: &World) -> bool {
    let Some(params_buffer) = world.get_resource::<ProceduralTextureParamsBuffer>() else {
        return false;
    };
    if params_buffer.0.buffer().is_none() {
        return false;
    }

    let Some(images) = world.get_resource::<ProceduralTextureImages>() else {
        return false;
    };
    let Some(gpu_images) = world.get_resource::<RenderAssets<GpuImage>>() else {
        return false;
    };
    gpu_images.get(&images.array).is_some()
}

fn calculate_mip_level_count(size: u32) -> u32 {
    let clamped = size.max(1);
    u32::BITS - clamped.leading_zeros()
}

fn mip_extent(size: u32, mip_level: u32) -> u32 {
    (size >> mip_level).max(1)
}

fn workgroup_count(size: u32) -> u32 {
    size.div_ceil(WORKGROUP_SIZE)
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
