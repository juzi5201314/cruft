use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use bevy::image::ImageSamplerDescriptor;
use bevy::prelude::*;
use bevy::render::extract_resource::ExtractResource;
use serde::de::IntoDeserializer;
use serde::{Deserialize, Serialize};

use crate::error::TextureDataError;
use crate::schema::{
    RawBlendSpec, RawColorFieldSpec, RawColorMappingSpec, RawCurveSpec, RawFaceBinding,
    RawFaceBindingObject, RawLayer, RawLayerFields, RawMaskJitter, RawMaskSpec, RawNoiseSpec,
    RawNormalSpec, RawOutput, RawPalette, RawPaletteItem, RawSamplerSpec, RawScalarFieldSpec,
    RawScalarRemapSpec, RawSignalSpec, RawSurface, RawSurfaceDefaults, RawTexture,
    RawTextureDefaults, RawTextureFaces, RawTextureSet, StrictValue,
};

const SPEC_ID: &str = "cruft.procedural_texture";
const SPEC_VERSION: &str = "1.0.0";
const PROFILE_ID: &str = "voxel_cube_pbr";
const COMPILER_ALGORITHM_VERSION: &str = "cruft.proc_textures.compiler.v2";
const MAX_SURFACES: usize = 4096;
const MAX_TEXTURES: usize = 4096;
const MAX_SIGNALS: usize = 64;
const MAX_LAYERS: usize = 16;
const MAX_MASK_DEPTH: usize = 8;
const MAX_PALETTE_STOPS: usize = 64;
const MAX_OUTPUT_SIZE: u32 = 1024;
const MAX_OCTAVES: u32 = 8;
const MAX_ANISOTROPY: u8 = 16;
const DEFAULT_SURFACE_LOGICAL_SIZE: u32 = 16;
const DEFAULT_SURFACE_PIXEL_SNAP: bool = true;
const DEFAULT_SURFACE_DOMAIN: SurfaceDomain = SurfaceDomain::FaceUv;
const DEFAULT_SURFACE_TILE_MODE: TileMode = TileMode::Repeat;
const DEFAULT_SURFACE_SEED: u32 = 0;
const DEFAULT_NORMAL_SPEC: NormalSpec = NormalSpec {
    mode: NormalMode::Flat,
    strength: 1.0,
};
const DEFAULT_TEXTURE_ALPHA_MODE: AlphaMode = AlphaMode::Opaque;
const DEFAULT_CUTOUT_THRESHOLD: f32 = 0.5;
const DEFAULT_SAMPLER_SPEC: TextureSamplerSpec = TextureSamplerSpec {
    mag_filter: FilterModeSpec::Nearest,
    min_filter: FilterModeSpec::Nearest,
    mipmap_filter: MipmapFilterModeSpec::Nearest,
    anisotropy: 1,
    address_u: AddressModeSpec::ClampToEdge,
    address_v: AddressModeSpec::ClampToEdge,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CubeFace {
    Top,
    Bottom,
    North,
    South,
    East,
    West,
}

impl CubeFace {
    pub const ALL: [Self; 6] = [
        Self::Top,
        Self::Bottom,
        Self::North,
        Self::South,
        Self::East,
        Self::West,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::North => "north",
            Self::South => "south",
            Self::East => "east",
            Self::West => "west",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SurfaceDomain {
    FaceUv,
    BlockSpace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TileMode {
    Repeat,
    Clamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlphaMode {
    Opaque = 0,
    Mask = 1,
    Blend = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NormalMode {
    Flat,
    DeriveFromHeight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NormalFormat {
    OpenGl,
    DirectX,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MipmapMode {
    None,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterModeSpec {
    Nearest,
    Linear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MipmapFilterModeSpec {
    None,
    Nearest,
    Linear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AddressModeSpec {
    ClampToEdge,
    Repeat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorMappingMode {
    Quantized,
    Gradient,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DitherMode {
    None,
    Bayer4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CurveInterpolation {
    Linear,
    Smoothstep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoiseBasis {
    Value,
    Gradient,
    Cellular,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoiseFractal {
    None,
    Fbm,
    Billow,
    Ridged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelPackKind {
    Albedo,
    Normal,
    Orm,
    Emissive,
    Height,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorChannelKind {
    Albedo,
    Emissive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalOp {
    Add,
    Multiply,
    Min,
    Max,
    Subtract,
    Average,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScalarBlendMode {
    Mix,
    Add,
    Multiply,
    Max,
    Min,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorBlendMode {
    Mix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaskAxis {
    U,
    V,
    X,
    Y,
    Z,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaletteStop {
    pub at: f32,
    pub color: [f32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NormalSpec {
    pub mode: NormalMode,
    pub strength: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct FaceTransform {
    pub rotate: u16,
    pub flip_u: bool,
    pub flip_v: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextureFingerprint(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TexturePackId(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelLayerRef {
    pub pack_id: TexturePackId,
    pub layer_index: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaceChannels {
    pub albedo: ChannelLayerRef,
    pub normal: ChannelLayerRef,
    pub orm: ChannelLayerRef,
    pub emissive: ChannelLayerRef,
    pub height: ChannelLayerRef,
}

pub type ResolvedFace = FaceChannels;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedTexture {
    pub sampler: TextureSamplerSpec,
    pub alpha_mode: AlphaMode,
    pub cutout_threshold: f32,
    pub top: ResolvedFace,
    pub bottom: ResolvedFace,
    pub north: ResolvedFace,
    pub south: ResolvedFace,
    pub east: ResolvedFace,
    pub west: ResolvedFace,
}

impl ResolvedTexture {
    pub fn face(&self, face: CubeFace) -> &ResolvedFace {
        match face {
            CubeFace::Top => &self.top,
            CubeFace::Bottom => &self.bottom,
            CubeFace::North => &self.north,
            CubeFace::South => &self.south,
            CubeFace::East => &self.east,
            CubeFace::West => &self.west,
        }
    }
}

#[derive(Resource, Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TextureRegistry {
    pub textures: BTreeMap<String, ResolvedTexture>,
    pub fingerprint: Option<TextureFingerprint>,
}

impl TextureRegistry {
    pub fn get(&self, name: &str) -> Option<&ResolvedTexture> {
        self.textures.get(name)
    }
}

#[derive(Resource, Debug, Clone, Default, PartialEq, ExtractResource)]
pub struct TextureRuntimePacks {
    pub packs: Vec<TextureRuntimePack>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextureRuntimePack {
    pub id: TexturePackId,
    pub layer_count: u32,
    pub mip_level_count: u32,
    pub albedo: Handle<Image>,
    pub normal: Handle<Image>,
    pub orm: Handle<Image>,
    pub emissive: Handle<Image>,
    pub height: Handle<Image>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanonicalTextureSet {
    pub json: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompiledTextureSet {
    pub source_path: PathBuf,
    pub spec: String,
    pub spec_version: String,
    pub profile: String,
    pub meta: BTreeMap<String, serde_json::Value>,
    pub output: OutputSpec,
    pub surfaces: BTreeMap<String, CompiledSurface>,
    pub textures: BTreeMap<String, CompiledTexture>,
    pub extensions_used: Vec<String>,
    pub extensions: BTreeMap<String, serde_json::Value>,
    pub canonical: CanonicalTextureSet,
    pub fingerprint: TextureFingerprint,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputSpec {
    pub size: u32,
    pub mipmaps: MipmapMode,
    pub normal_format: NormalFormat,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompiledSurface {
    pub logical_size: u32,
    pub pixel_snap: bool,
    pub domain: SurfaceDomain,
    pub tile_mode: TileMode,
    pub seed: u32,
    pub signals: BTreeMap<String, CompiledSignal>,
    pub signal_order: Vec<String>,
    pub base: CompiledLayerFields,
    pub layers: Vec<CompiledLayer>,
    pub normal: NormalSpec,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompiledTexture {
    pub sampler: TextureSamplerSpec,
    pub alpha_mode: AlphaMode,
    pub cutout_threshold: f32,
    pub faces: BTreeMap<CubeFace, CompiledFaceBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CompiledFaceBinding {
    pub surface: String,
    pub transform: FaceTransform,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TextureSamplerSpec {
    pub mag_filter: FilterModeSpec,
    pub min_filter: FilterModeSpec,
    pub mipmap_filter: MipmapFilterModeSpec,
    pub anisotropy: u8,
    pub address_u: AddressModeSpec,
    pub address_v: AddressModeSpec,
}

impl TextureSamplerSpec {
    pub fn to_image_sampler_descriptor(self) -> ImageSamplerDescriptor {
        ImageSamplerDescriptor {
            address_mode_u: match self.address_u {
                AddressModeSpec::ClampToEdge => bevy::image::ImageAddressMode::ClampToEdge,
                AddressModeSpec::Repeat => bevy::image::ImageAddressMode::Repeat,
            },
            address_mode_v: match self.address_v {
                AddressModeSpec::ClampToEdge => bevy::image::ImageAddressMode::ClampToEdge,
                AddressModeSpec::Repeat => bevy::image::ImageAddressMode::Repeat,
            },
            mag_filter: match self.mag_filter {
                FilterModeSpec::Nearest => bevy::image::ImageFilterMode::Nearest,
                FilterModeSpec::Linear => bevy::image::ImageFilterMode::Linear,
            },
            min_filter: match self.min_filter {
                FilterModeSpec::Nearest => bevy::image::ImageFilterMode::Nearest,
                FilterModeSpec::Linear => bevy::image::ImageFilterMode::Linear,
            },
            mipmap_filter: match self.mipmap_filter {
                MipmapFilterModeSpec::None | MipmapFilterModeSpec::Nearest => {
                    bevy::image::ImageFilterMode::Nearest
                }
                MipmapFilterModeSpec::Linear => bevy::image::ImageFilterMode::Linear,
            },
            anisotropy_clamp: u16::from(self.anisotropy),
            ..default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompiledSignal {
    pub kind: CompiledSignalKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CompiledSignalKind {
    Constant {
        value: f32,
    },
    Noise {
        noise: NoiseSpec,
        remap: ScalarRemapSpec,
    },
    Curve {
        source: String,
        curve: CurveSpec,
        clamp: [f32; 2],
    },
    Combine {
        op: SignalOp,
        inputs: Vec<String>,
        clamp: [f32; 2],
    },
    Mask {
        mask: MaskSpec,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompiledLayerFields {
    pub albedo: CompiledColorField,
    pub height: Option<CompiledScalarField>,
    pub roughness: Option<CompiledScalarField>,
    pub ao: Option<CompiledScalarField>,
    pub metallic: Option<CompiledScalarField>,
    pub emissive: Option<CompiledColorField>,
    pub opacity: Option<CompiledScalarField>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompiledLayer {
    pub name: Option<String>,
    pub mask: MaskSpec,
    pub strength: f32,
    pub fields: PartialLayerFields,
    pub blend: LayerBlendSpec,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartialLayerFields {
    pub albedo: Option<CompiledColorField>,
    pub height: Option<CompiledScalarField>,
    pub roughness: Option<CompiledScalarField>,
    pub ao: Option<CompiledScalarField>,
    pub metallic: Option<CompiledScalarField>,
    pub emissive: Option<CompiledColorField>,
    pub opacity: Option<CompiledScalarField>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerBlendSpec {
    pub albedo: ColorBlendMode,
    pub height: ScalarBlendMode,
    pub roughness: ScalarBlendMode,
    pub ao: ScalarBlendMode,
    pub metallic: ScalarBlendMode,
    pub emissive: ColorBlendMode,
    pub opacity: ScalarBlendMode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompiledColorField {
    pub source: ColorFieldSource,
    pub mapping: ColorMappingSpec,
    pub palette: Option<Vec<PaletteStop>>,
    pub constant: Option<[f32; 3]>,
    pub intensity: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ColorFieldSource {
    Constant,
    Signal(String),
    InlineNoise(NoiseSpec),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompiledScalarField {
    pub source: ScalarFieldSource,
    pub remap: Option<ScalarRemapSpec>,
    pub constant: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ScalarFieldSource {
    Constant,
    Signal(String),
    InlineNoise(NoiseSpec),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColorMappingSpec {
    pub mode: ColorMappingMode,
    pub levels: u32,
    pub contrast: f32,
    pub bias: f32,
    pub invert: bool,
    pub dither: DitherMode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScalarRemapSpec {
    pub range: [f32; 2],
    pub contrast: f32,
    pub bias: f32,
    pub invert: bool,
    pub curve: CurveSpec,
    pub clamp: [f32; 2],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurveSpec {
    pub interp: CurveInterpolation,
    pub stops: Vec<[f32; 2]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoiseSpec {
    pub basis: NoiseBasis,
    pub fractal: NoiseFractal,
    pub scale: f32,
    pub stretch: [f32; 3],
    pub octaves: u32,
    pub lacunarity: f32,
    pub gain: f32,
    pub offset: [f32; 3],
    pub seed_offset: u32,
    pub cellular_return: Option<String>,
    pub warp: WarpSpec,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WarpSpec {
    pub amplitude: f32,
    pub basis: NoiseBasis,
    pub fractal: NoiseFractal,
    pub scale_multiplier: f32,
    pub octaves: u32,
    pub lacunarity: f32,
    pub gain: f32,
    pub seed_offset: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MaskSpec {
    Full,
    Signal {
        source: String,
    },
    AxisBand {
        axis: MaskAxis,
        from: f32,
        to: f32,
        falloff: f32,
        invert: bool,
        jitter: Option<MaskJitterSpec>,
    },
    Threshold {
        source: ThresholdSource,
        threshold: f32,
        softness: f32,
        invert: bool,
    },
    EdgeDistance {
        from: f32,
        to: f32,
        falloff: f32,
        invert: bool,
    },
    And {
        items: Vec<MaskSpec>,
    },
    Or {
        items: Vec<MaskSpec>,
    },
    Subtract {
        items: Vec<MaskSpec>,
    },
    Not {
        item: Box<MaskSpec>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaskJitterSpec {
    pub amount: f32,
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ThresholdSource {
    Signal(String),
    InlineNoise(NoiseSpec),
}

fn default_curve() -> CurveSpec {
    CurveSpec {
        interp: CurveInterpolation::Linear,
        stops: vec![[0.0, 0.0], [1.0, 1.0]],
    }
}

fn default_color_mapping() -> ColorMappingSpec {
    ColorMappingSpec {
        mode: ColorMappingMode::Quantized,
        levels: 0,
        contrast: 1.0,
        bias: 0.0,
        invert: false,
        dither: DitherMode::None,
    }
}

fn identity_scalar_remap() -> ScalarRemapSpec {
    ScalarRemapSpec {
        range: [0.0, 1.0],
        contrast: 1.0,
        bias: 0.0,
        invert: false,
        curve: default_curve(),
        clamp: [0.0, 1.0],
    }
}

pub fn load_and_compile_texture_set(
    path: impl AsRef<Path>,
) -> Result<CompiledTextureSet, TextureDataError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TextureDataError::io(path, error.to_string()))?;
    compile_texture_set(path, &bytes)
}

pub fn compile_texture_set(
    path: impl AsRef<Path>,
    bytes: &[u8],
) -> Result<CompiledTextureSet, TextureDataError> {
    let path = path.as_ref();
    let strict: StrictValue = serde_json::from_slice(bytes)
        .map_err(|error| TextureDataError::parse(path, None, error.to_string()))?;
    strict.reject_core_nulls(path, "")?;
    let value = strict.to_serde_value();
    let raw: RawTextureSet =
        serde_path_to_error::deserialize(value.into_deserializer()).map_err(|error| {
            let path_string = error.path().to_string();
            let path_opt = if path_string.is_empty() {
                None
            } else {
                Some(path_string)
            };
            TextureDataError::parse(path, path_opt, error.inner().to_string())
        })?;
    compile_raw_texture_set(path.to_path_buf(), raw)
}

fn compile_raw_texture_set(
    path: PathBuf,
    raw: RawTextureSet,
) -> Result<CompiledTextureSet, TextureDataError> {
    validate_top_level(&path, &raw)?;

    let output = compile_output(&path, &raw.output)?;
    let surface_defaults =
        compile_surface_defaults(&path, "defaults.surface", &raw.defaults.surface)?;
    let texture_defaults =
        compile_texture_defaults(&path, "defaults.texture", &raw.defaults.texture)?;

    let mut surfaces = BTreeMap::new();
    for (name, surface) in &raw.surfaces {
        validate_name(&path, &format!("surfaces.{name}"), name)?;
        surfaces.insert(
            name.clone(),
            compile_surface(&path, name, surface, &surface_defaults, output.size)?,
        );
    }

    let mut textures = BTreeMap::new();
    for (name, texture) in &raw.textures {
        validate_name(&path, &format!("textures.{name}"), name)?;
        textures.insert(
            name.clone(),
            compile_texture(&path, name, texture, &texture_defaults, &surfaces)?,
        );
    }

    let canonical_surfaces = canonicalize_surfaces(&path, &surfaces)?;
    let canonical_value = serde_json::to_value(CanonicalTextureSetData {
        spec: raw.spec.clone(),
        spec_version: raw.spec_version.clone(),
        profile: raw.profile.clone(),
        output: output.clone(),
        surfaces: canonical_surfaces,
        textures: textures.clone(),
        extensions_used: raw.extensions_used.clone(),
        extensions: raw.extensions.clone(),
    })
    .map_err(|error| TextureDataError::validate(&path, "canonical", error.to_string()))?;
    let canonical_json = serde_json::to_string(&canonical_value)
        .map_err(|error| TextureDataError::validate(&path, "canonical", error.to_string()))?;
    let fingerprint = {
        let mut hasher = blake3::Hasher::new();
        hasher.update(canonical_json.as_bytes());
        hasher.update(SPEC_ID.as_bytes());
        hasher.update(SPEC_VERSION.as_bytes());
        hasher.update(PROFILE_ID.as_bytes());
        hasher.update(COMPILER_ALGORITHM_VERSION.as_bytes());
        TextureFingerprint(hasher.finalize().to_hex().to_string())
    };

    Ok(CompiledTextureSet {
        source_path: path,
        spec: raw.spec,
        spec_version: raw.spec_version,
        profile: raw.profile,
        meta: raw.meta,
        output,
        surfaces,
        textures,
        extensions_used: raw.extensions_used,
        extensions: raw.extensions,
        canonical: CanonicalTextureSet {
            json: canonical_json,
        },
        fingerprint,
    })
}

#[derive(Debug, Clone, Serialize)]
struct CanonicalTextureSetData {
    spec: String,
    spec_version: String,
    profile: String,
    output: OutputSpec,
    surfaces: BTreeMap<String, CompiledSurface>,
    textures: BTreeMap<String, CompiledTexture>,
    extensions_used: Vec<String>,
    extensions: BTreeMap<String, serde_json::Value>,
}

fn validate_top_level(path: &Path, raw: &RawTextureSet) -> Result<(), TextureDataError> {
    if raw.spec != SPEC_ID {
        return Err(TextureDataError::validate(
            path,
            "spec",
            format!("expected `{SPEC_ID}`"),
        ));
    }
    if raw.spec_version != SPEC_VERSION {
        return Err(TextureDataError::validate(
            path,
            "spec_version",
            format!("expected `{SPEC_VERSION}`"),
        ));
    }
    if raw.profile != PROFILE_ID {
        return Err(TextureDataError::validate(
            path,
            "profile",
            format!("expected `{PROFILE_ID}`"),
        ));
    }
    if raw.surfaces.is_empty() {
        return Err(TextureDataError::validate(
            path,
            "surfaces",
            "must contain at least one surface",
        ));
    }
    if raw.textures.is_empty() {
        return Err(TextureDataError::validate(
            path,
            "textures",
            "must contain at least one texture",
        ));
    }
    if raw.surfaces.len() > MAX_SURFACES {
        return Err(TextureDataError::validate(
            path,
            "surfaces",
            format!("surface count exceeds {MAX_SURFACES}"),
        ));
    }
    if raw.textures.len() > MAX_TEXTURES {
        return Err(TextureDataError::validate(
            path,
            "textures",
            format!("texture count exceeds {MAX_TEXTURES}"),
        ));
    }
    if !raw.extensions_used.is_empty() {
        return Err(TextureDataError::validate(
            path,
            "extensions_used",
            "extensions are not supported by this implementation",
        ));
    }
    if !raw.extensions.is_empty() {
        return Err(TextureDataError::validate(
            path,
            "extensions",
            "extensions payloads are not supported by this implementation",
        ));
    }
    Ok(())
}

fn canonicalize_surfaces(
    path: &Path,
    surfaces: &BTreeMap<String, CompiledSurface>,
) -> Result<BTreeMap<String, CompiledSurface>, TextureDataError> {
    let mut canonical = BTreeMap::new();
    for (name, surface) in surfaces {
        canonical.insert(name.clone(), canonicalize_surface(path, name, surface)?);
    }
    Ok(canonical)
}

fn canonicalize_surface(
    path: &Path,
    name: &str,
    surface: &CompiledSurface,
) -> Result<CompiledSurface, TextureDataError> {
    let mut surface = surface.clone();
    let mut generated_signals = Vec::new();
    let mut used_names = surface.signals.keys().cloned().collect::<BTreeSet<_>>();
    let mut inline_index = 0u32;

    for signal in surface.signals.values_mut() {
        expand_signal_inline_noise(
            signal,
            &mut generated_signals,
            &mut used_names,
            &mut inline_index,
        );
    }
    expand_layer_fields_inline_noise(
        &mut surface.base,
        &mut generated_signals,
        &mut used_names,
        &mut inline_index,
    );
    for layer in &mut surface.layers {
        expand_mask_inline_noise(
            &mut layer.mask,
            &mut generated_signals,
            &mut used_names,
            &mut inline_index,
        );
        expand_partial_layer_fields_inline_noise(
            &mut layer.fields,
            &mut generated_signals,
            &mut used_names,
            &mut inline_index,
        );
    }

    for (signal_name, signal) in generated_signals {
        surface.signals.insert(signal_name, signal);
    }
    surface.signal_order = topo_sort_signals(path, name, &surface.signals)?;
    Ok(surface)
}

fn expand_signal_inline_noise(
    signal: &mut CompiledSignal,
    generated_signals: &mut Vec<(String, CompiledSignal)>,
    used_names: &mut BTreeSet<String>,
    inline_index: &mut u32,
) {
    if let CompiledSignalKind::Mask { mask } = &mut signal.kind {
        expand_mask_inline_noise(mask, generated_signals, used_names, inline_index);
    }
}

fn expand_layer_fields_inline_noise(
    fields: &mut CompiledLayerFields,
    generated_signals: &mut Vec<(String, CompiledSignal)>,
    used_names: &mut BTreeSet<String>,
    inline_index: &mut u32,
) {
    expand_color_field_inline_noise(
        &mut fields.albedo,
        generated_signals,
        used_names,
        inline_index,
    );
    if let Some(field) = &mut fields.height {
        expand_scalar_field_inline_noise(field, generated_signals, used_names, inline_index);
    }
    if let Some(field) = &mut fields.roughness {
        expand_scalar_field_inline_noise(field, generated_signals, used_names, inline_index);
    }
    if let Some(field) = &mut fields.ao {
        expand_scalar_field_inline_noise(field, generated_signals, used_names, inline_index);
    }
    if let Some(field) = &mut fields.metallic {
        expand_scalar_field_inline_noise(field, generated_signals, used_names, inline_index);
    }
    if let Some(field) = &mut fields.emissive {
        expand_color_field_inline_noise(field, generated_signals, used_names, inline_index);
    }
    if let Some(field) = &mut fields.opacity {
        expand_scalar_field_inline_noise(field, generated_signals, used_names, inline_index);
    }
}

fn expand_partial_layer_fields_inline_noise(
    fields: &mut PartialLayerFields,
    generated_signals: &mut Vec<(String, CompiledSignal)>,
    used_names: &mut BTreeSet<String>,
    inline_index: &mut u32,
) {
    if let Some(field) = &mut fields.albedo {
        expand_color_field_inline_noise(field, generated_signals, used_names, inline_index);
    }
    if let Some(field) = &mut fields.height {
        expand_scalar_field_inline_noise(field, generated_signals, used_names, inline_index);
    }
    if let Some(field) = &mut fields.roughness {
        expand_scalar_field_inline_noise(field, generated_signals, used_names, inline_index);
    }
    if let Some(field) = &mut fields.ao {
        expand_scalar_field_inline_noise(field, generated_signals, used_names, inline_index);
    }
    if let Some(field) = &mut fields.metallic {
        expand_scalar_field_inline_noise(field, generated_signals, used_names, inline_index);
    }
    if let Some(field) = &mut fields.emissive {
        expand_color_field_inline_noise(field, generated_signals, used_names, inline_index);
    }
    if let Some(field) = &mut fields.opacity {
        expand_scalar_field_inline_noise(field, generated_signals, used_names, inline_index);
    }
}

fn expand_color_field_inline_noise(
    field: &mut CompiledColorField,
    generated_signals: &mut Vec<(String, CompiledSignal)>,
    used_names: &mut BTreeSet<String>,
    inline_index: &mut u32,
) {
    if let ColorFieldSource::InlineNoise(noise) = &field.source {
        let signal_name = next_inline_signal_name(used_names, inline_index);
        generated_signals.push((signal_name.clone(), inline_noise_signal(noise.clone())));
        field.source = ColorFieldSource::Signal(signal_name);
    }
}

fn expand_scalar_field_inline_noise(
    field: &mut CompiledScalarField,
    generated_signals: &mut Vec<(String, CompiledSignal)>,
    used_names: &mut BTreeSet<String>,
    inline_index: &mut u32,
) {
    if let ScalarFieldSource::InlineNoise(noise) = &field.source {
        let signal_name = next_inline_signal_name(used_names, inline_index);
        generated_signals.push((signal_name.clone(), inline_noise_signal(noise.clone())));
        field.source = ScalarFieldSource::Signal(signal_name);
    }
}

fn expand_mask_inline_noise(
    mask: &mut MaskSpec,
    generated_signals: &mut Vec<(String, CompiledSignal)>,
    used_names: &mut BTreeSet<String>,
    inline_index: &mut u32,
) {
    match mask {
        MaskSpec::Threshold { source, .. } => {
            if let ThresholdSource::InlineNoise(noise) = source {
                let signal_name = next_inline_signal_name(used_names, inline_index);
                generated_signals.push((signal_name.clone(), inline_noise_signal(noise.clone())));
                *source = ThresholdSource::Signal(signal_name);
            }
        }
        MaskSpec::And { items } | MaskSpec::Or { items } | MaskSpec::Subtract { items } => {
            for item in items {
                expand_mask_inline_noise(item, generated_signals, used_names, inline_index);
            }
        }
        MaskSpec::Not { item } => {
            expand_mask_inline_noise(item, generated_signals, used_names, inline_index);
        }
        MaskSpec::Full
        | MaskSpec::Signal { .. }
        | MaskSpec::AxisBand { .. }
        | MaskSpec::EdgeDistance { .. } => {}
    }
}

fn next_inline_signal_name(used_names: &mut BTreeSet<String>, inline_index: &mut u32) -> String {
    loop {
        *inline_index += 1;
        let candidate = format!("__inline_noise_{:04}", *inline_index);
        if used_names.insert(candidate.clone()) {
            return candidate;
        }
    }
}

fn inline_noise_signal(noise: NoiseSpec) -> CompiledSignal {
    CompiledSignal {
        kind: CompiledSignalKind::Noise {
            noise,
            remap: identity_scalar_remap(),
        },
    }
}

fn compile_output(path: &Path, raw: &RawOutput) -> Result<OutputSpec, TextureDataError> {
    if raw.size < 16 || raw.size > MAX_OUTPUT_SIZE || !raw.size.is_power_of_two() {
        return Err(TextureDataError::validate(
            path,
            "output.size",
            "must be within 16..1024 and be a power of two",
        ));
    }
    let mipmaps = match raw.mipmaps.as_deref().unwrap_or("full") {
        "none" => MipmapMode::None,
        "full" => MipmapMode::Full,
        _ => {
            return Err(TextureDataError::validate(
                path,
                "output.mipmaps",
                "must be `none` or `full`",
            ))
        }
    };
    let normal_format = match raw.normal_format.as_deref().unwrap_or("opengl") {
        "opengl" => NormalFormat::OpenGl,
        "directx" => NormalFormat::DirectX,
        _ => {
            return Err(TextureDataError::validate(
                path,
                "output.normal_format",
                "must be `opengl` or `directx`",
            ))
        }
    };
    Ok(OutputSpec {
        size: raw.size,
        mipmaps,
        normal_format,
    })
}

#[derive(Debug, Clone, Copy)]
struct CompiledSurfaceDefaults {
    logical_size: u32,
    pixel_snap: bool,
    domain: SurfaceDomain,
    tile_mode: TileMode,
    seed: u32,
    normal: NormalSpec,
}

fn compile_surface_defaults(
    path: &Path,
    base_path: &str,
    raw: &RawSurfaceDefaults,
) -> Result<CompiledSurfaceDefaults, TextureDataError> {
    Ok(CompiledSurfaceDefaults {
        logical_size: raw.logical_size.unwrap_or(DEFAULT_SURFACE_LOGICAL_SIZE),
        pixel_snap: raw.pixel_snap.unwrap_or(DEFAULT_SURFACE_PIXEL_SNAP),
        domain: parse_domain(path, &format!("{base_path}.domain"), raw.domain.as_deref())?
            .unwrap_or(DEFAULT_SURFACE_DOMAIN),
        tile_mode: parse_tile_mode(
            path,
            &format!("{base_path}.tile_mode"),
            raw.tile_mode.as_deref(),
        )?
        .unwrap_or(DEFAULT_SURFACE_TILE_MODE),
        seed: raw.seed.unwrap_or(DEFAULT_SURFACE_SEED),
        normal: compile_normal_spec(path, &format!("{base_path}.normal"), raw.normal.as_ref())?
            .unwrap_or(DEFAULT_NORMAL_SPEC),
    })
}

#[derive(Debug, Clone, Copy)]
struct CompiledTextureDefaults {
    sampler: TextureSamplerSpec,
    alpha_mode: AlphaMode,
    cutout_threshold: f32,
}

fn compile_texture_defaults(
    path: &Path,
    base_path: &str,
    raw: &RawTextureDefaults,
) -> Result<CompiledTextureDefaults, TextureDataError> {
    Ok(CompiledTextureDefaults {
        sampler: compile_sampler_spec(path, &format!("{base_path}.sampler"), raw.sampler.as_ref())?
            .unwrap_or(DEFAULT_SAMPLER_SPEC),
        alpha_mode: parse_alpha_mode(
            path,
            &format!("{base_path}.alpha_mode"),
            raw.alpha_mode.as_deref(),
        )?
        .unwrap_or(DEFAULT_TEXTURE_ALPHA_MODE),
        cutout_threshold: validate_unit_interval(
            path,
            &format!("{base_path}.cutout_threshold"),
            raw.cutout_threshold.unwrap_or(DEFAULT_CUTOUT_THRESHOLD),
        )?,
    })
}

fn compile_surface(
    path: &Path,
    name: &str,
    raw: &RawSurface,
    defaults: &CompiledSurfaceDefaults,
    output_size: u32,
) -> Result<CompiledSurface, TextureDataError> {
    let logical_size = raw.logical_size.unwrap_or(defaults.logical_size);
    if logical_size < 4 || logical_size > output_size {
        return Err(TextureDataError::validate(
            path,
            format!("surfaces.{name}.logical_size"),
            "must be within 4..output.size",
        ));
    }
    let pixel_snap = raw.pixel_snap.unwrap_or(defaults.pixel_snap);
    if pixel_snap && !output_size.is_multiple_of(logical_size) {
        return Err(TextureDataError::validate(
            path,
            format!("surfaces.{name}.pixel_snap"),
            "requires output.size % logical_size == 0",
        ));
    }
    if raw.layers.len() > MAX_LAYERS {
        return Err(TextureDataError::validate(
            path,
            format!("surfaces.{name}.layers"),
            format!("layer count exceeds {MAX_LAYERS}"),
        ));
    }

    let domain = parse_domain(
        path,
        &format!("surfaces.{name}.domain"),
        raw.domain.as_deref(),
    )?
    .unwrap_or(defaults.domain);
    let tile_mode = parse_tile_mode(
        path,
        &format!("surfaces.{name}.tile_mode"),
        raw.tile_mode.as_deref(),
    )?
    .unwrap_or(defaults.tile_mode);
    let normal = compile_normal_spec(
        path,
        &format!("surfaces.{name}.normal"),
        raw.normal.as_ref(),
    )?
    .unwrap_or(defaults.normal);

    let mut signals = BTreeMap::new();
    for (signal_name, signal) in &raw.signals {
        validate_name(
            path,
            &format!("surfaces.{name}.signals.{signal_name}"),
            signal_name,
        )?;
        signals.insert(
            signal_name.clone(),
            compile_signal(
                path,
                &format!("surfaces.{name}.signals.{signal_name}"),
                signal,
            )?,
        );
    }
    if signals.len() > MAX_SIGNALS {
        return Err(TextureDataError::validate(
            path,
            format!("surfaces.{name}.signals"),
            format!("signal count exceeds {MAX_SIGNALS}"),
        ));
    }
    let signal_order = topo_sort_signals(path, name, &signals)?;

    let base = compile_base_layer(path, &format!("surfaces.{name}.base"), &raw.base)?;
    let mut layers = Vec::with_capacity(raw.layers.len());
    for (index, layer) in raw.layers.iter().enumerate() {
        layers.push(compile_layer(
            path,
            &format!("surfaces.{name}.layers.{index}"),
            layer,
            domain,
            0,
        )?);
    }

    if normal.mode == NormalMode::DeriveFromHeight && !surface_has_effective_height(&base, &layers)
    {
        return Err(TextureDataError::validate(
            path,
            format!("surfaces.{name}.normal"),
            "derive_from_height requires a composed height channel",
        ));
    }

    Ok(CompiledSurface {
        logical_size,
        pixel_snap,
        domain,
        tile_mode,
        seed: raw.seed.unwrap_or(defaults.seed),
        signals,
        signal_order,
        base,
        layers,
        normal,
    })
}

fn surface_has_effective_height(base: &CompiledLayerFields, layers: &[CompiledLayer]) -> bool {
    base.height.is_some()
        || layers
            .iter()
            .any(|layer| layer.strength > 0.0 && layer.fields.height.is_some())
}

fn compile_texture(
    path: &Path,
    name: &str,
    raw: &RawTexture,
    defaults: &CompiledTextureDefaults,
    surfaces: &BTreeMap<String, CompiledSurface>,
) -> Result<CompiledTexture, TextureDataError> {
    let sampler = compile_sampler_spec(
        path,
        &format!("textures.{name}.sampler"),
        raw.sampler.as_ref(),
    )?
    .unwrap_or(defaults.sampler);
    let alpha_mode = parse_alpha_mode(
        path,
        &format!("textures.{name}.alpha_mode"),
        raw.alpha_mode.as_deref(),
    )?
    .unwrap_or(defaults.alpha_mode);
    let cutout_threshold = validate_unit_interval(
        path,
        &format!("textures.{name}.cutout_threshold"),
        raw.cutout_threshold.unwrap_or(defaults.cutout_threshold),
    )?;
    let faces = resolve_texture_faces(path, name, &raw.faces, surfaces)?;
    Ok(CompiledTexture {
        sampler,
        alpha_mode,
        cutout_threshold,
        faces,
    })
}

fn resolve_texture_faces(
    path: &Path,
    texture_name: &str,
    faces: &RawTextureFaces,
    surfaces: &BTreeMap<String, CompiledSurface>,
) -> Result<BTreeMap<CubeFace, CompiledFaceBinding>, TextureDataError> {
    let mut resolved = BTreeMap::new();
    for face in CubeFace::ALL {
        let binding = match face {
            CubeFace::Top => faces.top.as_ref().or(faces.all.as_ref()),
            CubeFace::Bottom => faces.bottom.as_ref().or(faces.all.as_ref()),
            CubeFace::North => faces
                .north
                .as_ref()
                .or(faces.sides.as_ref())
                .or(faces.all.as_ref()),
            CubeFace::South => faces
                .south
                .as_ref()
                .or(faces.sides.as_ref())
                .or(faces.all.as_ref()),
            CubeFace::East => faces
                .east
                .as_ref()
                .or(faces.sides.as_ref())
                .or(faces.all.as_ref()),
            CubeFace::West => faces
                .west
                .as_ref()
                .or(faces.sides.as_ref())
                .or(faces.all.as_ref()),
        };
        let Some(binding) = binding else {
            return Err(TextureDataError::validate(
                path,
                format!("textures.{texture_name}.faces.{}", face.as_str()),
                "could not resolve face binding",
            ));
        };
        let compiled = compile_face_binding(
            path,
            &format!("textures.{texture_name}.faces.{}", face.as_str()),
            binding,
        )?;
        let Some(surface) = surfaces.get(&compiled.surface) else {
            return Err(TextureDataError::validate(
                path,
                format!("textures.{texture_name}.faces.{}.surface", face.as_str()),
                format!("unknown surface `{}`", compiled.surface),
            ));
        };
        if surface.domain == SurfaceDomain::BlockSpace
            && compiled.transform != FaceTransform::default()
        {
            return Err(TextureDataError::validate(
                path,
                format!("textures.{texture_name}.faces.{}", face.as_str()),
                "block_space surface must not use rotate/flip",
            ));
        }
        resolved.insert(face, compiled);
    }
    Ok(resolved)
}

fn compile_face_binding(
    path: &Path,
    base_path: &str,
    raw: &RawFaceBinding,
) -> Result<CompiledFaceBinding, TextureDataError> {
    match raw {
        RawFaceBinding::Name(surface) => Ok(CompiledFaceBinding {
            surface: surface.clone(),
            transform: FaceTransform::default(),
        }),
        RawFaceBinding::Binding(RawFaceBindingObject {
            surface,
            rotate,
            flip_u,
            flip_v,
        }) => {
            let rotate = rotate.unwrap_or(0);
            if !matches!(rotate, 0 | 90 | 180 | 270) {
                return Err(TextureDataError::validate(
                    path,
                    format!("{base_path}.rotate"),
                    "must be one of 0/90/180/270",
                ));
            }
            Ok(CompiledFaceBinding {
                surface: surface.clone(),
                transform: FaceTransform {
                    rotate,
                    flip_u: flip_u.unwrap_or(false),
                    flip_v: flip_v.unwrap_or(false),
                },
            })
        }
    }
}

fn compile_base_layer(
    path: &Path,
    base_path: &str,
    raw: &RawLayerFields,
) -> Result<CompiledLayerFields, TextureDataError> {
    let Some(albedo) = raw.albedo.as_ref() else {
        return Err(TextureDataError::validate(
            path,
            format!("{base_path}.albedo"),
            "base.albedo is required",
        ));
    };
    Ok(CompiledLayerFields {
        albedo: compile_color_field(
            path,
            &format!("{base_path}.albedo"),
            albedo,
            ColorChannelKind::Albedo,
        )?,
        height: raw
            .height
            .as_ref()
            .map(|value| compile_scalar_field(path, &format!("{base_path}.height"), value))
            .transpose()?,
        roughness: raw
            .roughness
            .as_ref()
            .map(|value| compile_scalar_field(path, &format!("{base_path}.roughness"), value))
            .transpose()?,
        ao: raw
            .ao
            .as_ref()
            .map(|value| compile_scalar_field(path, &format!("{base_path}.ao"), value))
            .transpose()?,
        metallic: raw
            .metallic
            .as_ref()
            .map(|value| compile_scalar_field(path, &format!("{base_path}.metallic"), value))
            .transpose()?,
        emissive: raw
            .emissive
            .as_ref()
            .map(|value| {
                compile_color_field(
                    path,
                    &format!("{base_path}.emissive"),
                    value,
                    ColorChannelKind::Emissive,
                )
            })
            .transpose()?,
        opacity: raw
            .opacity
            .as_ref()
            .map(|value| compile_scalar_field(path, &format!("{base_path}.opacity"), value))
            .transpose()?,
    })
}

fn compile_layer(
    path: &Path,
    base_path: &str,
    raw: &RawLayer,
    domain: SurfaceDomain,
    mask_depth: usize,
) -> Result<CompiledLayer, TextureDataError> {
    let strength = validate_unit_interval(
        path,
        &format!("{base_path}.strength"),
        raw.strength.unwrap_or(1.0),
    )?;
    let fields = PartialLayerFields {
        albedo: raw
            .albedo
            .as_ref()
            .map(|value| {
                compile_color_field(
                    path,
                    &format!("{base_path}.albedo"),
                    value,
                    ColorChannelKind::Albedo,
                )
            })
            .transpose()?,
        height: raw
            .height
            .as_ref()
            .map(|value| compile_scalar_field(path, &format!("{base_path}.height"), value))
            .transpose()?,
        roughness: raw
            .roughness
            .as_ref()
            .map(|value| compile_scalar_field(path, &format!("{base_path}.roughness"), value))
            .transpose()?,
        ao: raw
            .ao
            .as_ref()
            .map(|value| compile_scalar_field(path, &format!("{base_path}.ao"), value))
            .transpose()?,
        metallic: raw
            .metallic
            .as_ref()
            .map(|value| compile_scalar_field(path, &format!("{base_path}.metallic"), value))
            .transpose()?,
        emissive: raw
            .emissive
            .as_ref()
            .map(|value| {
                compile_color_field(
                    path,
                    &format!("{base_path}.emissive"),
                    value,
                    ColorChannelKind::Emissive,
                )
            })
            .transpose()?,
        opacity: raw
            .opacity
            .as_ref()
            .map(|value| compile_scalar_field(path, &format!("{base_path}.opacity"), value))
            .transpose()?,
    };
    if fields.albedo.is_none()
        && fields.height.is_none()
        && fields.roughness.is_none()
        && fields.ao.is_none()
        && fields.metallic.is_none()
        && fields.emissive.is_none()
        && fields.opacity.is_none()
    {
        return Err(TextureDataError::validate(
            path,
            base_path.to_string(),
            "layer must write at least one channel",
        ));
    }
    Ok(CompiledLayer {
        name: raw.name.clone(),
        mask: compile_mask(
            path,
            &format!("{base_path}.mask"),
            &raw.mask,
            domain,
            mask_depth + 1,
        )?,
        strength,
        fields,
        blend: compile_blend_spec(path, &format!("{base_path}.blend"), &raw.blend)?,
    })
}

fn compile_blend_spec(
    path: &Path,
    base_path: &str,
    raw: &RawBlendSpec,
) -> Result<LayerBlendSpec, TextureDataError> {
    Ok(LayerBlendSpec {
        albedo: parse_color_blend_mode(
            path,
            &format!("{base_path}.albedo"),
            raw.albedo.as_deref(),
        )?,
        height: parse_scalar_blend_mode(
            path,
            &format!("{base_path}.height"),
            raw.height.as_deref(),
        )?,
        roughness: parse_scalar_blend_mode(
            path,
            &format!("{base_path}.roughness"),
            raw.roughness.as_deref(),
        )?,
        ao: parse_scalar_blend_mode(path, &format!("{base_path}.ao"), raw.ao.as_deref())?,
        metallic: parse_scalar_blend_mode(
            path,
            &format!("{base_path}.metallic"),
            raw.metallic.as_deref(),
        )?,
        emissive: parse_color_blend_mode(
            path,
            &format!("{base_path}.emissive"),
            raw.emissive.as_deref(),
        )?,
        opacity: parse_scalar_blend_mode(
            path,
            &format!("{base_path}.opacity"),
            raw.opacity.as_deref(),
        )?,
    })
}

fn compile_signal(
    path: &Path,
    base_path: &str,
    raw: &RawSignalSpec,
) -> Result<CompiledSignal, TextureDataError> {
    let kind = match raw.kind.as_str() {
        "constant" => {
            let value = validate_unit_interval(
                path,
                &format!("{base_path}.value"),
                raw.value.unwrap_or(-1.0),
            )?;
            CompiledSignalKind::Constant { value }
        }
        "noise" => CompiledSignalKind::Noise {
            noise: compile_noise(path, &format!("{base_path}.noise"), raw.noise.as_ref())?,
            remap: compile_scalar_remap(path, &format!("{base_path}.remap"), raw.remap.as_ref())?,
        },
        "curve" => CompiledSignalKind::Curve {
            source: require_string(path, &format!("{base_path}.source"), raw.source.as_ref())?,
            curve: compile_curve(path, &format!("{base_path}.curve"), raw.curve.as_ref())?,
            clamp: validate_range(
                path,
                &format!("{base_path}.clamp"),
                raw.clamp.unwrap_or([0.0, 1.0]),
            )?,
        },
        "combine" => {
            let inputs = raw.inputs.clone().unwrap_or_default();
            if inputs.is_empty() {
                return Err(TextureDataError::validate(
                    path,
                    format!("{base_path}.inputs"),
                    "combine inputs must not be empty",
                ));
            }
            let op = match raw.op.as_deref() {
                Some("add") => SignalOp::Add,
                Some("multiply") => SignalOp::Multiply,
                Some("min") => SignalOp::Min,
                Some("max") => SignalOp::Max,
                Some("subtract") => SignalOp::Subtract,
                Some("average") => SignalOp::Average,
                _ => {
                    return Err(TextureDataError::validate(
                        path,
                        format!("{base_path}.op"),
                        "invalid combine op",
                    ))
                }
            };
            if op == SignalOp::Subtract && inputs.len() != 2 {
                return Err(TextureDataError::validate(
                    path,
                    format!("{base_path}.inputs"),
                    "subtract requires exactly 2 inputs",
                ));
            }
            if op != SignalOp::Subtract && !(2..=8).contains(&inputs.len()) {
                return Err(TextureDataError::validate(
                    path,
                    format!("{base_path}.inputs"),
                    "combine input count must be within 2..8",
                ));
            }
            CompiledSignalKind::Combine {
                op,
                inputs,
                clamp: validate_range(
                    path,
                    &format!("{base_path}.clamp"),
                    raw.clamp.unwrap_or([0.0, 1.0]),
                )?,
            }
        }
        "mask" => CompiledSignalKind::Mask {
            mask: compile_mask(
                path,
                &format!("{base_path}.mask"),
                raw.mask.as_ref().ok_or_else(|| {
                    TextureDataError::validate(
                        path,
                        format!("{base_path}.mask"),
                        "mask signal requires `mask`",
                    )
                })?,
                SurfaceDomain::FaceUv,
                1,
            )?,
        },
        _ => {
            return Err(TextureDataError::validate(
                path,
                base_path.to_string(),
                "invalid signal kind",
            ))
        }
    };
    Ok(CompiledSignal { kind })
}

fn compile_color_field(
    path: &Path,
    base_path: &str,
    raw: &RawColorFieldSpec,
    channel: ColorChannelKind,
) -> Result<CompiledColorField, TextureDataError> {
    let intensity = raw.intensity.unwrap_or(1.0);
    if intensity < 0.0 {
        return Err(TextureDataError::validate(
            path,
            format!("{base_path}.intensity"),
            "must be >= 0",
        ));
    }
    if channel == ColorChannelKind::Albedo && (intensity - 1.0).abs() > f32::EPSILON {
        return Err(TextureDataError::validate(
            path,
            format!("{base_path}.intensity"),
            "albedo intensity must equal 1.0",
        ));
    }
    match raw.mode.as_str() {
        "constant" => Ok(CompiledColorField {
            source: ColorFieldSource::Constant,
            mapping: default_color_mapping(),
            palette: None,
            constant: Some(parse_color(
                path,
                &format!("{base_path}.value"),
                raw.value.as_deref(),
            )?),
            intensity,
        }),
        "palette" => {
            let source = match (raw.source.as_ref(), raw.noise.as_ref()) {
                (Some(source), None) => ColorFieldSource::Signal(source.clone()),
                (None, Some(noise)) => ColorFieldSource::InlineNoise(compile_noise(
                    path,
                    &format!("{base_path}.noise"),
                    Some(noise),
                )?),
                _ => {
                    return Err(TextureDataError::validate(
                        path,
                        base_path.to_string(),
                        "palette mode requires exactly one of `source` or `noise`",
                    ))
                }
            };
            let palette =
                compile_palette(path, &format!("{base_path}.palette"), raw.palette.as_ref())?;
            Ok(CompiledColorField {
                source,
                mapping: compile_color_mapping(
                    path,
                    &format!("{base_path}.mapping"),
                    raw.mapping.as_ref(),
                )?,
                palette: Some(palette),
                constant: None,
                intensity,
            })
        }
        _ => Err(TextureDataError::validate(
            path,
            base_path.to_string(),
            "invalid color field mode",
        )),
    }
}

fn compile_scalar_field(
    path: &Path,
    base_path: &str,
    raw: &RawScalarFieldSpec,
) -> Result<CompiledScalarField, TextureDataError> {
    match raw.mode.as_str() {
        "constant" => Ok(CompiledScalarField {
            source: ScalarFieldSource::Constant,
            remap: None,
            constant: Some(validate_finite(
                path,
                &format!("{base_path}.value"),
                raw.value.unwrap_or(0.0),
            )?),
        }),
        "signal" => Ok(CompiledScalarField {
            source: ScalarFieldSource::Signal(require_string(
                path,
                &format!("{base_path}.source"),
                raw.source.as_ref(),
            )?),
            remap: Some(compile_scalar_remap(
                path,
                &format!("{base_path}.remap"),
                raw.remap.as_ref(),
            )?),
            constant: None,
        }),
        "noise" => Ok(CompiledScalarField {
            source: ScalarFieldSource::InlineNoise(compile_noise(
                path,
                &format!("{base_path}.noise"),
                raw.noise.as_ref(),
            )?),
            remap: Some(compile_scalar_remap(
                path,
                &format!("{base_path}.remap"),
                raw.remap.as_ref(),
            )?),
            constant: None,
        }),
        _ => Err(TextureDataError::validate(
            path,
            base_path.to_string(),
            "invalid scalar field mode",
        )),
    }
}

fn compile_palette(
    path: &Path,
    base_path: &str,
    palette: Option<&RawPalette>,
) -> Result<Vec<PaletteStop>, TextureDataError> {
    let Some(palette) = palette else {
        return Err(TextureDataError::validate(
            path,
            base_path.to_string(),
            "palette is required",
        ));
    };
    if !(2..=MAX_PALETTE_STOPS).contains(&palette.len()) {
        return Err(TextureDataError::validate(
            path,
            base_path.to_string(),
            format!("palette stops must be within 2..{MAX_PALETTE_STOPS}"),
        ));
    }
    let mut stops = Vec::with_capacity(palette.len());
    let explicit = palette
        .iter()
        .any(|item| matches!(item, RawPaletteItem::Stop(_)));
    if explicit {
        for (index, item) in palette.iter().enumerate() {
            let RawPaletteItem::Stop(stop) = item else {
                return Err(TextureDataError::validate(
                    path,
                    format!("{base_path}.{index}"),
                    "palette cannot mix shorthand and explicit stops",
                ));
            };
            stops.push(PaletteStop {
                at: stop.at,
                color: parse_color(
                    path,
                    &format!("{base_path}.{index}.color"),
                    Some(&stop.color),
                )?,
            });
        }
    } else {
        let denom = (palette.len() - 1) as f32;
        for (index, item) in palette.iter().enumerate() {
            let RawPaletteItem::Color(color) = item else {
                unreachable!("mixed palette handled above");
            };
            stops.push(PaletteStop {
                at: index as f32 / denom,
                color: parse_color(path, &format!("{base_path}.{index}"), Some(color))?,
            });
        }
    }
    if (stops.first().map(|stop| stop.at).unwrap_or_default() - 0.0).abs() > f32::EPSILON
        || (stops.last().map(|stop| stop.at).unwrap_or_default() - 1.0).abs() > f32::EPSILON
    {
        return Err(TextureDataError::validate(
            path,
            base_path.to_string(),
            "palette stops must start at 0.0 and end at 1.0",
        ));
    }
    let mut last_at = -1.0f32;
    for (index, stop) in stops.iter().enumerate() {
        if !(0.0..=1.0).contains(&stop.at) || stop.at <= last_at {
            return Err(TextureDataError::validate(
                path,
                format!("{base_path}.{index}.at"),
                "palette stop positions must be within 0..1 and strictly increasing",
            ));
        }
        last_at = stop.at;
    }
    Ok(stops)
}

fn compile_color_mapping(
    path: &Path,
    base_path: &str,
    raw: Option<&RawColorMappingSpec>,
) -> Result<ColorMappingSpec, TextureDataError> {
    let raw = raw.cloned().unwrap_or(RawColorMappingSpec {
        mode: None,
        levels: None,
        contrast: None,
        bias: None,
        invert: None,
        dither: None,
    });
    let mode = match raw.mode.as_deref().unwrap_or("quantized") {
        "quantized" => ColorMappingMode::Quantized,
        "gradient" => ColorMappingMode::Gradient,
        _ => {
            return Err(TextureDataError::validate(
                path,
                format!("{base_path}.mode"),
                "invalid mapping mode",
            ))
        }
    };
    let levels = raw.levels.unwrap_or(0);
    if levels != 0 && !(2..=64).contains(&levels) {
        return Err(TextureDataError::validate(
            path,
            format!("{base_path}.levels"),
            "levels must be 0 or within 2..64",
        ));
    }
    let contrast = raw.contrast.unwrap_or(1.0);
    if !contrast.is_finite() || contrast <= 0.0 {
        return Err(TextureDataError::validate(
            path,
            format!("{base_path}.contrast"),
            "contrast must be > 0",
        ));
    }
    let dither = match raw.dither.as_deref().unwrap_or("none") {
        "none" => DitherMode::None,
        "bayer4" => DitherMode::Bayer4,
        _ => {
            return Err(TextureDataError::validate(
                path,
                format!("{base_path}.dither"),
                "invalid dither mode",
            ))
        }
    };
    if mode == ColorMappingMode::Gradient && dither != DitherMode::None {
        return Err(TextureDataError::validate(
            path,
            format!("{base_path}.dither"),
            "gradient mapping does not support dithering",
        ));
    }
    Ok(ColorMappingSpec {
        mode,
        levels,
        contrast,
        bias: raw.bias.unwrap_or(0.0),
        invert: raw.invert.unwrap_or(false),
        dither,
    })
}

fn compile_scalar_remap(
    path: &Path,
    base_path: &str,
    raw: Option<&RawScalarRemapSpec>,
) -> Result<ScalarRemapSpec, TextureDataError> {
    let raw = raw.cloned().unwrap_or(RawScalarRemapSpec {
        range: None,
        contrast: None,
        bias: None,
        invert: None,
        curve: None,
        clamp: None,
    });
    let range = validate_range(
        path,
        &format!("{base_path}.range"),
        raw.range.unwrap_or([0.0, 1.0]),
    )?;
    let contrast = raw.contrast.unwrap_or(1.0);
    if !contrast.is_finite() || contrast <= 0.0 {
        return Err(TextureDataError::validate(
            path,
            format!("{base_path}.contrast"),
            "contrast must be > 0",
        ));
    }
    Ok(ScalarRemapSpec {
        range,
        contrast,
        bias: raw.bias.unwrap_or(0.0),
        invert: raw.invert.unwrap_or(false),
        curve: raw
            .curve
            .as_ref()
            .map(|curve| compile_curve(path, &format!("{base_path}.curve"), Some(curve)))
            .transpose()?
            .unwrap_or_else(default_curve),
        clamp: validate_range(
            path,
            &format!("{base_path}.clamp"),
            raw.clamp.unwrap_or([0.0, 1.0]),
        )?,
    })
}

fn compile_curve(
    path: &Path,
    base_path: &str,
    raw: Option<&RawCurveSpec>,
) -> Result<CurveSpec, TextureDataError> {
    let Some(raw) = raw else {
        return Err(TextureDataError::validate(
            path,
            base_path.to_string(),
            "curve is required",
        ));
    };
    let interp = match raw.interp.as_str() {
        "linear" => CurveInterpolation::Linear,
        "smoothstep" => CurveInterpolation::Smoothstep,
        _ => {
            return Err(TextureDataError::validate(
                path,
                format!("{base_path}.interp"),
                "invalid curve interpolation",
            ))
        }
    };
    if !(2..=64).contains(&raw.stops.len()) {
        return Err(TextureDataError::validate(
            path,
            format!("{base_path}.stops"),
            "curve stops must be within 2..64",
        ));
    }
    let mut last_x = -1.0f32;
    for (index, stop) in raw.stops.iter().enumerate() {
        let [x, _y] = *stop;
        if index == 0 && (x - 0.0).abs() > f32::EPSILON {
            return Err(TextureDataError::validate(
                path,
                format!("{base_path}.stops.{index}"),
                "curve must start at x=0",
            ));
        }
        if index == raw.stops.len() - 1 && (x - 1.0).abs() > f32::EPSILON {
            return Err(TextureDataError::validate(
                path,
                format!("{base_path}.stops.{index}"),
                "curve must end at x=1",
            ));
        }
        if x <= last_x {
            return Err(TextureDataError::validate(
                path,
                format!("{base_path}.stops.{index}"),
                "curve stop x values must be strictly increasing",
            ));
        }
        last_x = x;
    }
    Ok(CurveSpec {
        interp,
        stops: raw.stops.clone(),
    })
}

fn compile_noise(
    path: &Path,
    base_path: &str,
    raw: Option<&RawNoiseSpec>,
) -> Result<NoiseSpec, TextureDataError> {
    let Some(raw) = raw else {
        return Err(TextureDataError::validate(
            path,
            base_path.to_string(),
            "noise spec is required",
        ));
    };
    if !raw.scale.is_finite() || raw.scale <= 0.0 {
        return Err(TextureDataError::validate(
            path,
            format!("{base_path}.scale"),
            "scale must be > 0",
        ));
    }
    let basis = match raw.basis.as_str() {
        "value" => NoiseBasis::Value,
        "gradient" => NoiseBasis::Gradient,
        "cellular" => NoiseBasis::Cellular,
        _ => {
            return Err(TextureDataError::validate(
                path,
                format!("{base_path}.basis"),
                "invalid noise basis",
            ))
        }
    };
    let fractal = match raw.fractal.as_str() {
        "none" => NoiseFractal::None,
        "fbm" => NoiseFractal::Fbm,
        "billow" => NoiseFractal::Billow,
        "ridged" => NoiseFractal::Ridged,
        _ => {
            return Err(TextureDataError::validate(
                path,
                format!("{base_path}.fractal"),
                "invalid noise fractal",
            ))
        }
    };
    let octaves = raw.octaves.unwrap_or(4);
    if !(1..=MAX_OCTAVES).contains(&octaves) {
        return Err(TextureDataError::validate(
            path,
            format!("{base_path}.octaves"),
            "octaves must be within 1..8",
        ));
    }
    if basis == NoiseBasis::Cellular && fractal != NoiseFractal::None {
        return Err(TextureDataError::validate(
            path,
            format!("{base_path}.fractal"),
            "cellular basis only supports fractal=none",
        ));
    }
    if fractal == NoiseFractal::None && octaves != 1 {
        return Err(TextureDataError::validate(
            path,
            format!("{base_path}.octaves"),
            "fractal=none requires octaves=1",
        ));
    }
    let lacunarity = raw.lacunarity.unwrap_or(2.0);
    if !lacunarity.is_finite() || lacunarity <= 1.0 {
        return Err(TextureDataError::validate(
            path,
            format!("{base_path}.lacunarity"),
            "lacunarity must be > 1",
        ));
    }
    let gain = raw.gain.unwrap_or(0.5);
    if !gain.is_finite() || !(0.0..=1.0).contains(&gain) {
        return Err(TextureDataError::validate(
            path,
            format!("{base_path}.gain"),
            "gain must be within 0..1",
        ));
    }
    let stretch = raw.stretch.unwrap_or([1.0, 1.0, 1.0]);
    for (index, value) in stretch.iter().enumerate() {
        if !value.is_finite() || *value <= 0.0 {
            return Err(TextureDataError::validate(
                path,
                format!("{base_path}.stretch.{index}"),
                "stretch values must be > 0",
            ));
        }
    }
    let cellular_return = match (basis, raw.cellular_return.as_deref()) {
        (NoiseBasis::Cellular, Some("f1")) | (NoiseBasis::Cellular, None) => Some("f1".to_string()),
        (NoiseBasis::Cellular, Some(_)) => {
            return Err(TextureDataError::validate(
                path,
                format!("{base_path}.cellular_return"),
                "cellular_return must be `f1`",
            ))
        }
        (_, Some(_)) => {
            return Err(TextureDataError::validate(
                path,
                format!("{base_path}.cellular_return"),
                "cellular_return is only valid for cellular basis",
            ))
        }
        _ => None,
    };
    let warp = compile_warp(path, &format!("{base_path}.warp"), raw.warp.as_ref())?;
    Ok(NoiseSpec {
        basis,
        fractal,
        scale: raw.scale,
        stretch,
        octaves,
        lacunarity,
        gain,
        offset: raw.offset.unwrap_or([0.0, 0.0, 0.0]),
        seed_offset: raw.seed_offset.unwrap_or(0),
        cellular_return,
        warp,
    })
}

fn compile_warp(
    path: &Path,
    base_path: &str,
    raw: Option<&crate::schema::RawWarpSpec>,
) -> Result<WarpSpec, TextureDataError> {
    let Some(raw) = raw else {
        return Ok(WarpSpec {
            amplitude: 0.0,
            basis: NoiseBasis::Gradient,
            fractal: NoiseFractal::Fbm,
            scale_multiplier: 1.0,
            octaves: 2,
            lacunarity: 2.0,
            gain: 0.5,
            seed_offset: 4096,
        });
    };
    let amplitude = raw.amplitude.unwrap_or(0.0);
    if !amplitude.is_finite() || amplitude < 0.0 {
        return Err(TextureDataError::validate(
            path,
            format!("{base_path}.amplitude"),
            "warp amplitude must be >= 0",
        ));
    }
    let basis = match raw.basis.as_deref().unwrap_or("gradient") {
        "value" => NoiseBasis::Value,
        "gradient" => NoiseBasis::Gradient,
        "cellular" => NoiseBasis::Cellular,
        _ => {
            return Err(TextureDataError::validate(
                path,
                format!("{base_path}.basis"),
                "invalid warp basis",
            ))
        }
    };
    let fractal = match raw.fractal.as_deref().unwrap_or("fbm") {
        "none" => NoiseFractal::None,
        "fbm" => NoiseFractal::Fbm,
        "billow" => NoiseFractal::Billow,
        "ridged" => NoiseFractal::Ridged,
        _ => {
            return Err(TextureDataError::validate(
                path,
                format!("{base_path}.fractal"),
                "invalid warp fractal",
            ))
        }
    };
    let scale_multiplier = raw.scale_multiplier.unwrap_or(1.0);
    if !scale_multiplier.is_finite() || scale_multiplier <= 0.0 {
        return Err(TextureDataError::validate(
            path,
            format!("{base_path}.scale_multiplier"),
            "warp scale_multiplier must be > 0",
        ));
    }
    let octaves = raw.octaves.unwrap_or(2);
    if !(1..=MAX_OCTAVES).contains(&octaves) {
        return Err(TextureDataError::validate(
            path,
            format!("{base_path}.octaves"),
            "octaves must be within 1..8",
        ));
    }
    let lacunarity = raw.lacunarity.unwrap_or(2.0);
    if !lacunarity.is_finite() || lacunarity <= 1.0 {
        return Err(TextureDataError::validate(
            path,
            format!("{base_path}.lacunarity"),
            "lacunarity must be > 1",
        ));
    }
    let gain = raw.gain.unwrap_or(0.5);
    if !gain.is_finite() || !(0.0..=1.0).contains(&gain) {
        return Err(TextureDataError::validate(
            path,
            format!("{base_path}.gain"),
            "gain must be within 0..1",
        ));
    }
    Ok(WarpSpec {
        amplitude,
        basis,
        fractal,
        scale_multiplier,
        octaves,
        lacunarity,
        gain,
        seed_offset: raw.seed_offset.unwrap_or(4096),
    })
}

fn compile_mask(
    path: &Path,
    base_path: &str,
    raw: &RawMaskSpec,
    domain: SurfaceDomain,
    depth: usize,
) -> Result<MaskSpec, TextureDataError> {
    if depth > MAX_MASK_DEPTH {
        return Err(TextureDataError::validate(
            path,
            base_path.to_string(),
            format!("mask recursion depth exceeds {MAX_MASK_DEPTH}"),
        ));
    }
    match raw.mode.as_str() {
        "full" => Ok(MaskSpec::Full),
        "signal" => Ok(MaskSpec::Signal {
            source: require_string(path, &format!("{base_path}.source"), raw.source.as_ref())?,
        }),
        "axis_band" => {
            let axis = match raw.axis.as_deref() {
                Some("u") => MaskAxis::U,
                Some("v") => MaskAxis::V,
                Some("x") => MaskAxis::X,
                Some("y") => MaskAxis::Y,
                Some("z") => MaskAxis::Z,
                _ => {
                    return Err(TextureDataError::validate(
                        path,
                        format!("{base_path}.axis"),
                        "invalid axis",
                    ))
                }
            };
            let from = validate_finite(
                path,
                &format!("{base_path}.from"),
                raw.from.unwrap_or(f32::NAN),
            )?;
            let to = validate_finite(path, &format!("{base_path}.to"), raw.to.unwrap_or(f32::NAN))?;
            if to <= from {
                return Err(TextureDataError::validate(
                    path,
                    format!("{base_path}.to"),
                    "`to` must be greater than `from`",
                ));
            }
            let falloff = validate_non_negative(
                path,
                &format!("{base_path}.falloff"),
                raw.falloff.unwrap_or(0.0),
            )?;
            Ok(MaskSpec::AxisBand {
                axis,
                from,
                to,
                falloff,
                invert: raw.invert.unwrap_or(false),
                jitter: raw
                    .jitter
                    .as_ref()
                    .map(|jitter| compile_mask_jitter(path, &format!("{base_path}.jitter"), jitter))
                    .transpose()?,
            })
        }
        "threshold" => {
            let threshold = validate_unit_interval(
                path,
                &format!("{base_path}.threshold"),
                raw.threshold.unwrap_or(-1.0),
            )?;
            let softness = validate_unit_interval(
                path,
                &format!("{base_path}.softness"),
                raw.softness.unwrap_or(0.0),
            )?;
            let source = match (raw.source.as_ref(), raw.noise.as_ref()) {
                (Some(source), None) => ThresholdSource::Signal(source.clone()),
                (None, Some(noise)) => ThresholdSource::InlineNoise(compile_noise(
                    path,
                    &format!("{base_path}.noise"),
                    Some(noise),
                )?),
                _ => {
                    return Err(TextureDataError::validate(
                        path,
                        base_path.to_string(),
                        "threshold mask requires exactly one of `source` or `noise`",
                    ))
                }
            };
            Ok(MaskSpec::Threshold {
                source,
                threshold,
                softness,
                invert: raw.invert.unwrap_or(false),
            })
        }
        "edge_distance" => {
            if domain != SurfaceDomain::FaceUv {
                return Err(TextureDataError::validate(
                    path,
                    base_path.to_string(),
                    "edge_distance is only valid for face_uv surfaces",
                ));
            }
            let from = validate_finite(
                path,
                &format!("{base_path}.from"),
                raw.from.unwrap_or(f32::NAN),
            )?;
            let to = validate_finite(path, &format!("{base_path}.to"), raw.to.unwrap_or(f32::NAN))?;
            if to <= from {
                return Err(TextureDataError::validate(
                    path,
                    format!("{base_path}.to"),
                    "`to` must be greater than `from`",
                ));
            }
            let falloff = validate_non_negative(
                path,
                &format!("{base_path}.falloff"),
                raw.falloff.unwrap_or(0.0),
            )?;
            Ok(MaskSpec::EdgeDistance {
                from,
                to,
                falloff,
                invert: raw.invert.unwrap_or(false),
            })
        }
        "and" => compile_mask_list(path, base_path, raw, domain, depth, MaskListKind::And),
        "or" => compile_mask_list(path, base_path, raw, domain, depth, MaskListKind::Or),
        "subtract" => {
            compile_mask_list(path, base_path, raw, domain, depth, MaskListKind::Subtract)
        }
        "not" => Ok(MaskSpec::Not {
            item: Box::new(compile_mask(
                path,
                &format!("{base_path}.item"),
                raw.item.as_ref().ok_or_else(|| {
                    TextureDataError::validate(
                        path,
                        format!("{base_path}.item"),
                        "not mask requires `item`",
                    )
                })?,
                domain,
                depth + 1,
            )?),
        }),
        _ => Err(TextureDataError::validate(
            path,
            base_path.to_string(),
            "invalid mask mode",
        )),
    }
}

enum MaskListKind {
    And,
    Or,
    Subtract,
}

fn compile_mask_list(
    path: &Path,
    base_path: &str,
    raw: &RawMaskSpec,
    domain: SurfaceDomain,
    depth: usize,
    kind: MaskListKind,
) -> Result<MaskSpec, TextureDataError> {
    let items = raw.items.as_ref().ok_or_else(|| {
        TextureDataError::validate(
            path,
            format!("{base_path}.items"),
            "mask list requires `items`",
        )
    })?;
    match kind {
        MaskListKind::Subtract if items.len() != 2 => {
            return Err(TextureDataError::validate(
                path,
                format!("{base_path}.items"),
                "subtract mask requires exactly 2 items",
            ))
        }
        MaskListKind::And | MaskListKind::Or if !(2..=8).contains(&items.len()) => {
            return Err(TextureDataError::validate(
                path,
                format!("{base_path}.items"),
                "mask item count must be within 2..8",
            ))
        }
        _ => {}
    }
    let compiled = items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            compile_mask(
                path,
                &format!("{base_path}.items.{index}"),
                item,
                domain,
                depth + 1,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(match kind {
        MaskListKind::And => MaskSpec::And { items: compiled },
        MaskListKind::Or => MaskSpec::Or { items: compiled },
        MaskListKind::Subtract => MaskSpec::Subtract { items: compiled },
    })
}

fn compile_mask_jitter(
    path: &Path,
    base_path: &str,
    raw: &RawMaskJitter,
) -> Result<MaskJitterSpec, TextureDataError> {
    let amount = validate_non_negative(
        path,
        &format!("{base_path}.amount"),
        raw.amount.unwrap_or(0.0),
    )?;
    Ok(MaskJitterSpec {
        amount,
        source: raw.source.clone(),
    })
}

fn compile_normal_spec(
    path: &Path,
    base_path: &str,
    raw: Option<&RawNormalSpec>,
) -> Result<Option<NormalSpec>, TextureDataError> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let mode = match raw.mode.as_str() {
        "flat" => NormalMode::Flat,
        "derive_from_height" => NormalMode::DeriveFromHeight,
        _ => {
            return Err(TextureDataError::validate(
                path,
                format!("{base_path}.mode"),
                "invalid normal mode",
            ))
        }
    };
    let strength = validate_non_negative(
        path,
        &format!("{base_path}.strength"),
        raw.strength.unwrap_or(1.0),
    )?;
    Ok(Some(NormalSpec { mode, strength }))
}

fn compile_sampler_spec(
    path: &Path,
    base_path: &str,
    raw: Option<&RawSamplerSpec>,
) -> Result<Option<TextureSamplerSpec>, TextureDataError> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let mag_filter = match raw.mag_filter.as_deref().unwrap_or("nearest") {
        "nearest" => FilterModeSpec::Nearest,
        "linear" => FilterModeSpec::Linear,
        _ => {
            return Err(TextureDataError::validate(
                path,
                format!("{base_path}.mag_filter"),
                "invalid filter mode",
            ))
        }
    };
    let min_filter = match raw.min_filter.as_deref().unwrap_or("nearest") {
        "nearest" => FilterModeSpec::Nearest,
        "linear" => FilterModeSpec::Linear,
        _ => {
            return Err(TextureDataError::validate(
                path,
                format!("{base_path}.min_filter"),
                "invalid filter mode",
            ))
        }
    };
    let mipmap_filter = match raw.mipmap_filter.as_deref().unwrap_or("nearest") {
        "none" => MipmapFilterModeSpec::None,
        "nearest" => MipmapFilterModeSpec::Nearest,
        "linear" => MipmapFilterModeSpec::Linear,
        _ => {
            return Err(TextureDataError::validate(
                path,
                format!("{base_path}.mipmap_filter"),
                "invalid mipmap filter",
            ))
        }
    };
    let anisotropy = raw.anisotropy.unwrap_or(1);
    if !(1..=MAX_ANISOTROPY).contains(&anisotropy) {
        return Err(TextureDataError::validate(
            path,
            format!("{base_path}.anisotropy"),
            "anisotropy must be within 1..16",
        ));
    }
    let address_u = match raw.address_u.as_deref().unwrap_or("clamp_to_edge") {
        "clamp_to_edge" => AddressModeSpec::ClampToEdge,
        "repeat" => AddressModeSpec::Repeat,
        _ => {
            return Err(TextureDataError::validate(
                path,
                format!("{base_path}.address_u"),
                "invalid address mode",
            ))
        }
    };
    let address_v = match raw.address_v.as_deref().unwrap_or("clamp_to_edge") {
        "clamp_to_edge" => AddressModeSpec::ClampToEdge,
        "repeat" => AddressModeSpec::Repeat,
        _ => {
            return Err(TextureDataError::validate(
                path,
                format!("{base_path}.address_v"),
                "invalid address mode",
            ))
        }
    };
    Ok(Some(TextureSamplerSpec {
        mag_filter,
        min_filter,
        mipmap_filter,
        anisotropy,
        address_u,
        address_v,
    }))
}

fn topo_sort_signals(
    path: &Path,
    surface_name: &str,
    signals: &BTreeMap<String, CompiledSignal>,
) -> Result<Vec<String>, TextureDataError> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Mark {
        Visiting,
        Visited,
    }
    let mut order = Vec::with_capacity(signals.len());
    let mut marks = BTreeMap::new();

    fn dependencies(signal: &CompiledSignal) -> Vec<&str> {
        match &signal.kind {
            CompiledSignalKind::Constant { .. }
            | CompiledSignalKind::Noise { .. }
            | CompiledSignalKind::Mask { .. } => Vec::new(),
            CompiledSignalKind::Curve { source, .. } => vec![source.as_str()],
            CompiledSignalKind::Combine { inputs, .. } => {
                inputs.iter().map(String::as_str).collect()
            }
        }
    }

    fn visit(
        path: &Path,
        surface_name: &str,
        name: &str,
        signals: &BTreeMap<String, CompiledSignal>,
        marks: &mut BTreeMap<String, Mark>,
        order: &mut Vec<String>,
    ) -> Result<(), TextureDataError> {
        if let Some(mark) = marks.get(name) {
            if *mark == Mark::Visited {
                return Ok(());
            }
            return Err(TextureDataError::validate(
                path,
                format!("surfaces.{surface_name}.signals.{name}"),
                "signal graph contains a cycle",
            ));
        }
        marks.insert(name.to_string(), Mark::Visiting);
        let signal = signals.get(name).ok_or_else(|| {
            TextureDataError::validate(
                path,
                format!("surfaces.{surface_name}.signals.{name}"),
                "unknown referenced signal",
            )
        })?;
        for dependency in dependencies(signal) {
            if !signals.contains_key(dependency) {
                return Err(TextureDataError::validate(
                    path,
                    format!("surfaces.{surface_name}.signals.{name}"),
                    format!("unknown signal `{dependency}`"),
                ));
            }
            visit(path, surface_name, dependency, signals, marks, order)?;
        }
        marks.insert(name.to_string(), Mark::Visited);
        order.push(name.to_string());
        Ok(())
    }

    for name in signals.keys() {
        visit(path, surface_name, name, signals, &mut marks, &mut order)?;
    }
    Ok(order)
}

fn validate_name(path: &Path, base_path: &str, name: &str) -> Result<(), TextureDataError> {
    let valid = !name.is_empty()
        && name.chars().enumerate().all(|(index, ch)| {
            ch.is_ascii_lowercase() || ch.is_ascii_digit() || (index > 0 && ch == '_')
        });
    if !valid {
        return Err(TextureDataError::validate(
            path,
            base_path.to_string(),
            "name must match ^[a-z0-9][a-z0-9_]*$",
        ));
    }
    Ok(())
}

fn parse_color(
    path: &Path,
    base_path: &str,
    value: Option<&str>,
) -> Result<[f32; 3], TextureDataError> {
    let Some(value) = value else {
        return Err(TextureDataError::validate(
            path,
            base_path.to_string(),
            "color is required",
        ));
    };
    if value.len() != 7 || !value.starts_with('#') {
        return Err(TextureDataError::validate(
            path,
            base_path.to_string(),
            "color must use #RRGGBB format",
        ));
    }
    let parse = |range: std::ops::Range<usize>| -> Result<f32, TextureDataError> {
        let component = u8::from_str_radix(&value[range], 16).map_err(|_| {
            TextureDataError::validate(path, base_path.to_string(), "invalid hex color component")
        })?;
        Ok(srgb_u8_to_linear(component))
    };
    Ok([parse(1..3)?, parse(3..5)?, parse(5..7)?])
}

fn srgb_u8_to_linear(component: u8) -> f32 {
    let value = component as f32 / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn parse_domain(
    path: &Path,
    base_path: &str,
    value: Option<&str>,
) -> Result<Option<SurfaceDomain>, TextureDataError> {
    let Some(value) = value else {
        return Ok(None);
    };
    match value {
        "face_uv" => Ok(Some(SurfaceDomain::FaceUv)),
        "block_space" => Ok(Some(SurfaceDomain::BlockSpace)),
        _ => Err(TextureDataError::validate(
            path,
            base_path.to_string(),
            "invalid surface domain",
        )),
    }
}

fn parse_tile_mode(
    path: &Path,
    base_path: &str,
    value: Option<&str>,
) -> Result<Option<TileMode>, TextureDataError> {
    let Some(value) = value else {
        return Ok(None);
    };
    match value {
        "repeat" => Ok(Some(TileMode::Repeat)),
        "clamp" => Ok(Some(TileMode::Clamp)),
        _ => Err(TextureDataError::validate(
            path,
            base_path.to_string(),
            "invalid tile_mode",
        )),
    }
}

fn parse_alpha_mode(
    path: &Path,
    base_path: &str,
    value: Option<&str>,
) -> Result<Option<AlphaMode>, TextureDataError> {
    let Some(value) = value else {
        return Ok(None);
    };
    match value {
        "opaque" => Ok(Some(AlphaMode::Opaque)),
        "mask" => Ok(Some(AlphaMode::Mask)),
        "blend" => Ok(Some(AlphaMode::Blend)),
        _ => Err(TextureDataError::validate(
            path,
            base_path.to_string(),
            "invalid alpha_mode",
        )),
    }
}

fn parse_color_blend_mode(
    path: &Path,
    base_path: &str,
    value: Option<&str>,
) -> Result<ColorBlendMode, TextureDataError> {
    match value.unwrap_or("mix") {
        "mix" => Ok(ColorBlendMode::Mix),
        _ => Err(TextureDataError::validate(
            path,
            base_path.to_string(),
            "color blend mode must be `mix`",
        )),
    }
}

fn parse_scalar_blend_mode(
    path: &Path,
    base_path: &str,
    value: Option<&str>,
) -> Result<ScalarBlendMode, TextureDataError> {
    match value.unwrap_or("mix") {
        "mix" => Ok(ScalarBlendMode::Mix),
        "add" => Ok(ScalarBlendMode::Add),
        "multiply" => Ok(ScalarBlendMode::Multiply),
        "max" => Ok(ScalarBlendMode::Max),
        "min" => Ok(ScalarBlendMode::Min),
        _ => Err(TextureDataError::validate(
            path,
            base_path.to_string(),
            "invalid scalar blend mode",
        )),
    }
}

fn validate_range(
    path: &Path,
    base_path: &str,
    range: [f32; 2],
) -> Result<[f32; 2], TextureDataError> {
    if !range[0].is_finite() || !range[1].is_finite() || range[0] > range[1] {
        return Err(TextureDataError::validate(
            path,
            base_path.to_string(),
            "range is invalid",
        ));
    }
    Ok(range)
}

fn validate_unit_interval(
    path: &Path,
    base_path: &str,
    value: f32,
) -> Result<f32, TextureDataError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(TextureDataError::validate(
            path,
            base_path.to_string(),
            "value must be within 0..1",
        ));
    }
    Ok(value)
}

fn validate_non_negative(
    path: &Path,
    base_path: &str,
    value: f32,
) -> Result<f32, TextureDataError> {
    if !value.is_finite() || value < 0.0 {
        return Err(TextureDataError::validate(
            path,
            base_path.to_string(),
            "value must be >= 0",
        ));
    }
    Ok(value)
}

fn validate_finite(path: &Path, base_path: &str, value: f32) -> Result<f32, TextureDataError> {
    if !value.is_finite() {
        return Err(TextureDataError::validate(
            path,
            base_path.to_string(),
            "value must be finite",
        ));
    }
    Ok(value)
}

fn require_string(
    path: &Path,
    base_path: &str,
    value: Option<&String>,
) -> Result<String, TextureDataError> {
    let Some(value) = value else {
        return Err(TextureDataError::validate(
            path,
            base_path.to_string(),
            "string value is required",
        ));
    };
    Ok(value.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_json_rejects_duplicate_keys() {
        let bytes = br#"{"a":1,"a":2}"#;
        let error = compile_texture_set("/tmp/test.texture.json", bytes).expect_err("must fail");
        assert!(error.to_string().contains("duplicate object key"));
    }

    #[test]
    fn compile_docs_example_like_texture_set() {
        let bytes = br##"{
          "spec": "cruft.procedural_texture",
          "spec_version": "1.0.0",
          "profile": "voxel_cube_pbr",
          "output": {"size": 64, "mipmaps": "full", "normal_format": "opengl"},
          "surfaces": {
            "dirt": {
              "base": {
                "albedo": {
                  "mode": "palette",
                  "source": "base_noise",
                  "palette": ["#4A321F", "#5C3D26"]
                }
              },
              "signals": {
                "base_noise": {
                  "kind": "noise",
                  "noise": {
                    "basis": "value",
                    "fractal": "none",
                    "scale": 1.0,
                    "octaves": 1
                  }
                }
              }
            }
          },
          "textures": {
            "dirt_block": {
              "faces": {
                "all": "dirt"
              }
            }
          }
        }"##;
        let compiled = compile_texture_set("/tmp/test.texture.json", bytes).expect("must compile");
        assert_eq!(compiled.output.size, 64);
        assert!(compiled.textures.contains_key("dirt_block"));
        assert!(!compiled.canonical.json.is_empty());
    }

    #[test]
    fn derive_from_height_requires_composed_height_channel() {
        let bytes = br##"{
          "spec": "cruft.procedural_texture",
          "spec_version": "1.0.0",
          "profile": "voxel_cube_pbr",
          "output": {"size": 16},
          "surfaces": {
            "stone": {
              "normal": {"mode": "derive_from_height"},
              "base": {
                "albedo": {
                  "mode": "constant",
                  "value": "#808080"
                }
              }
            }
          },
          "textures": {
            "stone_block": {
              "faces": {
                "all": "stone"
              }
            }
          }
        }"##;
        let error = compile_texture_set("/tmp/test.texture.json", bytes).expect_err("must fail");

        assert_eq!(error.path.as_deref(), Some("surfaces.stone.normal"));
        assert!(error.message.contains("requires a composed height channel"));
    }

    #[test]
    fn canonical_form_expands_inline_noise_shorthand() {
        let bytes = br##"{
          "spec": "cruft.procedural_texture",
          "spec_version": "1.0.0",
          "profile": "voxel_cube_pbr",
          "output": {"size": 16},
          "surfaces": {
            "stone": {
              "base": {
                "albedo": {
                  "mode": "palette",
                  "noise": {
                    "basis": "value",
                    "fractal": "none",
                    "scale": 1.0,
                    "octaves": 1
                  },
                  "palette": ["#404040", "#808080"]
                },
                "height": {
                  "mode": "noise",
                  "noise": {
                    "basis": "gradient",
                    "fractal": "none",
                    "scale": 1.0,
                    "octaves": 1
                  }
                }
              },
              "layers": [
                {
                  "mask": {
                    "mode": "threshold",
                    "noise": {
                      "basis": "value",
                      "fractal": "none",
                      "scale": 2.0,
                      "octaves": 1
                    },
                    "threshold": 0.5
                  },
                  "height": {
                    "mode": "noise",
                    "noise": {
                      "basis": "value",
                      "fractal": "none",
                      "scale": 3.0,
                      "octaves": 1
                    }
                  }
                }
              ]
            }
          },
          "textures": {
            "stone_block": {
              "faces": {
                "all": "stone"
              }
            }
          }
        }"##;
        let compiled = compile_texture_set("/tmp/test.texture.json", bytes).expect("must compile");

        assert!(!compiled.canonical.json.contains("InlineNoise"));
        assert!(compiled.canonical.json.contains("__inline_noise_"));
    }
}
