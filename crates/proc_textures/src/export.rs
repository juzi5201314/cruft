use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::compiler::{CompiledTextureSet, CubeFace, TextureFingerprint};
use crate::error::TextureDataError;
use crate::generator::{build_runtime_texture_assets, MipLevel};

#[derive(Debug, Clone, Serialize)]
pub struct ExportManifest {
    pub fingerprint: TextureFingerprint,
    pub output_size: u32,
    pub mip_level_count: u32,
    pub textures: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ExportedTextureSet {
    pub root: PathBuf,
    pub manifest: ExportManifest,
}

pub fn export_compiled_texture_set_to_dir(
    compiled: &CompiledTextureSet,
    output_dir: impl AsRef<Path>,
) -> Result<ExportedTextureSet, TextureDataError> {
    let output_dir = output_dir.as_ref().to_path_buf();
    fs::create_dir_all(&output_dir).map_err(|error| {
        TextureDataError::export(&compiled.source_path, None, error.to_string())
    })?;
    let runtime = build_runtime_texture_assets(compiled)?;

    for (texture_name, texture) in &compiled.textures {
        let texture_dir = output_dir.join(texture_name);
        fs::create_dir_all(&texture_dir).map_err(|error| {
            TextureDataError::export(
                &compiled.source_path,
                Some(texture_name.clone()),
                error.to_string(),
            )
        })?;
        for face in CubeFace::ALL {
            let face_dir = texture_dir.join(face.as_str());
            fs::create_dir_all(&face_dir).map_err(|error| {
                TextureDataError::export(
                    &compiled.source_path,
                    Some(format!("{texture_name}.{}", face.as_str())),
                    error.to_string(),
                )
            })?;
            let channels = texture.faces.get(&face).expect("face resolved");
            let layer = runtime
                .generated
                .layers
                .iter()
                .find(|candidate| {
                    candidate.surface == channels.surface
                        && candidate.face == face
                        && candidate.transform == channels.transform
                })
                .expect("generated face layer exists");

            export_rgba8_png(
                face_dir.join("albedo.png"),
                &layer.albedo[0],
                true,
                &compiled.source_path,
            )?;
            export_rgba8_png(
                face_dir.join("normal.png"),
                &layer.normal[0],
                false,
                &compiled.source_path,
            )?;
            export_rgba8_png(
                face_dir.join("orm.png"),
                &layer.orm[0],
                false,
                &compiled.source_path,
            )?;
            export_rgba8_png(
                face_dir.join("emissive.png"),
                &layer.emissive[0],
                true,
                &compiled.source_path,
            )?;
            export_r16_png(
                face_dir.join("height.png"),
                &layer.height[0],
                &compiled.source_path,
            )?;
        }
    }

    let manifest = ExportManifest {
        fingerprint: compiled.fingerprint.clone(),
        output_size: compiled.output.size,
        mip_level_count: runtime.pack_data.mip_level_count,
        textures: compiled.textures.keys().cloned().collect(),
    };
    let manifest_path = output_dir.join("manifest.json");
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).map_err(|error| {
            TextureDataError::export(&compiled.source_path, None, error.to_string())
        })?,
    )
    .map_err(|error| TextureDataError::export(&compiled.source_path, None, error.to_string()))?;

    Ok(ExportedTextureSet {
        root: output_dir,
        manifest,
    })
}

fn export_rgba8_png<T>(
    path: PathBuf,
    level: &MipLevel<T>,
    srgb: bool,
    source_path: &Path,
) -> Result<(), TextureDataError>
where
    T: Copy + IntoRgba,
{
    let file = fs::File::create(&path)
        .map_err(|error| TextureDataError::export(source_path, None, error.to_string()))?;
    let mut writer = std::io::BufWriter::new(file);
    let mut encoder = png::Encoder::new(&mut writer, level.size, level.size);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    if srgb {
        encoder.set_source_gamma(png::ScaledFloat::from_scaled(45455));
        encoder.set_source_chromaticities(png::SourceChromaticities::new(
            (0.3127, 0.3290),
            (0.64, 0.33),
            (0.30, 0.60),
            (0.15, 0.06),
        ));
    }
    let mut png_writer = encoder
        .write_header()
        .map_err(|error| TextureDataError::export(source_path, None, error.to_string()))?;
    let mut bytes = Vec::with_capacity(level.data.len() * 4);
    for pixel in &level.data {
        let rgba = pixel.as_rgba8(srgb);
        bytes.extend_from_slice(&rgba);
    }
    png_writer
        .write_image_data(&bytes)
        .map_err(|error| TextureDataError::export(source_path, None, error.to_string()))?;
    Ok(())
}

fn export_r16_png(
    path: PathBuf,
    level: &MipLevel<f32>,
    source_path: &Path,
) -> Result<(), TextureDataError> {
    let file = fs::File::create(&path)
        .map_err(|error| TextureDataError::export(source_path, None, error.to_string()))?;
    let mut writer = std::io::BufWriter::new(file);
    let mut encoder = png::Encoder::new(&mut writer, level.size, level.size);
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Sixteen);
    let mut png_writer = encoder
        .write_header()
        .map_err(|error| TextureDataError::export(source_path, None, error.to_string()))?;
    let mut bytes = Vec::with_capacity(level.data.len() * 2);
    for value in &level.data {
        bytes.extend_from_slice(&((value.clamp(0.0, 1.0) * 65535.0).round() as u16).to_be_bytes());
    }
    png_writer
        .write_image_data(&bytes)
        .map_err(|error| TextureDataError::export(source_path, None, error.to_string()))?;
    Ok(())
}

trait IntoRgba {
    fn as_rgba8(&self, srgb: bool) -> [u8; 4];
}

impl IntoRgba for bevy::prelude::Vec4 {
    fn as_rgba8(&self, srgb: bool) -> [u8; 4] {
        if srgb {
            [
                linear_to_srgb_u8(self.x),
                linear_to_srgb_u8(self.y),
                linear_to_srgb_u8(self.z),
                (self.w.clamp(0.0, 1.0) * 255.0).round() as u8,
            ]
        } else {
            [
                (self.x.clamp(0.0, 1.0) * 255.0).round() as u8,
                (self.y.clamp(0.0, 1.0) * 255.0).round() as u8,
                (self.z.clamp(0.0, 1.0) * 255.0).round() as u8,
                (self.w.clamp(0.0, 1.0) * 255.0).round() as u8,
            ]
        }
    }
}

impl IntoRgba for bevy::prelude::Vec3 {
    fn as_rgba8(&self, _srgb: bool) -> [u8; 4] {
        [
            (self.x.clamp(0.0, 1.0) * 255.0).round() as u8,
            (self.y.clamp(0.0, 1.0) * 255.0).round() as u8,
            (self.z.clamp(0.0, 1.0) * 255.0).round() as u8,
            255,
        ]
    }
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
