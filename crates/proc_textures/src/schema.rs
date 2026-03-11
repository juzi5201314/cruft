use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Formatter;

use serde::de::{Error as DeError, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

use crate::error::TextureDataError;

#[derive(Debug, Clone, PartialEq)]
pub enum StrictValue {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Array(Vec<StrictValue>),
    Object(Vec<(String, StrictValue)>),
}

impl<'de> Deserialize<'de> for StrictValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StrictValueVisitor;

        impl<'de> Visitor<'de> for StrictValueVisitor {
            type Value = StrictValue;

            fn expecting(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("strict JSON value")
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
                Ok(StrictValue::Bool(value))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(StrictValue::Number(serde_json::Number::from(value)))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(StrictValue::Number(serde_json::Number::from(value)))
            }

            fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                let Some(number) = serde_json::Number::from_f64(value) else {
                    return Err(E::custom("non-finite number is not allowed"));
                };
                Ok(StrictValue::Number(number))
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
                Ok(StrictValue::String(value.to_string()))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
                Ok(StrictValue::String(value))
            }

            fn visit_none<E>(self) -> Result<Self::Value, E> {
                Ok(StrictValue::Null)
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(StrictValue::Null)
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values = Vec::new();
                while let Some(value) = seq.next_element::<StrictValue>()? {
                    values.push(value);
                }
                Ok(StrictValue::Array(values))
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut values = Vec::new();
                let mut seen = BTreeSet::new();
                while let Some((key, value)) = map.next_entry::<String, StrictValue>()? {
                    if !seen.insert(key.clone()) {
                        return Err(A::Error::custom(format!("duplicate object key `{key}`")));
                    }
                    values.push((key, value));
                }
                Ok(StrictValue::Object(values))
            }
        }

        deserializer.deserialize_any(StrictValueVisitor)
    }
}

impl StrictValue {
    pub fn to_serde_value(&self) -> serde_json::Value {
        match self {
            Self::Null => serde_json::Value::Null,
            Self::Bool(value) => serde_json::Value::Bool(*value),
            Self::Number(value) => serde_json::Value::Number(value.clone()),
            Self::String(value) => serde_json::Value::String(value.clone()),
            Self::Array(values) => {
                serde_json::Value::Array(values.iter().map(Self::to_serde_value).collect())
            }
            Self::Object(values) => {
                let mut object = serde_json::Map::with_capacity(values.len());
                for (key, value) in values {
                    object.insert(key.clone(), value.to_serde_value());
                }
                serde_json::Value::Object(object)
            }
        }
    }

    pub fn reject_core_nulls(
        &self,
        file: &std::path::Path,
        path: &str,
    ) -> Result<(), TextureDataError> {
        match self {
            Self::Null => {
                if path.is_empty() || path == "meta" || path == "extensions" {
                    return Ok(());
                }
                Err(TextureDataError::parse(
                    file,
                    Some(path.to_string()),
                    "null is not allowed in core schema fields",
                ))
            }
            Self::Array(values) => {
                for (index, value) in values.iter().enumerate() {
                    let child = if path.is_empty() {
                        index.to_string()
                    } else {
                        format!("{path}.{index}")
                    };
                    value.reject_core_nulls(file, &child)?;
                }
                Ok(())
            }
            Self::Object(values) => {
                for (key, value) in values {
                    let child = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };
                    let allow_subtree = child == "meta" || child == "extensions";
                    if allow_subtree {
                        continue;
                    }
                    value.reject_core_nulls(file, &child)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawTextureSet {
    pub spec: String,
    pub spec_version: String,
    pub profile: String,
    #[serde(default)]
    pub meta: BTreeMap<String, serde_json::Value>,
    pub output: RawOutput,
    #[serde(default)]
    pub defaults: RawDefaults,
    pub surfaces: BTreeMap<String, RawSurface>,
    pub textures: BTreeMap<String, RawTexture>,
    #[serde(default)]
    pub extensions_used: Vec<String>,
    #[serde(default)]
    pub extensions: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawDefaults {
    #[serde(default)]
    pub surface: RawSurfaceDefaults,
    #[serde(default)]
    pub texture: RawTextureDefaults,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawSurfaceDefaults {
    pub logical_size: Option<u32>,
    pub pixel_snap: Option<bool>,
    pub domain: Option<String>,
    pub tile_mode: Option<String>,
    pub seed: Option<u32>,
    pub normal: Option<RawNormalSpec>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawTextureDefaults {
    pub sampler: Option<RawSamplerSpec>,
    pub alpha_mode: Option<String>,
    pub cutout_threshold: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawOutput {
    pub size: u32,
    #[serde(default)]
    pub mipmaps: Option<String>,
    #[serde(default)]
    pub normal_format: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawSurface {
    #[serde(default)]
    pub logical_size: Option<u32>,
    #[serde(default)]
    pub pixel_snap: Option<bool>,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub tile_mode: Option<String>,
    #[serde(default)]
    pub seed: Option<u32>,
    #[serde(default)]
    pub signals: BTreeMap<String, RawSignalSpec>,
    pub base: RawLayerFields,
    #[serde(default)]
    pub layers: Vec<RawLayer>,
    #[serde(default)]
    pub normal: Option<RawNormalSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawTexture {
    #[serde(default)]
    pub sampler: Option<RawSamplerSpec>,
    #[serde(default)]
    pub alpha_mode: Option<String>,
    #[serde(default)]
    pub cutout_threshold: Option<f32>,
    pub faces: RawTextureFaces,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawTextureFaces {
    #[serde(default)]
    pub all: Option<RawFaceBinding>,
    #[serde(default)]
    pub top: Option<RawFaceBinding>,
    #[serde(default)]
    pub bottom: Option<RawFaceBinding>,
    #[serde(default)]
    pub sides: Option<RawFaceBinding>,
    #[serde(default)]
    pub north: Option<RawFaceBinding>,
    #[serde(default)]
    pub south: Option<RawFaceBinding>,
    #[serde(default)]
    pub east: Option<RawFaceBinding>,
    #[serde(default)]
    pub west: Option<RawFaceBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RawFaceBinding {
    Name(String),
    Binding(RawFaceBindingObject),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawFaceBindingObject {
    pub surface: String,
    #[serde(default)]
    pub rotate: Option<u16>,
    #[serde(default)]
    pub flip_u: Option<bool>,
    #[serde(default)]
    pub flip_v: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawLayer {
    #[serde(default)]
    pub name: Option<String>,
    pub mask: RawMaskSpec,
    #[serde(default)]
    pub strength: Option<f32>,
    #[serde(default)]
    pub albedo: Option<RawColorFieldSpec>,
    #[serde(default)]
    pub height: Option<RawScalarFieldSpec>,
    #[serde(default)]
    pub roughness: Option<RawScalarFieldSpec>,
    #[serde(default)]
    pub ao: Option<RawScalarFieldSpec>,
    #[serde(default)]
    pub metallic: Option<RawScalarFieldSpec>,
    #[serde(default)]
    pub emissive: Option<RawColorFieldSpec>,
    #[serde(default)]
    pub opacity: Option<RawScalarFieldSpec>,
    #[serde(default)]
    pub blend: RawBlendSpec,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawLayerFields {
    #[serde(default)]
    pub albedo: Option<RawColorFieldSpec>,
    #[serde(default)]
    pub height: Option<RawScalarFieldSpec>,
    #[serde(default)]
    pub roughness: Option<RawScalarFieldSpec>,
    #[serde(default)]
    pub ao: Option<RawScalarFieldSpec>,
    #[serde(default)]
    pub metallic: Option<RawScalarFieldSpec>,
    #[serde(default)]
    pub emissive: Option<RawColorFieldSpec>,
    #[serde(default)]
    pub opacity: Option<RawScalarFieldSpec>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawBlendSpec {
    #[serde(default)]
    pub albedo: Option<String>,
    #[serde(default)]
    pub height: Option<String>,
    #[serde(default)]
    pub roughness: Option<String>,
    #[serde(default)]
    pub ao: Option<String>,
    #[serde(default)]
    pub metallic: Option<String>,
    #[serde(default)]
    pub emissive: Option<String>,
    #[serde(default)]
    pub opacity: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawSignalSpec {
    pub kind: String,
    #[serde(default)]
    pub value: Option<f32>,
    #[serde(default)]
    pub noise: Option<RawNoiseSpec>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub curve: Option<RawCurveSpec>,
    #[serde(default)]
    pub clamp: Option<[f32; 2]>,
    #[serde(default)]
    pub op: Option<String>,
    #[serde(default)]
    pub inputs: Option<Vec<String>>,
    #[serde(default)]
    pub mask: Option<RawMaskSpec>,
    #[serde(default)]
    pub remap: Option<RawScalarRemapSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawColorFieldSpec {
    pub mode: String,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub noise: Option<RawNoiseSpec>,
    #[serde(default)]
    pub palette: Option<RawPalette>,
    #[serde(default)]
    pub mapping: Option<RawColorMappingSpec>,
    #[serde(default)]
    pub intensity: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawScalarFieldSpec {
    pub mode: String,
    #[serde(default)]
    pub value: Option<f32>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub noise: Option<RawNoiseSpec>,
    #[serde(default)]
    pub remap: Option<RawScalarRemapSpec>,
}

pub type RawPalette = Vec<RawPaletteItem>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RawPaletteItem {
    Color(String),
    Stop(RawPaletteStop),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawPaletteStop {
    pub at: f32,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawColorMappingSpec {
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub levels: Option<u32>,
    #[serde(default)]
    pub contrast: Option<f32>,
    #[serde(default)]
    pub bias: Option<f32>,
    #[serde(default)]
    pub invert: Option<bool>,
    #[serde(default)]
    pub dither: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawScalarRemapSpec {
    #[serde(default)]
    pub range: Option<[f32; 2]>,
    #[serde(default)]
    pub contrast: Option<f32>,
    #[serde(default)]
    pub bias: Option<f32>,
    #[serde(default)]
    pub invert: Option<bool>,
    #[serde(default)]
    pub curve: Option<RawCurveSpec>,
    #[serde(default)]
    pub clamp: Option<[f32; 2]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawCurveSpec {
    pub interp: String,
    pub stops: Vec<[f32; 2]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawNoiseSpec {
    pub basis: String,
    pub fractal: String,
    #[serde(default)]
    pub cellular_return: Option<String>,
    pub scale: f32,
    #[serde(default)]
    pub stretch: Option<[f32; 3]>,
    #[serde(default)]
    pub octaves: Option<u32>,
    #[serde(default)]
    pub lacunarity: Option<f32>,
    #[serde(default)]
    pub gain: Option<f32>,
    #[serde(default)]
    pub offset: Option<[f32; 3]>,
    #[serde(default)]
    pub seed_offset: Option<u32>,
    #[serde(default)]
    pub warp: Option<RawWarpSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawWarpSpec {
    #[serde(default)]
    pub amplitude: Option<f32>,
    #[serde(default)]
    pub basis: Option<String>,
    #[serde(default)]
    pub fractal: Option<String>,
    #[serde(default)]
    pub scale_multiplier: Option<f32>,
    #[serde(default)]
    pub octaves: Option<u32>,
    #[serde(default)]
    pub lacunarity: Option<f32>,
    #[serde(default)]
    pub gain: Option<f32>,
    #[serde(default)]
    pub seed_offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawMaskSpec {
    pub mode: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub noise: Option<RawNoiseSpec>,
    #[serde(default)]
    pub axis: Option<String>,
    #[serde(default)]
    pub from: Option<f32>,
    #[serde(default)]
    pub to: Option<f32>,
    #[serde(default)]
    pub falloff: Option<f32>,
    #[serde(default)]
    pub invert: Option<bool>,
    #[serde(default)]
    pub jitter: Option<RawMaskJitter>,
    #[serde(default)]
    pub threshold: Option<f32>,
    #[serde(default)]
    pub softness: Option<f32>,
    #[serde(default)]
    pub items: Option<Vec<RawMaskSpec>>,
    #[serde(default)]
    pub item: Option<Box<RawMaskSpec>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawMaskJitter {
    #[serde(default)]
    pub amount: Option<f32>,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawNormalSpec {
    pub mode: String,
    #[serde(default)]
    pub strength: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawSamplerSpec {
    #[serde(default)]
    pub mag_filter: Option<String>,
    #[serde(default)]
    pub min_filter: Option<String>,
    #[serde(default)]
    pub mipmap_filter: Option<String>,
    #[serde(default)]
    pub anisotropy: Option<u8>,
    #[serde(default)]
    pub address_u: Option<String>,
    #[serde(default)]
    pub address_v: Option<String>,
}
