#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

pub const WORLD_FORMAT_VERSION_V2: u32 = 2;
pub const WORLD_HEADER_FILE: &str = "header.cruft";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorldGenPreset {
    ModernSurface,
    Superflat,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SuperflatParams {
    pub base_height: i32,
    pub top_thickness: u8,
    pub filler_thickness: u8,
}

impl Default for SuperflatParams {
    fn default() -> Self {
        Self {
            base_height: 16,
            top_thickness: 1,
            filler_thickness: 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModernSurfaceParams {
    pub sea_level: i32,
    pub warp_scale: f32,
    pub warp_amplitude: f32,
    pub continental_scale: f32,
    pub erosion_scale: f32,
    pub ridge_scale: f32,
    pub detail_scale: f32,
    pub climate_scale: f32,
    pub continental_gain: f32,
    pub mountain_gain: f32,
    pub erosion_gain: f32,
    pub detail_gain: f32,
    pub mountain_start: f32,
    pub mountain_end: f32,
    pub beach_band: i32,
    pub snow_line: i32,
}

impl Default for ModernSurfaceParams {
    fn default() -> Self {
        Self {
            sea_level: 24,
            warp_scale: 1.0 / 1024.0,
            warp_amplitude: 96.0,
            continental_scale: 1.0 / 640.0,
            erosion_scale: 1.0 / 420.0,
            ridge_scale: 1.0 / 360.0,
            detail_scale: 1.0 / 96.0,
            climate_scale: 1.0 / 512.0,
            continental_gain: 52.0,
            mountain_gain: 72.0,
            erosion_gain: 26.0,
            detail_gain: 8.0,
            mountain_start: 0.15,
            mountain_end: 0.75,
            beach_band: 2,
            snow_line: 92,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldGenConfig {
    pub seed: u64,
    pub preset: WorldGenPreset,
    pub algo_version: u32,
    pub modern_surface: ModernSurfaceParams,
    pub superflat: SuperflatParams,
}

impl Default for WorldGenConfig {
    fn default() -> Self {
        Self {
            seed: 0,
            preset: WorldGenPreset::ModernSurface,
            algo_version: 1,
            modern_surface: ModernSurfaceParams::default(),
            superflat: SuperflatParams::default(),
        }
    }
}

impl WorldGenConfig {
    pub fn modern_surface(seed: u64) -> Self {
        Self {
            seed,
            ..Self::default()
        }
    }

    pub fn superflat(seed: u64) -> Self {
        Self {
            seed,
            preset: WorldGenPreset::Superflat,
            ..Self::default()
        }
    }

    pub fn params_hash(&self) -> u64 {
        let mut h = 0xcbf29ce484222325u64;

        fn mix_u32(h: &mut u64, v: u32) {
            for b in v.to_le_bytes() {
                *h ^= b as u64;
                *h = h.wrapping_mul(0x100000001b3);
            }
        }

        fn mix_u64(h: &mut u64, v: u64) {
            for b in v.to_le_bytes() {
                *h ^= b as u64;
                *h = h.wrapping_mul(0x100000001b3);
            }
        }

        fn mix_i32(h: &mut u64, v: i32) {
            mix_u32(h, v as u32);
        }

        fn mix_f32(h: &mut u64, v: f32) {
            mix_u32(h, v.to_bits());
        }

        mix_u64(&mut h, self.seed);
        mix_u32(&mut h, self.algo_version);
        mix_u32(
            &mut h,
            match self.preset {
                WorldGenPreset::ModernSurface => 1,
                WorldGenPreset::Superflat => 2,
            },
        );

        match self.preset {
            WorldGenPreset::ModernSurface => {
                let p = &self.modern_surface;
                mix_i32(&mut h, p.sea_level);
                mix_f32(&mut h, p.warp_scale);
                mix_f32(&mut h, p.warp_amplitude);
                mix_f32(&mut h, p.continental_scale);
                mix_f32(&mut h, p.erosion_scale);
                mix_f32(&mut h, p.ridge_scale);
                mix_f32(&mut h, p.detail_scale);
                mix_f32(&mut h, p.climate_scale);
                mix_f32(&mut h, p.continental_gain);
                mix_f32(&mut h, p.mountain_gain);
                mix_f32(&mut h, p.erosion_gain);
                mix_f32(&mut h, p.detail_gain);
                mix_f32(&mut h, p.mountain_start);
                mix_f32(&mut h, p.mountain_end);
                mix_i32(&mut h, p.beach_band);
                mix_i32(&mut h, p.snow_line);
            }
            WorldGenPreset::Superflat => {
                let p = &self.superflat;
                mix_i32(&mut h, p.base_height);
                mix_u32(&mut h, p.top_thickness as u32);
                mix_u32(&mut h, p.filler_thickness as u32);
            }
        }

        h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn superflat_config_marks_preset() {
        let cfg = WorldGenConfig::superflat(123);
        assert_eq!(cfg.preset, WorldGenPreset::Superflat);
    }

    #[test]
    fn params_hash_varies_by_preset() {
        let modern = WorldGenConfig::modern_surface(7);
        let superflat = WorldGenConfig::superflat(7);
        assert_ne!(modern.params_hash(), superflat.params_hash());
    }

    #[test]
    fn world_header_roundtrip_and_validate() {
        let header = WorldHeaderV2::new(
            "world-1".to_string(),
            1_700_000_000,
            WorldGenConfig::superflat(42),
        );

        let bytes = serde_json::to_vec(&header).expect("serialize world header");
        let decoded: WorldHeaderV2 = serde_json::from_slice(&bytes).expect("deserialize");

        decoded.validate().expect("header should validate");
        assert_eq!(decoded.generator.preset, WorldGenPreset::Superflat);
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldHeaderV2 {
    pub world_format_version: u32,
    pub world_uuid: String,
    pub created_at: i64,
    pub generator: WorldGenConfig,
    pub params_hash: u64,
}

impl WorldHeaderV2 {
    pub fn new(world_uuid: String, created_at: i64, generator: WorldGenConfig) -> Self {
        let params_hash = generator.params_hash();
        Self {
            world_format_version: WORLD_FORMAT_VERSION_V2,
            world_uuid,
            created_at,
            generator,
            params_hash,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.world_format_version != WORLD_FORMAT_VERSION_V2 {
            return Err(format!(
                "unsupported world_format_version: {}, expected {}",
                self.world_format_version, WORLD_FORMAT_VERSION_V2
            ));
        }

        let expected = self.generator.params_hash();
        if self.params_hash != expected {
            return Err(format!(
                "world header params_hash mismatch: header={}, expected={expected}",
                self.params_hash
            ));
        }

        Ok(())
    }
}
