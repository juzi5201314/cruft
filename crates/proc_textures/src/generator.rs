use std::collections::{BTreeMap, HashMap};

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor,
    TextureViewDimension,
};
use noise::{NoiseFn, Perlin, Value, Worley};

use crate::compiler::{
    ChannelLayerRef, CompiledColorField, CompiledLayer, CompiledLayerFields, CompiledScalarField,
    CompiledSignalKind, CompiledTextureSet, CubeFace, CurveInterpolation, CurveSpec, DitherMode,
    FaceChannels, FaceTransform, MaskAxis, MaskSpec, MipmapMode, NoiseBasis, NoiseFractal,
    NoiseSpec, NormalFormat, NormalMode, OutputSpec, PaletteStop, ResolvedTexture, ScalarBlendMode,
    ScalarFieldSource, ScalarRemapSpec, SignalOp, SurfaceDomain, TexturePackId, TextureRegistry,
    TextureRuntimePack, ThresholdSource, TileMode, WarpSpec,
};
use crate::error::TextureDataError;

const BAYER4: [[f32; 4]; 4] = [
    [0.5 / 16.0, 8.5 / 16.0, 2.5 / 16.0, 10.5 / 16.0],
    [12.5 / 16.0, 4.5 / 16.0, 14.5 / 16.0, 6.5 / 16.0],
    [3.5 / 16.0, 11.5 / 16.0, 1.5 / 16.0, 9.5 / 16.0],
    [15.5 / 16.0, 7.5 / 16.0, 13.5 / 16.0, 5.5 / 16.0],
];

#[derive(Debug, Clone)]
pub struct GeneratedTextureSet {
    pub output: OutputSpec,
    pub layers: Vec<GeneratedFaceLayer>,
}

#[derive(Debug, Clone)]
pub struct GeneratedFaceLayer {
    pub surface: String,
    pub face: CubeFace,
    pub transform: FaceTransform,
    pub albedo: Vec<MipLevel<Vec4>>,
    pub normal: Vec<MipLevel<Vec3>>,
    pub orm: Vec<MipLevel<Vec4>>,
    pub emissive: Vec<MipLevel<Vec4>>,
    pub height: Vec<MipLevel<f32>>,
}

#[derive(Debug, Clone)]
pub struct MipLevel<T> {
    pub size: u32,
    pub data: Vec<T>,
}

#[derive(Debug, Clone)]
pub struct RuntimeTextureBuild {
    pub generated: GeneratedTextureSet,
    pub registry: TextureRegistry,
    pub pack_data: RuntimeTexturePackData,
}

#[derive(Debug, Clone)]
pub struct RuntimeTexturePackData {
    pub pack_id: TexturePackId,
    pub layer_count: u32,
    pub mip_level_count: u32,
    pub albedo_bytes: Vec<u8>,
    pub normal_bytes: Vec<u8>,
    pub orm_bytes: Vec<u8>,
    pub emissive_bytes: Vec<u8>,
    pub height_bytes: Vec<u8>,
}

impl RuntimeTexturePackData {
    pub fn create_runtime_pack(&self, images: &mut Assets<Image>) -> TextureRuntimePack {
        let size = Extent3d {
            width: self.base_size(),
            height: self.base_size(),
            depth_or_array_layers: self.layer_count,
        };
        let albedo = images.add(build_array_image(
            size,
            self.mip_level_count,
            TextureFormat::Rgba8UnormSrgb,
            self.albedo_bytes.clone(),
        ));
        let normal = images.add(build_array_image(
            size,
            self.mip_level_count,
            TextureFormat::Rgba8Unorm,
            self.normal_bytes.clone(),
        ));
        let orm = images.add(build_array_image(
            size,
            self.mip_level_count,
            TextureFormat::Rgba8Unorm,
            self.orm_bytes.clone(),
        ));
        let emissive = images.add(build_array_image(
            size,
            self.mip_level_count,
            TextureFormat::Rgba8UnormSrgb,
            self.emissive_bytes.clone(),
        ));
        let height = images.add(build_array_image(
            size,
            self.mip_level_count,
            TextureFormat::R16Unorm,
            self.height_bytes.clone(),
        ));
        TextureRuntimePack {
            id: self.pack_id,
            layer_count: self.layer_count,
            mip_level_count: self.mip_level_count,
            albedo,
            normal,
            orm,
            emissive,
            height,
        }
    }

    fn base_size(&self) -> u32 {
        let mut size = 1;
        for _ in 1..self.mip_level_count {
            size *= 2;
        }
        size
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct FaceLayerKey {
    surface: String,
    face: CubeFace,
    transform: FaceTransform,
}

#[derive(Debug, Clone, Copy)]
struct PixelCoord {
    u: f32,
    v: f32,
    x: f32,
    y: f32,
    z: f32,
}

struct EvalContext<'a> {
    surface: &'a crate::compiler::CompiledSurface,
    face: CubeFace,
    signal_cache: HashMap<String, Vec<f32>>,
}

pub fn build_runtime_texture_assets(
    compiled: &CompiledTextureSet,
) -> Result<RuntimeTextureBuild, TextureDataError> {
    let mut unique_faces = Vec::new();
    let mut face_indices = HashMap::new();
    for texture in compiled.textures.values() {
        for face in CubeFace::ALL {
            let binding = texture
                .faces
                .get(&face)
                .expect("face resolution is complete");
            let key = FaceLayerKey {
                surface: binding.surface.clone(),
                face,
                transform: binding.transform,
            };
            if !face_indices.contains_key(&key) {
                let index = unique_faces.len() as u16;
                face_indices.insert(key.clone(), index);
                unique_faces.push(key);
            }
        }
    }

    let mut layers = Vec::with_capacity(unique_faces.len());
    for key in &unique_faces {
        let surface = compiled
            .surfaces
            .get(&key.surface)
            .expect("resolved surface must exist");
        layers.push(generate_face_layer(
            compiled,
            &key.surface,
            surface,
            key.face,
            key.transform,
        )?);
    }

    let mut registry = TextureRegistry {
        textures: BTreeMap::new(),
        fingerprint: Some(compiled.fingerprint.clone()),
    };
    for (name, texture) in &compiled.textures {
        let face_ref = |face: CubeFace| -> FaceChannels {
            let binding = texture.faces.get(&face).expect("face is resolved");
            let index = face_indices[&FaceLayerKey {
                surface: binding.surface.clone(),
                face,
                transform: binding.transform,
            }];
            let layer_ref = |layer_index| ChannelLayerRef {
                pack_id: TexturePackId(0),
                layer_index,
            };
            FaceChannels {
                albedo: layer_ref(index),
                normal: layer_ref(index),
                orm: layer_ref(index),
                emissive: layer_ref(index),
                height: layer_ref(index),
            }
        };
        registry.textures.insert(
            name.clone(),
            ResolvedTexture {
                sampler: texture.sampler,
                alpha_mode: texture.alpha_mode,
                cutout_threshold: texture.cutout_threshold,
                top: face_ref(CubeFace::Top),
                bottom: face_ref(CubeFace::Bottom),
                north: face_ref(CubeFace::North),
                south: face_ref(CubeFace::South),
                east: face_ref(CubeFace::East),
                west: face_ref(CubeFace::West),
            },
        );
    }

    let output = compiled.output.clone();
    let mip_level_count = match output.mipmaps {
        MipmapMode::None => 1,
        MipmapMode::Full => output.size.ilog2() + 1,
    };
    let pack_data = RuntimeTexturePackData {
        pack_id: TexturePackId(0),
        layer_count: layers.len() as u32,
        mip_level_count,
        albedo_bytes: encode_rgba8_srgb_layers(&layers, |layer, mip| &layer.albedo[mip].data),
        normal_bytes: encode_normal_layers(&layers, compiled.output.normal_format, |layer, mip| {
            &layer.normal[mip].data
        }),
        orm_bytes: encode_rgba8_linear_layers(&layers, |layer, mip| &layer.orm[mip].data),
        emissive_bytes: encode_rgba8_srgb_layers(&layers, |layer, mip| &layer.emissive[mip].data),
        height_bytes: encode_r16_layers(&layers, |layer, mip| &layer.height[mip].data),
    };
    Ok(RuntimeTextureBuild {
        generated: GeneratedTextureSet { output, layers },
        registry,
        pack_data,
    })
}

fn build_array_image(
    size: Extent3d,
    mip_level_count: u32,
    format: TextureFormat,
    data: Vec<u8>,
) -> Image {
    let mut image = Image::new(
        size,
        TextureDimension::D2,
        data,
        format,
        RenderAssetUsages::default(),
    );
    image.texture_descriptor.mip_level_count = mip_level_count;
    image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::D2Array),
        ..default()
    });
    image.copy_on_resize = false;
    image
}

fn generate_face_layer(
    compiled: &CompiledTextureSet,
    surface_name: &str,
    surface: &crate::compiler::CompiledSurface,
    face: CubeFace,
    transform: FaceTransform,
) -> Result<GeneratedFaceLayer, TextureDataError> {
    let size = compiled.output.size;
    let ctx = EvalContext {
        surface,
        face,
        signal_cache: HashMap::new(),
    };
    let pixel_count = (size * size) as usize;
    let coords = build_coords(surface, face, transform, size);

    let mut albedo = vec![Vec4::ZERO; pixel_count];
    let mut height = vec![0.5f32; pixel_count];
    let mut roughness = vec![1.0f32; pixel_count];
    let mut ao = vec![1.0f32; pixel_count];
    let mut metallic = vec![0.0f32; pixel_count];
    let mut emissive = vec![Vec4::ZERO; pixel_count];
    let mut opacity = vec![1.0f32; pixel_count];

    apply_layer_fields(
        &ctx,
        &coords,
        &surface.base,
        &mut albedo,
        &mut height,
        &mut roughness,
        &mut ao,
        &mut metallic,
        &mut emissive,
        &mut opacity,
    );

    for layer in &surface.layers {
        let coverage = evaluate_mask_raster(&ctx, &coords, &layer.mask)
            .into_iter()
            .map(|value| (value * layer.strength).clamp(0.0, 1.0))
            .collect::<Vec<_>>();
        apply_partial_layer(
            &ctx,
            &coords,
            layer,
            &coverage,
            &mut albedo,
            &mut height,
            &mut roughness,
            &mut ao,
            &mut metallic,
            &mut emissive,
            &mut opacity,
        );
    }

    let normal_base = match surface.normal.mode {
        NormalMode::Flat => vec![Vec3::new(0.0, 0.0, 1.0); pixel_count],
        NormalMode::DeriveFromHeight => derive_normal_from_height(
            &height,
            size,
            surface.tile_mode,
            compiled.output.normal_format,
            surface.normal.strength,
        ),
    };

    let albedo_mips = build_mips_vec4(&albedo, size, compiled.output.mipmaps, true);
    let normal_mips = build_mips_normal(&normal_base, size, compiled.output.mipmaps);
    let orm_base = build_orm_base(&ao, &roughness, &metallic, &opacity);
    let orm_mips = build_mips_vec4(&orm_base, size, compiled.output.mipmaps, false);
    let emissive_mips = build_mips_vec4(&emissive, size, compiled.output.mipmaps, true);
    let height_mips = build_mips_scalar(
        &height,
        size,
        compiled.output.mipmaps,
        surface.tile_mode,
        opacity.as_slice(),
    );

    Ok(GeneratedFaceLayer {
        surface: surface_name.to_string(),
        face,
        transform,
        albedo: albedo_mips,
        normal: normal_mips,
        orm: orm_mips,
        emissive: emissive_mips,
        height: height_mips,
    })
}

fn build_coords(
    surface: &crate::compiler::CompiledSurface,
    face: CubeFace,
    transform: FaceTransform,
    size: u32,
) -> Vec<PixelCoord> {
    let mut coords = Vec::with_capacity((size * size) as usize);
    let inv_size = 1.0 / size as f32;
    for y in 0..size {
        for x in 0..size {
            let mut u = (x as f32 + 0.5) * inv_size;
            let mut v = (y as f32 + 0.5) * inv_size;
            if surface.domain == SurfaceDomain::FaceUv {
                (u, v) = apply_face_transform(u, v, transform);
            }
            let (bx, by, bz) = block_space_coords(face, u, v);
            coords.push(PixelCoord {
                u,
                v,
                x: bx,
                y: by,
                z: bz,
            });
        }
    }
    coords
}

fn apply_face_transform(mut u: f32, mut v: f32, transform: FaceTransform) -> (f32, f32) {
    match transform.rotate {
        90 => {
            let next_u = 1.0 - v;
            let next_v = u;
            u = next_u;
            v = next_v;
        }
        180 => {
            u = 1.0 - u;
            v = 1.0 - v;
        }
        270 => {
            let next_u = v;
            let next_v = 1.0 - u;
            u = next_u;
            v = next_v;
        }
        _ => {}
    }
    if transform.flip_u {
        u = 1.0 - u;
    }
    if transform.flip_v {
        v = 1.0 - v;
    }
    (u, v)
}

fn block_space_coords(face: CubeFace, u: f32, v: f32) -> (f32, f32, f32) {
    match face {
        CubeFace::Top => (u, 1.0, v),
        CubeFace::Bottom => (u, 0.0, 1.0 - v),
        CubeFace::North => (u, 1.0 - v, 0.0),
        CubeFace::South => (1.0 - u, 1.0 - v, 1.0),
        CubeFace::East => (1.0, 1.0 - v, 1.0 - u),
        CubeFace::West => (0.0, 1.0 - v, u),
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_layer_fields(
    ctx: &EvalContext<'_>,
    coords: &[PixelCoord],
    fields: &CompiledLayerFields,
    albedo: &mut [Vec4],
    height: &mut [f32],
    roughness: &mut [f32],
    ao: &mut [f32],
    metallic: &mut [f32],
    emissive: &mut [Vec4],
    opacity: &mut [f32],
) {
    let base_albedo = evaluate_color_field(ctx, coords, &fields.albedo);
    albedo.copy_from_slice(&base_albedo);
    if let Some(field) = &fields.height {
        let values = evaluate_scalar_field(ctx, coords, field);
        height.copy_from_slice(&values);
    }
    if let Some(field) = &fields.roughness {
        let values = evaluate_scalar_field(ctx, coords, field);
        roughness.copy_from_slice(&values);
    }
    if let Some(field) = &fields.ao {
        let values = evaluate_scalar_field(ctx, coords, field);
        ao.copy_from_slice(&values);
    }
    if let Some(field) = &fields.metallic {
        let values = evaluate_scalar_field(ctx, coords, field);
        metallic.copy_from_slice(&values);
    }
    if let Some(field) = &fields.emissive {
        let values = evaluate_color_field(ctx, coords, field);
        emissive.copy_from_slice(&values);
    }
    if let Some(field) = &fields.opacity {
        let values = evaluate_scalar_field(ctx, coords, field);
        opacity.copy_from_slice(&values);
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_partial_layer(
    ctx: &EvalContext<'_>,
    coords: &[PixelCoord],
    layer: &CompiledLayer,
    coverage: &[f32],
    albedo: &mut [Vec4],
    height: &mut [f32],
    roughness: &mut [f32],
    ao: &mut [f32],
    metallic: &mut [f32],
    emissive: &mut [Vec4],
    opacity: &mut [f32],
) {
    if let Some(field) = &layer.fields.albedo {
        let layer_values = evaluate_color_field(ctx, coords, field);
        for ((out, layer_value), coverage) in albedo.iter_mut().zip(layer_values).zip(coverage) {
            *out = out.lerp(layer_value, *coverage);
        }
    }
    if let Some(field) = &layer.fields.height {
        let layer_values = evaluate_scalar_field(ctx, coords, field);
        blend_scalar_layer(height, &layer_values, coverage, layer.blend.height);
    }
    if let Some(field) = &layer.fields.roughness {
        let layer_values = evaluate_scalar_field(ctx, coords, field);
        blend_scalar_layer(roughness, &layer_values, coverage, layer.blend.roughness);
    }
    if let Some(field) = &layer.fields.ao {
        let layer_values = evaluate_scalar_field(ctx, coords, field);
        blend_scalar_layer(ao, &layer_values, coverage, layer.blend.ao);
    }
    if let Some(field) = &layer.fields.metallic {
        let layer_values = evaluate_scalar_field(ctx, coords, field);
        blend_scalar_layer(metallic, &layer_values, coverage, layer.blend.metallic);
    }
    if let Some(field) = &layer.fields.emissive {
        let layer_values = evaluate_color_field(ctx, coords, field);
        for ((out, layer_value), coverage) in emissive.iter_mut().zip(layer_values).zip(coverage) {
            *out = out.lerp(layer_value, *coverage);
        }
    }
    if let Some(field) = &layer.fields.opacity {
        let layer_values = evaluate_scalar_field(ctx, coords, field);
        blend_scalar_layer(opacity, &layer_values, coverage, layer.blend.opacity);
    }
    for values in [height, roughness, ao, metallic, opacity] {
        for value in values.iter_mut() {
            *value = value.clamp(0.0, 1.0);
        }
    }
    for color in albedo.iter_mut() {
        color.x = color.x.clamp(0.0, 1.0);
        color.y = color.y.clamp(0.0, 1.0);
        color.z = color.z.clamp(0.0, 1.0);
        color.w = 1.0;
    }
    for color in emissive.iter_mut() {
        color.x = color.x.max(0.0);
        color.y = color.y.max(0.0);
        color.z = color.z.max(0.0);
        color.w = 1.0;
    }
}

fn blend_scalar_layer(target: &mut [f32], layer: &[f32], coverage: &[f32], mode: ScalarBlendMode) {
    for ((out, layer_value), coverage) in target.iter_mut().zip(layer).zip(coverage) {
        *out = match mode {
            ScalarBlendMode::Mix => out.lerp(*layer_value, *coverage),
            ScalarBlendMode::Add => *out + *layer_value * *coverage,
            ScalarBlendMode::Multiply => *out * (1.0f32.lerp(*layer_value, *coverage)),
            ScalarBlendMode::Max => out.max(*layer_value * *coverage),
            ScalarBlendMode::Min => out.min(1.0f32.lerp(*layer_value, *coverage)),
        };
    }
}

fn evaluate_color_field(
    ctx: &EvalContext<'_>,
    coords: &[PixelCoord],
    field: &CompiledColorField,
) -> Vec<Vec4> {
    match field.source {
        crate::compiler::ColorFieldSource::Constant => {
            let [r, g, b] = field.constant.expect("constant color exists");
            vec![Vec4::new(r, g, b, 1.0); coords.len()]
        }
        crate::compiler::ColorFieldSource::Signal(ref source) => {
            let signal = evaluate_signal_raster(ctx, source, coords);
            let mapping = field.mapping.clone();
            signal
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    let mapped = apply_color_mapping(
                        *value,
                        index,
                        mapping.clone(),
                        field.palette.as_ref().expect("palette exists"),
                    );
                    Vec4::new(
                        mapped[0] * field.intensity,
                        mapped[1] * field.intensity,
                        mapped[2] * field.intensity,
                        1.0,
                    )
                })
                .collect()
        }
        crate::compiler::ColorFieldSource::InlineNoise(ref noise) => {
            let signal = evaluate_inline_noise_raster(ctx, coords, noise, None);
            let mapping = field.mapping.clone();
            signal
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    let mapped = apply_color_mapping(
                        *value,
                        index,
                        mapping.clone(),
                        field.palette.as_ref().expect("palette exists"),
                    );
                    Vec4::new(
                        mapped[0] * field.intensity,
                        mapped[1] * field.intensity,
                        mapped[2] * field.intensity,
                        1.0,
                    )
                })
                .collect()
        }
    }
}

fn evaluate_scalar_field(
    ctx: &EvalContext<'_>,
    coords: &[PixelCoord],
    field: &CompiledScalarField,
) -> Vec<f32> {
    match field.source {
        ScalarFieldSource::Constant => {
            vec![field.constant.expect("constant scalar exists"); coords.len()]
        }
        ScalarFieldSource::Signal(ref source) => {
            let signal = evaluate_signal_raster(ctx, source, coords);
            signal
                .iter()
                .map(|value| {
                    apply_scalar_remap(*value, field.remap.as_ref().expect("remap exists"))
                })
                .collect()
        }
        ScalarFieldSource::InlineNoise(ref noise) => {
            let signal = evaluate_inline_noise_raster(ctx, coords, noise, None);
            signal
                .iter()
                .map(|value| {
                    apply_scalar_remap(*value, field.remap.as_ref().expect("remap exists"))
                })
                .collect()
        }
    }
}

fn evaluate_signal_raster(ctx: &EvalContext<'_>, name: &str, coords: &[PixelCoord]) -> Vec<f32> {
    if let Some(values) = ctx.signal_cache.get(name) {
        return values.clone();
    }
    let signal = ctx.surface.signals.get(name).expect("signal exists");
    let values = match &signal.kind {
        CompiledSignalKind::Constant { value } => vec![*value; coords.len()],
        CompiledSignalKind::Noise { noise, remap } => {
            evaluate_inline_noise_raster(ctx, coords, noise, Some(remap))
        }
        CompiledSignalKind::Curve {
            source,
            curve,
            clamp,
        } => evaluate_signal_raster(ctx, source, coords)
            .into_iter()
            .map(|value| {
                let curved = apply_curve(value, curve);
                curved.clamp(clamp[0], clamp[1])
            })
            .collect(),
        CompiledSignalKind::Combine { op, inputs, clamp } => {
            let inputs = inputs
                .iter()
                .map(|input| evaluate_signal_raster(ctx, input, coords))
                .collect::<Vec<_>>();
            let mut values = vec![0.0; coords.len()];
            for (index, out) in values.iter_mut().enumerate() {
                let operands = inputs
                    .iter()
                    .map(|values| values[index])
                    .collect::<Vec<_>>();
                let combined = match op {
                    SignalOp::Add => operands.iter().copied().sum(),
                    SignalOp::Multiply => operands.iter().copied().product(),
                    SignalOp::Min => operands.iter().copied().fold(f32::INFINITY, f32::min),
                    SignalOp::Max => operands.iter().copied().fold(f32::NEG_INFINITY, f32::max),
                    SignalOp::Subtract => (operands[0] - operands[1]).max(0.0),
                    SignalOp::Average => {
                        operands.iter().copied().sum::<f32>() / operands.len() as f32
                    }
                };
                *out = combined.clamp(clamp[0], clamp[1]);
            }
            values
        }
        CompiledSignalKind::Mask { mask } => evaluate_mask_raster(ctx, coords, mask),
    };
    values
}

fn evaluate_mask_raster(ctx: &EvalContext<'_>, coords: &[PixelCoord], mask: &MaskSpec) -> Vec<f32> {
    match mask {
        MaskSpec::Full => vec![1.0; coords.len()],
        MaskSpec::Signal { source } => evaluate_signal_raster(ctx, source, coords),
        MaskSpec::AxisBand {
            axis,
            from,
            to,
            falloff,
            invert,
            jitter,
        } => coords
            .iter()
            .enumerate()
            .map(|(index, coord)| {
                let axis_value = match axis {
                    MaskAxis::U => coord.u,
                    MaskAxis::V => coord.v,
                    MaskAxis::X => coord.x,
                    MaskAxis::Y => coord.y,
                    MaskAxis::Z => coord.z,
                };
                let jitter_value = jitter
                    .as_ref()
                    .map(|spec| {
                        let source = spec
                            .source
                            .as_ref()
                            .map(|name| evaluate_signal_raster(ctx, name, coords)[index])
                            .unwrap_or(0.5);
                        (source * 2.0 - 1.0) * spec.amount
                    })
                    .unwrap_or(0.0);
                let lower = *from + jitter_value;
                let upper = *to + jitter_value;
                let mask = if *falloff == 0.0 {
                    if axis_value >= lower && axis_value <= upper {
                        1.0
                    } else {
                        0.0
                    }
                } else {
                    let enter = smoothstep(lower - *falloff, lower, axis_value);
                    let exit = 1.0 - smoothstep(upper, upper + *falloff, axis_value);
                    (enter * exit).clamp(0.0, 1.0)
                };
                if *invert {
                    1.0 - mask
                } else {
                    mask
                }
            })
            .collect(),
        MaskSpec::Threshold {
            source,
            threshold,
            softness,
            invert,
        } => {
            let source_values = match source {
                ThresholdSource::Signal(name) => evaluate_signal_raster(ctx, name, coords),
                ThresholdSource::InlineNoise(noise) => {
                    evaluate_inline_noise_raster(ctx, coords, noise, None)
                }
            };
            source_values
                .into_iter()
                .map(|value| {
                    let mask = if *softness == 0.0 {
                        if value >= *threshold {
                            1.0
                        } else {
                            0.0
                        }
                    } else {
                        smoothstep(
                            *threshold - *softness * 0.5,
                            *threshold + *softness * 0.5,
                            value,
                        )
                    };
                    if *invert {
                        1.0 - mask
                    } else {
                        mask
                    }
                })
                .collect()
        }
        MaskSpec::EdgeDistance {
            from,
            to,
            falloff,
            invert,
        } => coords
            .iter()
            .map(|coord| {
                let distance = coord.u.min(1.0 - coord.u).min(coord.v.min(1.0 - coord.v));
                let mut mask = band_mask(distance, *from, *to, *falloff);
                if *invert {
                    mask = 1.0 - mask;
                }
                mask
            })
            .collect(),
        MaskSpec::And { items } => combine_masks(ctx, coords, items, f32::min, 1.0),
        MaskSpec::Or { items } => combine_masks(ctx, coords, items, f32::max, 0.0),
        MaskSpec::Subtract { items } => {
            let a = evaluate_mask_raster(ctx, coords, &items[0]);
            let b = evaluate_mask_raster(ctx, coords, &items[1]);
            a.into_iter()
                .zip(b)
                .map(|(a, b)| (a - b).max(0.0))
                .collect()
        }
        MaskSpec::Not { item } => evaluate_mask_raster(ctx, coords, item)
            .into_iter()
            .map(|value| 1.0 - value)
            .collect(),
    }
}

fn combine_masks(
    ctx: &EvalContext<'_>,
    coords: &[PixelCoord],
    items: &[MaskSpec],
    reducer: fn(f32, f32) -> f32,
    initial: f32,
) -> Vec<f32> {
    let item_values = items
        .iter()
        .map(|item| evaluate_mask_raster(ctx, coords, item))
        .collect::<Vec<_>>();
    let mut values = vec![initial; coords.len()];
    for index in 0..coords.len() {
        values[index] = item_values
            .iter()
            .fold(initial, |acc, item| reducer(acc, item[index]));
    }
    values
}

fn evaluate_inline_noise_raster(
    ctx: &EvalContext<'_>,
    coords: &[PixelCoord],
    noise: &NoiseSpec,
    remap: Option<&crate::compiler::ScalarRemapSpec>,
) -> Vec<f32> {
    coords
        .iter()
        .map(|coord| {
            let base = sample_noise(ctx.surface, noise, coord, ctx.face);
            match remap {
                Some(remap) => apply_scalar_remap(base, remap),
                None => base.clamp(0.0, 1.0),
            }
        })
        .collect()
}

fn sample_noise(
    surface: &crate::compiler::CompiledSurface,
    noise: &NoiseSpec,
    coord: &PixelCoord,
    face: CubeFace,
) -> f32 {
    let mut point = match surface.domain {
        SurfaceDomain::FaceUv => Vec3::new(coord.u, coord.v, 1.0),
        SurfaceDomain::BlockSpace => Vec3::new(coord.x, coord.y, coord.z),
    };
    if surface.pixel_snap {
        point = snap_point(point, surface.logical_size, surface.domain);
    }
    point += Vec3::new(noise.offset[0], noise.offset[1], noise.offset[2]);
    point = Vec3::new(
        point.x * noise.scale * noise.stretch[0],
        point.y * noise.scale * noise.stretch[1],
        point.z * noise.scale * noise.stretch[2],
    );
    if noise.warp.amplitude > 0.0 {
        let warp = sample_warp_vector(point, &noise.warp, noise.seed_offset ^ surface.seed());
        point += warp * noise.warp.amplitude;
    }
    if surface.tile_mode == TileMode::Repeat {
        point = Vec3::new(point.x.fract(), point.y.fract(), point.z.fract());
    }
    sample_noise_value(noise, point, surface.seed() ^ face_seed(face))
}

fn sample_warp_vector(point: Vec3, warp: &WarpSpec, seed: u32) -> Vec3 {
    Vec3::new(
        sample_noise_value_from_params(
            warp.basis,
            warp.fractal,
            point * warp.scale_multiplier,
            warp.octaves,
            warp.lacunarity,
            warp.gain,
            seed ^ warp.seed_offset ^ 0x9E37_79B9,
        ),
        sample_noise_value_from_params(
            warp.basis,
            warp.fractal,
            point * warp.scale_multiplier,
            warp.octaves,
            warp.lacunarity,
            warp.gain,
            seed ^ warp.seed_offset ^ 0x243F_6A88,
        ),
        sample_noise_value_from_params(
            warp.basis,
            warp.fractal,
            point * warp.scale_multiplier,
            warp.octaves,
            warp.lacunarity,
            warp.gain,
            seed ^ warp.seed_offset ^ 0xB7E1_5163,
        ),
    ) * 2.0
        - Vec3::ONE
}

fn sample_noise_value(noise: &NoiseSpec, point: Vec3, seed: u32) -> f32 {
    sample_noise_value_from_params(
        noise.basis,
        noise.fractal,
        point,
        noise.octaves,
        noise.lacunarity,
        noise.gain,
        seed ^ noise.seed_offset,
    )
}

fn sample_noise_value_from_params(
    basis: NoiseBasis,
    fractal: NoiseFractal,
    point: Vec3,
    octaves: u32,
    lacunarity: f32,
    gain: f32,
    seed: u32,
) -> f32 {
    let mut amplitude = 1.0f64;
    let mut frequency = 1.0f64;
    let mut sum = 0.0f64;
    let mut norm = 0.0f64;
    for octave in 0..octaves {
        let sample_point = [
            point.x as f64 * frequency,
            point.y as f64 * frequency,
            point.z as f64 * frequency,
        ];
        let mut value = sample_basis(basis, seed.wrapping_add(octave), sample_point);
        value = match fractal {
            NoiseFractal::None | NoiseFractal::Fbm => value,
            NoiseFractal::Billow => (value * 2.0 - 1.0).abs() * 0.5 + 0.5,
            NoiseFractal::Ridged => 1.0 - (value * 2.0 - 1.0).abs(),
        };
        sum += value * amplitude;
        norm += amplitude;
        amplitude *= gain as f64;
        frequency *= lacunarity as f64;
    }
    if norm == 0.0 {
        return 0.0;
    }
    (sum / norm).clamp(0.0, 1.0) as f32
}

fn sample_basis(basis: NoiseBasis, seed: u32, point: [f64; 3]) -> f64 {
    let raw = match basis {
        NoiseBasis::Value => Value::new(seed).get(point),
        NoiseBasis::Gradient => Perlin::new(seed).get(point),
        NoiseBasis::Cellular => Worley::new(seed).get(point),
    };
    ((raw + 1.0) * 0.5).clamp(0.0, 1.0)
}

fn apply_color_mapping(
    value: f32,
    index: usize,
    mapping: crate::compiler::ColorMappingSpec,
    palette: &[PaletteStop],
) -> [f32; 3] {
    let mapped = apply_mapping_common(value, mapping.contrast, mapping.bias, mapping.invert);
    match mapping.mode {
        crate::compiler::ColorMappingMode::Gradient => sample_palette_gradient(palette, mapped),
        crate::compiler::ColorMappingMode::Quantized => {
            let levels = if mapping.levels == 0 {
                palette.len().max(2) as u32
            } else {
                mapping.levels
            };
            let mut quantized = mapped;
            if mapping.dither == DitherMode::Bayer4 {
                let x = index % 4;
                let y = (index / 4) % 4;
                quantized = (quantized + (BAYER4[y][x] - 0.5) / levels as f32).clamp(0.0, 1.0);
            }
            let discrete = if levels <= 1 {
                0.0
            } else {
                let idx = (quantized * levels as f32).floor().min(levels as f32 - 1.0);
                idx / (levels as f32 - 1.0)
            };
            sample_palette_gradient(palette, discrete)
        }
    }
}

fn apply_mapping_common(value: f32, contrast: f32, bias: f32, invert: bool) -> f32 {
    let mut value = if invert { 1.0 - value } else { value };
    value = (value - 0.5) * contrast + 0.5 + bias;
    value.clamp(0.0, 1.0)
}

fn sample_palette_gradient(palette: &[PaletteStop], value: f32) -> [f32; 3] {
    let value = value.clamp(0.0, 1.0);
    for window in palette.windows(2) {
        let [a, b] = window else { continue };
        if value >= a.at && value <= b.at {
            let t = if (b.at - a.at).abs() <= f32::EPSILON {
                0.0
            } else {
                (value - a.at) / (b.at - a.at)
            };
            return [
                a.color[0] + (b.color[0] - a.color[0]) * t,
                a.color[1] + (b.color[1] - a.color[1]) * t,
                a.color[2] + (b.color[2] - a.color[2]) * t,
            ];
        }
    }
    palette
        .last()
        .map(|stop| stop.color)
        .unwrap_or([0.0, 0.0, 0.0])
}

fn apply_scalar_remap(value: f32, remap: &ScalarRemapSpec) -> f32 {
    let value = apply_mapping_common(value, remap.contrast, remap.bias, remap.invert);
    let value = apply_curve(value, &remap.curve);
    let ranged = remap.range[0] + (remap.range[1] - remap.range[0]) * value;
    ranged.clamp(remap.clamp[0], remap.clamp[1])
}

fn apply_curve(value: f32, curve: &CurveSpec) -> f32 {
    let value = value.clamp(0.0, 1.0);
    for window in curve.stops.windows(2) {
        let [a, b] = window else { continue };
        if value >= a[0] && value <= b[0] {
            let mut t = if (b[0] - a[0]).abs() <= f32::EPSILON {
                0.0
            } else {
                (value - a[0]) / (b[0] - a[0])
            };
            if curve.interp == CurveInterpolation::Smoothstep {
                t = t * t * (3.0 - 2.0 * t);
            }
            return a[1] + (b[1] - a[1]) * t;
        }
    }
    curve.stops.last().map(|stop| stop[1]).unwrap_or(value)
}

fn band_mask(value: f32, from: f32, to: f32, falloff: f32) -> f32 {
    if falloff == 0.0 {
        if value >= from && value <= to {
            1.0
        } else {
            0.0
        }
    } else {
        let enter = smoothstep(from - falloff, from, value);
        let exit = 1.0 - smoothstep(to, to + falloff, value);
        (enter * exit).clamp(0.0, 1.0)
    }
}

fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    if (edge1 - edge0).abs() <= f32::EPSILON {
        return if value >= edge1 { 1.0 } else { 0.0 };
    }
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn snap_point(point: Vec3, logical_size: u32, domain: SurfaceDomain) -> Vec3 {
    let logical = logical_size as f32;
    let snap = |value: f32| ((value * logical).floor() + 0.5) / logical;
    match domain {
        SurfaceDomain::FaceUv => Vec3::new(snap(point.x), snap(point.y), point.z),
        SurfaceDomain::BlockSpace => Vec3::new(snap(point.x), snap(point.y), snap(point.z)),
    }
}

fn face_seed(face: CubeFace) -> u32 {
    match face {
        CubeFace::Top => 0xA1F0_0002,
        CubeFace::Bottom => 0xA1F0_0003,
        CubeFace::North => 0xA1F0_0005,
        CubeFace::South => 0xA1F0_0006,
        CubeFace::East => 0xA1F0_0007,
        CubeFace::West => 0xA1F0_0008,
    }
}

trait SurfaceSeed {
    fn seed(&self) -> u32;
}

impl SurfaceSeed for crate::compiler::CompiledSurface {
    fn seed(&self) -> u32 {
        self.seed
    }
}

fn derive_normal_from_height(
    height: &[f32],
    size: u32,
    tile_mode: TileMode,
    normal_format: NormalFormat,
    strength: f32,
) -> Vec<Vec3> {
    let mut normals = vec![Vec3::Z; height.len()];
    let sample = |x: i32, y: i32| -> f32 {
        let wrap = |value: i32| value.rem_euclid(size as i32) as usize;
        let clamp = |value: i32| value.clamp(0, size as i32 - 1) as usize;
        let (sx, sy) = match tile_mode {
            TileMode::Repeat => (wrap(x), wrap(y)),
            TileMode::Clamp => (clamp(x), clamp(y)),
        };
        height[sy * size as usize + sx]
    };
    for y in 0..size as i32 {
        for x in 0..size as i32 {
            let hx0 = sample(x - 1, y);
            let hx1 = sample(x + 1, y);
            let hy0 = sample(x, y - 1);
            let hy1 = sample(x, y + 1);
            let dx = hx1 - hx0;
            let dy = hy1 - hy0;
            let mut normal = Vec3::new(-dx * strength, -dy * strength, 1.0).normalize_or_zero();
            if normal_format == NormalFormat::OpenGl {
                normal.y = -normal.y;
            }
            normals[y as usize * size as usize + x as usize] = normal;
        }
    }
    normals
}

fn build_orm_base(ao: &[f32], roughness: &[f32], metallic: &[f32], opacity: &[f32]) -> Vec<Vec4> {
    ao.iter()
        .zip(roughness)
        .zip(metallic)
        .zip(opacity)
        .map(|(((ao, roughness), metallic), opacity)| {
            Vec4::new(*ao, *roughness, *metallic, *opacity)
        })
        .collect()
}

fn build_mips_vec4(
    base: &[Vec4],
    size: u32,
    mipmaps: MipmapMode,
    _linear_color: bool,
) -> Vec<MipLevel<Vec4>> {
    let mut levels = vec![MipLevel {
        size,
        data: base.to_vec(),
    }];
    if mipmaps == MipmapMode::None {
        return levels;
    }
    while levels.last().map(|mip| mip.size).unwrap_or(1) > 1 {
        let prev = levels.last().expect("at least one mip");
        levels.push(MipLevel {
            size: (prev.size / 2).max(1),
            data: downsample_vec4(&prev.data, prev.size),
        });
    }
    levels
}

fn build_mips_normal(base: &[Vec3], size: u32, mipmaps: MipmapMode) -> Vec<MipLevel<Vec3>> {
    let mut levels = vec![MipLevel {
        size,
        data: base.to_vec(),
    }];
    if mipmaps == MipmapMode::None {
        return levels;
    }
    while levels.last().map(|mip| mip.size).unwrap_or(1) > 1 {
        let prev = levels.last().expect("at least one mip");
        levels.push(MipLevel {
            size: (prev.size / 2).max(1),
            data: downsample_normal(&prev.data, prev.size),
        });
    }
    levels
}

fn build_mips_scalar(
    base: &[f32],
    size: u32,
    mipmaps: MipmapMode,
    _tile_mode: TileMode,
    _opacity_base: &[f32],
) -> Vec<MipLevel<f32>> {
    let mut levels = vec![MipLevel {
        size,
        data: base.to_vec(),
    }];
    if mipmaps == MipmapMode::None {
        return levels;
    }
    while levels.last().map(|mip| mip.size).unwrap_or(1) > 1 {
        let prev = levels.last().expect("at least one mip");
        levels.push(MipLevel {
            size: (prev.size / 2).max(1),
            data: downsample_scalar(&prev.data, prev.size),
        });
    }
    levels
}

fn downsample_vec4(data: &[Vec4], size: u32) -> Vec<Vec4> {
    let next_size = (size / 2).max(1);
    let mut out = vec![Vec4::ZERO; (next_size * next_size) as usize];
    for y in 0..next_size {
        for x in 0..next_size {
            let sx = x * 2;
            let sy = y * 2;
            let samples = [
                data[(sy * size + sx) as usize],
                data[(sy * size + (sx + 1).min(size - 1)) as usize],
                data[(((sy + 1).min(size - 1)) * size + sx) as usize],
                data[(((sy + 1).min(size - 1)) * size + (sx + 1).min(size - 1)) as usize],
            ];
            out[(y * next_size + x) as usize] =
                (samples[0] + samples[1] + samples[2] + samples[3]) * 0.25;
        }
    }
    out
}

fn downsample_normal(data: &[Vec3], size: u32) -> Vec<Vec3> {
    let next_size = (size / 2).max(1);
    let mut out = vec![Vec3::Z; (next_size * next_size) as usize];
    for y in 0..next_size {
        for x in 0..next_size {
            let sx = x * 2;
            let sy = y * 2;
            let samples = [
                data[(sy * size + sx) as usize],
                data[(sy * size + (sx + 1).min(size - 1)) as usize],
                data[(((sy + 1).min(size - 1)) * size + sx) as usize],
                data[(((sy + 1).min(size - 1)) * size + (sx + 1).min(size - 1)) as usize],
            ];
            out[(y * next_size + x) as usize] =
                (samples[0] + samples[1] + samples[2] + samples[3]).normalize_or_zero();
        }
    }
    out
}

fn downsample_scalar(data: &[f32], size: u32) -> Vec<f32> {
    let next_size = (size / 2).max(1);
    let mut out = vec![0.0; (next_size * next_size) as usize];
    for y in 0..next_size {
        for x in 0..next_size {
            let sx = x * 2;
            let sy = y * 2;
            let samples = [
                data[(sy * size + sx) as usize],
                data[(sy * size + (sx + 1).min(size - 1)) as usize],
                data[(((sy + 1).min(size - 1)) * size + sx) as usize],
                data[(((sy + 1).min(size - 1)) * size + (sx + 1).min(size - 1)) as usize],
            ];
            out[(y * next_size + x) as usize] =
                (samples[0] + samples[1] + samples[2] + samples[3]) * 0.25;
        }
    }
    out
}

fn encode_rgba8_srgb_layers(
    layers: &[GeneratedFaceLayer],
    getter: impl Fn(&GeneratedFaceLayer, usize) -> &Vec<Vec4>,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    for layer in layers {
        for mip in 0..layer.albedo.len() {
            for value in getter(layer, mip) {
                bytes.push(linear_to_srgb_u8(value.x));
                bytes.push(linear_to_srgb_u8(value.y));
                bytes.push(linear_to_srgb_u8(value.z));
                bytes.push((value.w.clamp(0.0, 1.0) * 255.0).round() as u8);
            }
        }
    }
    bytes
}

fn encode_rgba8_linear_layers(
    layers: &[GeneratedFaceLayer],
    getter: impl Fn(&GeneratedFaceLayer, usize) -> &Vec<Vec4>,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    for layer in layers {
        for mip in 0..layer.orm.len() {
            for value in getter(layer, mip) {
                bytes.push((value.x.clamp(0.0, 1.0) * 255.0).round() as u8);
                bytes.push((value.y.clamp(0.0, 1.0) * 255.0).round() as u8);
                bytes.push((value.z.clamp(0.0, 1.0) * 255.0).round() as u8);
                bytes.push((value.w.clamp(0.0, 1.0) * 255.0).round() as u8);
            }
        }
    }
    bytes
}

fn encode_normal_layers(
    layers: &[GeneratedFaceLayer],
    normal_format: NormalFormat,
    getter: impl Fn(&GeneratedFaceLayer, usize) -> &Vec<Vec3>,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    for layer in layers {
        for mip in 0..layer.normal.len() {
            for value in getter(layer, mip) {
                let encoded = encode_normal(*value, normal_format);
                bytes.push((encoded.x.clamp(0.0, 1.0) * 255.0).round() as u8);
                bytes.push((encoded.y.clamp(0.0, 1.0) * 255.0).round() as u8);
                bytes.push((encoded.z.clamp(0.0, 1.0) * 255.0).round() as u8);
                bytes.push(255);
            }
        }
    }
    bytes
}

fn encode_r16_layers(
    layers: &[GeneratedFaceLayer],
    getter: impl Fn(&GeneratedFaceLayer, usize) -> &Vec<f32>,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    for layer in layers {
        for mip in 0..layer.height.len() {
            for value in getter(layer, mip) {
                let encoded = ((value.clamp(0.0, 1.0) * 65535.0).round() as u16).to_le_bytes();
                bytes.extend_from_slice(&encoded);
            }
        }
    }
    bytes
}

fn linear_to_srgb_u8(value: f32) -> u8 {
    let value = value.clamp(0.0, 1.0);
    let srgb = if value <= 0.0031308 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    (srgb.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn encode_normal(value: Vec3, normal_format: NormalFormat) -> Vec3 {
    let mut value = value.normalize_or_zero();
    if normal_format == NormalFormat::OpenGl {
        value.y = -value.y;
    }
    Vec3::new(
        value.x * 0.5 + 0.5,
        value.y * 0.5 + 0.5,
        value.z * 0.5 + 0.5,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::compile_texture_set;

    #[test]
    fn generated_runtime_assets_include_registry_faces() {
        let bytes = br##"{
          "spec": "cruft.procedural_texture",
          "spec_version": "1.0.0",
          "profile": "voxel_cube_pbr",
          "output": {"size": 16},
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
        let compiled = compile_texture_set("/tmp/test.texture.json", bytes).expect("compile");
        let runtime = build_runtime_texture_assets(&compiled).expect("generate");
        assert_eq!(runtime.generated.layers.len(), 6);
        assert!(runtime.registry.get("dirt_block").is_some());
        assert_eq!(runtime.pack_data.layer_count, 6);
    }
}
