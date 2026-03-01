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
            binding_types::{
                storage_buffer_read_only_sized, texture_storage_2d_array, uniform_buffer,
            },
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
const MIN_NOISE_SCALE: f32 = 1e-6;

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

/// 每个方块六个朝向对应到 texture array layer 的映射（faces 展开结果）。
#[derive(Resource, Debug, Clone, Default)]
pub struct BlockTextureFaceMappings(pub Vec<BlockTextureFaceMapping>);

#[derive(Debug, Clone)]
pub struct BlockTextureFaceMapping {
    pub name: String,
    pub legacy: u32,
    pub top: u32,
    pub bottom: u32,
    pub north: u32,
    pub south: u32,
    pub east: u32,
    pub west: u32,
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
                ExtractResourcePlugin::<ProceduralTexturePaletteStorage>::default(),
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
                (
                    prepare_procedural_texture_params_buffer,
                    prepare_procedural_texture_palette_buffer,
                )
                    .in_set(RenderSystems::PrepareBindGroups),
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

    let layers = &data.layers;
    if layers.is_empty() {
        let message = format!("{TEXTURE_DATA_PATH} must contain at least one texture layer");
        *status = ProcTexturesStatus::Failed(message.clone());
        log::error!("{message}");
        return;
    }
    if layers.len() > MAX_LAYERS {
        let message = format!(
            "Too many texture layers in {TEXTURE_DATA_PATH}: got {}, MAX_LAYERS={MAX_LAYERS}",
            layers.len()
        );
        *status = ProcTexturesStatus::Failed(message.clone());
        log::error!("{message}");
        return;
    }

    let layer_count = match u32::try_from(layers.len()) {
        Ok(layer_count) => layer_count,
        Err(_) => {
            let message = format!("Too many texture layers in {TEXTURE_DATA_PATH}: {}", layers.len());
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
    // data=None 时不需要 resize copy；否则首次上传会触发“无 previous asset”告警。
    image.copy_on_resize = false;

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
    let (array_params, palette_storage) = ProceduralTextureArrayParams::from_layers(layers);
    commands.insert_resource(array_params);
    commands.insert_resource(palette_storage);
    commands.insert_resource(BlockTextureFaceMappings(data.face_mappings.clone()));
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
    style: u32,
    seed: u32,
    octaves: u32,
    has_layer: u32,
    noise_scale: f32,
    warp_strength: f32,
    layer_ratio: f32,
    base_palette_offset: u32,
    base_palette_len: u32,
    top_palette_offset: u32,
    top_palette_len: u32,
    _pad0: u32,
}

#[derive(Resource, Clone, ExtractResource, ShaderType)]
struct ProceduralTextureArrayParams {
    layer_count: u32,
    _pad0: UVec3,
    _pad1: u32,
    layers: [ProceduralTextureLayerParams; MAX_LAYERS],
}

#[derive(Resource, Clone, ExtractResource, ShaderType)]
struct ProceduralTexturePaletteStorage {
    color_count: encase::ArrayLength,
    #[shader(size(runtime))]
    colors: Vec<Vec4>,
}

impl ProceduralTextureArrayParams {
    fn from_layers(layers_specs: &[ExpandedLayerSpec]) -> (Self, ProceduralTexturePaletteStorage) {
        let layer_count = u32::try_from(layers_specs.len()).expect("Too many layers");
        let mut palette_colors = Vec::new();

        let mut layers = std::array::from_fn(|_| ProceduralTextureLayerParams {
            style: TextureStyle::Minecraft.shader_style_tag(),
            seed: 0,
            octaves: 1,
            has_layer: 0,
            noise_scale: 1.0,
            warp_strength: 0.0,
            layer_ratio: 0.0,
            base_palette_offset: 0,
            base_palette_len: 1,
            top_palette_offset: 0,
            top_palette_len: 0,
            _pad0: 0,
        });

        for (i, spec) in layers_specs.iter().enumerate() {
            layers[i] = spec.to_layer_params(&mut palette_colors);
        }

        (
            Self {
                layer_count,
                _pad0: UVec3::ZERO,
                _pad1: 0,
                layers,
            },
            ProceduralTexturePaletteStorage {
                color_count: encase::ArrayLength,
                colors: palette_colors,
            },
        )
    }
}

#[derive(Debug, Clone)]
struct FaceLayerSpec {
    style: TextureStyle,
    size: u32,
    seed: u32,
    octaves: u32,
    noise_scale: f32,
    warp_strength: f32,
    has_layer: bool,
    layer_ratio: f32,
    base_palette: Vec<[u8; 3]>,
    top_palette: Vec<[u8; 3]>,
}

impl FaceLayerSpec {
    fn to_layer_params(&self, palette_colors: &mut Vec<Vec4>) -> ProceduralTextureLayerParams {
        let requested_size = self.size.max(1);
        let ratio = CANONICAL_TEXTURE_SIZE as f32 / requested_size as f32;
        // 归一到 64×64 的“语义缩放”策略：
        // - noise_scale：按分辨率比例缩放，让噪声周期（以像素计）保持接近不变
        // - warp_strength：按反比例缩放，让 warp 的像素位移保持接近不变
        let noise_scale = (self.noise_scale * ratio).max(MIN_NOISE_SCALE);
        let warp_strength = (self.warp_strength / ratio).max(0.0);
        let (base_palette_offset, base_palette_len) =
            append_palette(palette_colors, &self.base_palette);
        let (top_palette_offset, top_palette_len) = if self.has_layer {
            append_palette(palette_colors, &self.top_palette)
        } else {
            (0, 0)
        };

        ProceduralTextureLayerParams {
            style: self.style.shader_style_tag(),
            seed: self.seed,
            octaves: self.octaves.max(1),
            has_layer: u32::from(self.has_layer),
            noise_scale,
            warp_strength,
            layer_ratio: self.layer_ratio,
            base_palette_offset,
            base_palette_len,
            top_palette_offset,
            top_palette_len,
            _pad0: 0,
        }
    }
}

#[derive(Debug, Clone)]
struct ExpandedLayerSpec {
    _block_name: String,
    _face_slot: FaceSlot,
    layer: FaceLayerSpec,
}

impl ExpandedLayerSpec {
    fn to_layer_params(&self, palette_colors: &mut Vec<Vec4>) -> ProceduralTextureLayerParams {
        self.layer.to_layer_params(palette_colors)
    }
}

fn append_palette(palette_colors: &mut Vec<Vec4>, palette: &[[u8; 3]]) -> (u32, u32) {
    let offset = u32::try_from(palette_colors.len()).expect("palette offset exceeds u32::MAX");
    let len = u32::try_from(palette.len()).expect("palette length exceeds u32::MAX");
    palette_colors.extend(palette.iter().copied().map(rgb_u8_to_linear_vec4));
    (offset, len)
}

fn rgb_u8_to_linear_vec4(rgb: [u8; 3]) -> Vec4 {
    Vec4::new(
        srgb_u8_to_linear(rgb[0]),
        srgb_u8_to_linear(rgb[1]),
        srgb_u8_to_linear(rgb[2]),
        1.0,
    )
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
    layers: Vec<ExpandedLayerSpec>,
    face_mappings: Vec<BlockTextureFaceMapping>,
}

#[derive(Default, TypePath)]
struct TextureDataLoader;

#[derive(Debug)]
struct TextureDataLoadError {
    message: String,
}

impl TextureDataLoadError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for TextureDataLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for TextureDataLoadError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum FaceSlot {
    All,
    Top,
    Bottom,
    Sides,
    North,
    South,
    East,
    West,
}

impl FaceSlot {
    fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::Sides => "sides",
            Self::North => "north",
            Self::South => "south",
            Self::East => "east",
            Self::West => "west",
        }
    }

    fn seed_salt(self) -> u32 {
        match self {
            Self::All => 0xA1F0_0001,
            Self::Top => 0xA1F0_0002,
            Self::Bottom => 0xA1F0_0003,
            Self::Sides => 0xA1F0_0004,
            Self::North => 0xA1F0_0005,
            Self::South => 0xA1F0_0006,
            Self::East => 0xA1F0_0007,
            Self::West => 0xA1F0_0008,
        }
    }
}

const ALL_FACE_SLOTS: [FaceSlot; 8] = [
    FaceSlot::All,
    FaceSlot::Top,
    FaceSlot::Bottom,
    FaceSlot::Sides,
    FaceSlot::North,
    FaceSlot::South,
    FaceSlot::East,
    FaceSlot::West,
];

const LEGACY_PRIMARY_FACE_PRIORITY: [FaceSlot; 8] = [
    FaceSlot::All,
    FaceSlot::Sides,
    FaceSlot::Top,
    FaceSlot::Bottom,
    FaceSlot::North,
    FaceSlot::South,
    FaceSlot::East,
    FaceSlot::West,
];

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum TextureStyle {
    Minecraft,
    HdPixelArt,
    HdRealistic,
    VectorToon,
}

impl TextureStyle {
    fn shader_style_tag(self) -> u32 {
        match self {
            Self::Minecraft => 0,
            Self::HdPixelArt => 1,
            Self::HdRealistic => 2,
            Self::VectorToon => 3,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct TextureSpecInput {
    name: String,
    size: u32,
    seed: u32,
    style: TextureStyle,
    faces: TextureFacesInput,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct TextureFacesInput {
    #[serde(default)]
    all: Option<FaceSpecInput>,
    #[serde(default)]
    top: Option<FaceSpecInput>,
    #[serde(default)]
    bottom: Option<FaceSpecInput>,
    #[serde(default)]
    sides: Option<FaceSpecInput>,
    #[serde(default)]
    north: Option<FaceSpecInput>,
    #[serde(default)]
    south: Option<FaceSpecInput>,
    #[serde(default)]
    east: Option<FaceSpecInput>,
    #[serde(default)]
    west: Option<FaceSpecInput>,
}

impl TextureFacesInput {
    fn get(&self, slot: FaceSlot) -> Option<&FaceSpecInput> {
        match slot {
            FaceSlot::All => self.all.as_ref(),
            FaceSlot::Top => self.top.as_ref(),
            FaceSlot::Bottom => self.bottom.as_ref(),
            FaceSlot::Sides => self.sides.as_ref(),
            FaceSlot::North => self.north.as_ref(),
            FaceSlot::South => self.south.as_ref(),
            FaceSlot::East => self.east.as_ref(),
            FaceSlot::West => self.west.as_ref(),
        }
    }

    fn is_empty(&self) -> bool {
        ALL_FACE_SLOTS.iter().all(|slot| self.get(*slot).is_none())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct FaceSpecInput {
    has_layer: bool,
    #[serde(default)]
    layer_ratio: Option<f32>,
    #[serde(default)]
    top_layer_palette: Option<Vec<[u8; 3]>>,
    base: BaseSpecInput,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct BaseSpecInput {
    palette: Vec<[u8; 3]>,
    #[serde(default = "default_noise_scale")]
    noise_scale: f32,
    #[serde(default = "default_octaves")]
    octaves: u32,
    #[serde(default)]
    warp_strength: f32,
}

fn default_noise_scale() -> f32 {
    1.0
}

fn default_octaves() -> u32 {
    4
}

#[derive(Debug, Clone)]
struct PreparedBlockSpec {
    name: String,
    primary_face: FaceSlot,
    face_specs: HashMap<FaceSlot, FaceLayerSpec>,
    layer_indices: HashMap<FaceSlot, u32>,
}

fn build_texture_data(
    specs: Vec<TextureSpecInput>,
) -> Result<TextureDataAsset, TextureDataLoadError> {
    if specs.is_empty() {
        return Err(TextureDataLoadError::new(format!(
            "{TEXTURE_DATA_PATH} must contain at least one texture spec"
        )));
    }

    let mut blocks = Vec::with_capacity(specs.len());
    for spec in specs {
        if spec.faces.is_empty() {
            return Err(TextureDataLoadError::new(format!(
                "texture `{}` must define at least one face in `faces`",
                spec.name
            )));
        }

        let mut face_specs = HashMap::new();
        for slot in ALL_FACE_SLOTS {
            let Some(face_input) = spec.faces.get(slot) else {
                continue;
            };
            let normalized = normalize_face_spec(
                &spec.name, spec.size, spec.seed, spec.style, slot, face_input,
            )?;
            face_specs.insert(slot, normalized);
        }

        let Some(primary_face) = LEGACY_PRIMARY_FACE_PRIORITY
            .iter()
            .copied()
            .find(|slot| face_specs.contains_key(slot))
        else {
            return Err(TextureDataLoadError::new(format!(
                "texture `{}` has empty faces after normalization",
                spec.name
            )));
        };

        blocks.push(PreparedBlockSpec {
            name: spec.name,
            primary_face,
            face_specs,
            layer_indices: HashMap::new(),
        });
    }

    let mut layers = Vec::new();

    // 第一轮：每个 block 先放一个 primary layer，保持 legacy material_key 的稳定索引。
    for block in &mut blocks {
        let Some(primary_spec) = block.face_specs.get(&block.primary_face) else {
            return Err(TextureDataLoadError::new(format!(
                "texture `{}` missing primary face `{}`",
                block.name,
                block.primary_face.as_str()
            )));
        };
        let layer_index =
            push_expanded_layer(&mut layers, &block.name, block.primary_face, primary_spec)?;
        block.layer_indices.insert(block.primary_face, layer_index);
    }

    // 第二轮：补齐其余显式定义的 faces。
    for block in &mut blocks {
        for slot in ALL_FACE_SLOTS {
            if slot == block.primary_face {
                continue;
            }
            let Some(spec) = block.face_specs.get(&slot) else {
                continue;
            };
            let layer_index = push_expanded_layer(&mut layers, &block.name, slot, spec)?;
            block.layer_indices.insert(slot, layer_index);
        }
    }

    if layers.len() > MAX_LAYERS {
        return Err(TextureDataLoadError::new(format!(
            "Too many expanded texture layers in {TEXTURE_DATA_PATH}: got {}, MAX_LAYERS={MAX_LAYERS}",
            layers.len()
        )));
    }

    let mut face_mappings = Vec::with_capacity(blocks.len());
    for block in &blocks {
        face_mappings.push(build_face_mapping(block)?);
    }

    Ok(TextureDataAsset {
        layers,
        face_mappings,
    })
}

fn push_expanded_layer(
    layers: &mut Vec<ExpandedLayerSpec>,
    block_name: &str,
    face_slot: FaceSlot,
    layer_spec: &FaceLayerSpec,
) -> Result<u32, TextureDataLoadError> {
    if layers.len() >= MAX_LAYERS {
        return Err(TextureDataLoadError::new(format!(
            "expanded layers exceed MAX_LAYERS={MAX_LAYERS} while processing `{}`",
            block_name
        )));
    }
    let index = u32::try_from(layers.len()).map_err(|_| {
        TextureDataLoadError::new(format!(
            "layer index overflow while processing `{}`",
            block_name
        ))
    })?;
    layers.push(ExpandedLayerSpec {
        _block_name: block_name.to_string(),
        _face_slot: face_slot,
        layer: layer_spec.clone(),
    });
    Ok(index)
}

fn build_face_mapping(
    block: &PreparedBlockSpec,
) -> Result<BlockTextureFaceMapping, TextureDataLoadError> {
    let Some(&legacy) = block.layer_indices.get(&block.primary_face) else {
        return Err(TextureDataLoadError::new(format!(
            "texture `{}` missing legacy layer index",
            block.name
        )));
    };

    Ok(BlockTextureFaceMapping {
        name: block.name.clone(),
        legacy,
        top: resolve_layer_index(
            &block.layer_indices,
            legacy,
            [FaceSlot::Top, FaceSlot::Sides, FaceSlot::All],
        ),
        bottom: resolve_layer_index(
            &block.layer_indices,
            legacy,
            [FaceSlot::Bottom, FaceSlot::Sides, FaceSlot::All],
        ),
        north: resolve_layer_index(
            &block.layer_indices,
            legacy,
            [FaceSlot::North, FaceSlot::Sides, FaceSlot::All],
        ),
        south: resolve_layer_index(
            &block.layer_indices,
            legacy,
            [FaceSlot::South, FaceSlot::Sides, FaceSlot::All],
        ),
        east: resolve_layer_index(
            &block.layer_indices,
            legacy,
            [FaceSlot::East, FaceSlot::Sides, FaceSlot::All],
        ),
        west: resolve_layer_index(
            &block.layer_indices,
            legacy,
            [FaceSlot::West, FaceSlot::Sides, FaceSlot::All],
        ),
    })
}

fn resolve_layer_index(
    indices: &HashMap<FaceSlot, u32>,
    fallback: u32,
    priority: [FaceSlot; 3],
) -> u32 {
    for slot in priority {
        if let Some(index) = indices.get(&slot) {
            return *index;
        }
    }
    fallback
}

fn normalize_face_spec(
    block_name: &str,
    block_size: u32,
    block_seed: u32,
    block_style: TextureStyle,
    face_slot: FaceSlot,
    input: &FaceSpecInput,
) -> Result<FaceLayerSpec, TextureDataLoadError> {
    if !input.base.noise_scale.is_finite() || input.base.noise_scale <= 0.0 {
        return Err(TextureDataLoadError::new(format!(
            "texture `{}` face `{}` has invalid `base.noise_scale`: {} (must be > 0)",
            block_name,
            face_slot.as_str(),
            input.base.noise_scale
        )));
    }
    if input.base.octaves == 0 {
        return Err(TextureDataLoadError::new(format!(
            "texture `{}` face `{}` has invalid `base.octaves`: 0 (must be >= 1)",
            block_name,
            face_slot.as_str()
        )));
    }
    if !input.base.warp_strength.is_finite() || input.base.warp_strength < 0.0 {
        return Err(TextureDataLoadError::new(format!(
            "texture `{}` face `{}` has invalid `base.warp_strength`: {} (must be >= 0)",
            block_name,
            face_slot.as_str(),
            input.base.warp_strength
        )));
    }

    let base_path = format!(
        "texture `{}` face `{}` base.palette",
        block_name,
        face_slot.as_str()
    );
    let base_palette = encode_palette(&input.base.palette, &base_path)?;

    let (top_palette, layer_ratio) = if input.has_layer {
        let Some(layer_ratio) = input.layer_ratio else {
            return Err(TextureDataLoadError::new(format!(
                "texture `{}` face `{}` has_layer=true but `layer_ratio` is missing",
                block_name,
                face_slot.as_str()
            )));
        };
        if !layer_ratio.is_finite() || !(0.0..=1.0).contains(&layer_ratio) {
            return Err(TextureDataLoadError::new(format!(
                "texture `{}` face `{}` has invalid `layer_ratio`: {} (must be within 0..1)",
                block_name,
                face_slot.as_str(),
                layer_ratio
            )));
        }
        let Some(top_colors) = input.top_layer_palette.as_ref() else {
            return Err(TextureDataLoadError::new(format!(
                "texture `{}` face `{}` has_layer=true but `top_layer_palette` is missing",
                block_name,
                face_slot.as_str()
            )));
        };
        let top_path = format!(
            "texture `{}` face `{}` top_layer_palette",
            block_name,
            face_slot.as_str()
        );
        let top_palette = encode_palette(top_colors, &top_path)?;
        (top_palette, layer_ratio)
    } else {
        if input.layer_ratio.is_some() || input.top_layer_palette.is_some() {
            log::warn!(
                "texture `{}` face `{}` has_layer=false，但提供了 layer 字段；这些字段将被忽略",
                block_name,
                face_slot.as_str()
            );
        }
        (Vec::new(), 0.0)
    };

    Ok(FaceLayerSpec {
        style: block_style,
        size: block_size.max(1),
        seed: block_seed ^ face_slot.seed_salt(),
        octaves: input.base.octaves.max(1),
        noise_scale: input.base.noise_scale,
        warp_strength: input.base.warp_strength,
        has_layer: input.has_layer,
        layer_ratio,
        base_palette,
        top_palette,
    })
}

fn encode_palette(
    palette: &[[u8; 3]],
    field_path: &str,
) -> Result<Vec<[u8; 3]>, TextureDataLoadError> {
    if palette.len() < 2 {
        return Err(TextureDataLoadError::new(format!(
            "{field_path} must contain at least 2 colors"
        )));
    }
    u32::try_from(palette.len())
        .map_err(|_| TextureDataLoadError::new(format!("{field_path} color count overflow")))?;
    Ok(palette.to_vec())
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
        let specs: Vec<TextureSpecInput> =
            serde_json::from_slice(&bytes).map_err(|e| TextureDataLoadError {
                message: e.to_string(),
            })?;
        build_texture_data(specs)
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
struct ProceduralTextureParamsBuffer(UniformBuffer<ProceduralTextureArrayParams>);

#[derive(Resource)]
struct ProceduralTexturePaletteBuffer(StorageBuffer<ProceduralTexturePaletteStorage>);

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
                storage_buffer_read_only_sized(false, None),
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

fn prepare_procedural_texture_palette_buffer(
    mut commands: Commands,
    palettes: Option<Res<ProceduralTexturePaletteStorage>>,
    render_device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    existing: Option<Res<ProceduralTexturePaletteBuffer>>,
) {
    if existing.is_some() {
        return;
    }

    let Some(palettes) = palettes else {
        return;
    };

    let mut storage_buffer = StorageBuffer::from(palettes.into_inner().clone());
    storage_buffer.set_label(Some("procedural_texture_palettes"));
    storage_buffer.write_buffer(&render_device, &queue);
    commands.insert_resource(ProceduralTexturePaletteBuffer(storage_buffer));
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
        let palette_buffer = world.resource::<ProceduralTexturePaletteBuffer>();
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
                &BindGroupEntries::sequential((&mip_view, &params_buffer.0, &palette_buffer.0)),
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
    let Some(palette_buffer) = world.get_resource::<ProceduralTexturePaletteBuffer>() else {
        return false;
    };
    if palette_buffer.0.buffer().is_none() {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn layer_spec(
        style: TextureStyle,
        base_palette: Vec<[u8; 3]>,
        top_palette: Vec<[u8; 3]>,
        has_layer: bool,
    ) -> ExpandedLayerSpec {
        ExpandedLayerSpec {
            _block_name: "test_block".to_string(),
            _face_slot: FaceSlot::All,
            layer: FaceLayerSpec {
                style,
                size: CANONICAL_TEXTURE_SIZE,
                seed: 7,
                octaves: 4,
                noise_scale: 1.0,
                warp_strength: 0.0,
                has_layer,
                layer_ratio: 0.5,
                base_palette,
                top_palette,
            },
        }
    }

    #[test]
    fn encode_palette_accepts_more_than_four_colors() {
        let palette = vec![
            [10, 10, 10],
            [30, 30, 30],
            [50, 50, 50],
            [70, 70, 70],
            [90, 90, 90],
            [110, 110, 110],
        ];

        let encoded = encode_palette(&palette, "test.palette").expect("palette should be valid");
        assert_eq!(encoded.len(), 6);
        assert_eq!(encoded, palette);
    }

    #[test]
    fn from_layers_builds_palette_offsets_and_lengths() {
        let layers = vec![
            layer_spec(
                TextureStyle::HdPixelArt,
                vec![[10, 20, 30], [40, 50, 60], [70, 80, 90]],
                vec![[90, 90, 90], [120, 120, 120]],
                true,
            ),
            layer_spec(
                TextureStyle::HdRealistic,
                vec![[1, 1, 1], [2, 2, 2], [3, 3, 3], [4, 4, 4]],
                Vec::new(),
                false,
            ),
        ];

        let (params, palettes) = ProceduralTextureArrayParams::from_layers(&layers);
        assert_eq!(params.layer_count, 2);

        let l0 = &params.layers[0];
        assert_eq!(l0.style, TextureStyle::HdPixelArt.shader_style_tag());
        assert_eq!(l0.base_palette_offset, 0);
        assert_eq!(l0.base_palette_len, 3);
        assert_eq!(l0.top_palette_offset, 3);
        assert_eq!(l0.top_palette_len, 2);

        let l1 = &params.layers[1];
        assert_eq!(l1.style, TextureStyle::HdRealistic.shader_style_tag());
        assert_eq!(l1.base_palette_offset, 5);
        assert_eq!(l1.base_palette_len, 4);
        assert_eq!(l1.top_palette_offset, 0);
        assert_eq!(l1.top_palette_len, 0);

        assert_eq!(palettes.colors.len(), 9);
    }

    #[test]
    fn array_params_uniform_layout_is_valid() {
        ProceduralTextureArrayParams::assert_uniform_compat();
    }
}
