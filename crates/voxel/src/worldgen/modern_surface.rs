use cruft_worldgen_spec::{WorldGenConfig, WorldGenPreset};
use noise::{NoiseFn, OpenSimplex, Perlin};

use crate::blocks::{DIRT, GRASS, GRAVEL, SAND, SNOW, STONE};
use crate::coords::ChunkKey;
use crate::worldgen::{column_index, ColumnSurface, GeneratedChunk, CHUNK_AREA};
use crate::CHUNK_SIZE;

use super::WorldGenerator;

pub struct ModernSurfaceGenerator {
    config: WorldGenConfig,
    continental: OpenSimplex,
    erosion: OpenSimplex,
    ridge: OpenSimplex,
    detail: Perlin,
    temperature: Perlin,
    humidity: Perlin,
    warp_x: Perlin,
    warp_z: Perlin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SurfaceBiome {
    Plains,
    Beach,
    Rocky,
    SnowPeak,
}

impl ModernSurfaceGenerator {
    pub fn new(config: WorldGenConfig) -> Self {
        let base = config.seed;
        Self {
            config,
            continental: OpenSimplex::new(seed32(base, 0xA1)),
            erosion: OpenSimplex::new(seed32(base, 0xB2)),
            ridge: OpenSimplex::new(seed32(base, 0xC3)),
            detail: Perlin::new(seed32(base, 0xD4)),
            temperature: Perlin::new(seed32(base, 0xE5)),
            humidity: Perlin::new(seed32(base, 0xF6)),
            warp_x: Perlin::new(seed32(base, 0x31)),
            warp_z: Perlin::new(seed32(base, 0x52)),
        }
    }

    fn sample_surface_with_biome(&self, wx: i32, wz: i32) -> (i32, SurfaceBiome) {
        let p = &self.config.modern_surface;

        let warp_input_x = (wx as f64) * (p.warp_scale as f64);
        let warp_input_z = (wz as f64) * (p.warp_scale as f64);
        let warp_dx = self.warp_x.get([warp_input_x, warp_input_z]) as f32 * p.warp_amplitude;
        let warp_dz = self.warp_z.get([warp_input_x, warp_input_z]) as f32 * p.warp_amplitude;

        let sx = wx as f32 + warp_dx;
        let sz = wz as f32 + warp_dz;

        let continental = fbm2(
            &self.continental,
            sx as f64 * p.continental_scale as f64,
            sz as f64 * p.continental_scale as f64,
            5,
            2.0,
            0.5,
        );
        let erosion = fbm2(
            &self.erosion,
            sx as f64 * p.erosion_scale as f64,
            sz as f64 * p.erosion_scale as f64,
            4,
            2.0,
            0.52,
        );
        let ridge_raw = fbm2(
            &self.ridge,
            sx as f64 * p.ridge_scale as f64,
            sz as f64 * p.ridge_scale as f64,
            4,
            2.0,
            0.5,
        );
        let ridge = 1.0 - ridge_raw.abs();
        let detail = fbm2(
            &self.detail,
            sx as f64 * p.detail_scale as f64,
            sz as f64 * p.detail_scale as f64,
            3,
            2.0,
            0.5,
        );
        let temperature = fbm2(
            &self.temperature,
            sx as f64 * p.climate_scale as f64,
            sz as f64 * p.climate_scale as f64,
            3,
            2.0,
            0.5,
        );
        let humidity = fbm2(
            &self.humidity,
            sx as f64 * p.climate_scale as f64,
            sz as f64 * p.climate_scale as f64,
            3,
            2.0,
            0.5,
        );

        let mountain_mask = smoothstep(p.mountain_start, p.mountain_end, ridge);

        let mut height = p.sea_level as f32;
        height += continental * p.continental_gain;
        height += mountain_mask * p.mountain_gain;
        height -= erosion * p.erosion_gain;
        height += detail * p.detail_gain;

        let height = height.round() as i32;

        let biome = if height <= p.sea_level + p.beach_band {
            SurfaceBiome::Beach
        } else if height >= p.snow_line || (temperature < -0.25 && mountain_mask > 0.45) {
            SurfaceBiome::SnowPeak
        } else if mountain_mask > 0.58 || erosion < -0.28 || humidity < -0.35 {
            SurfaceBiome::Rocky
        } else {
            SurfaceBiome::Plains
        };

        (height, biome)
    }

    fn column_surface(&self, wx: i32, wz: i32) -> ColumnSurface {
        let (height, biome) = self.sample_surface_with_biome(wx, wz);
        match biome {
            SurfaceBiome::Plains => ColumnSurface {
                height,
                top: GRASS,
                filler: DIRT,
                stone_depth: 4,
            },
            SurfaceBiome::Beach => ColumnSurface {
                height,
                top: SAND,
                filler: SAND,
                stone_depth: 4,
            },
            SurfaceBiome::Rocky => ColumnSurface {
                height,
                top: GRAVEL,
                filler: STONE,
                stone_depth: 2,
            },
            SurfaceBiome::SnowPeak => ColumnSurface {
                height,
                top: SNOW,
                filler: STONE,
                stone_depth: 2,
            },
        }
    }
}

impl WorldGenerator for ModernSurfaceGenerator {
    fn preset(&self) -> WorldGenPreset {
        self.config.preset
    }

    fn algo_version(&self) -> u32 {
        self.config.algo_version
    }

    fn sample_surface_height(&self, wx: i32, wz: i32) -> i32 {
        self.sample_surface_with_biome(wx, wz).0
    }

    fn generate_chunk(&self, key: ChunkKey) -> GeneratedChunk {
        let mut columns = [ColumnSurface::default(); CHUNK_AREA];
        let base = key.min_world_voxel();

        for lz in 0..CHUNK_SIZE {
            for lx in 0..CHUNK_SIZE {
                let col = self.column_surface(base.x + lx, base.z + lz);
                columns[column_index(lx as u8, lz as u8)] = col;
            }
        }

        GeneratedChunk::SurfaceColumns(Box::new(columns))
    }
}

fn fbm2<N>(noise: &N, x: f64, z: f64, octaves: u8, lacunarity: f64, gain: f64) -> f32
where
    N: NoiseFn<f64, 2>,
{
    let mut sum = 0.0;
    let mut amp = 1.0;
    let mut freq = 1.0;
    let mut norm = 0.0;

    for _ in 0..octaves {
        sum += noise.get([x * freq, z * freq]) * amp;
        norm += amp;
        amp *= gain;
        freq *= lacunarity;
    }

    if norm == 0.0 {
        return 0.0;
    }

    (sum / norm) as f32
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if edge0 >= edge1 {
        return if x >= edge0 { 1.0 } else { 0.0 };
    }

    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn seed32(seed: u64, salt: u64) -> u32 {
    let mut v = seed ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    v ^= v >> 33;
    v = v.wrapping_mul(0xff51_afd7_ed55_8ccd);
    v ^= v >> 33;
    v = v.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    v ^= v >> 33;
    v as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_is_deterministic() {
        let g = ModernSurfaceGenerator::new(WorldGenConfig::modern_surface(42));
        assert_eq!(
            g.sample_surface_height(128, -77),
            g.sample_surface_height(128, -77)
        );
    }

    #[test]
    fn different_seeds_vary() {
        let g1 = ModernSurfaceGenerator::new(WorldGenConfig::modern_surface(1));
        let g2 = ModernSurfaceGenerator::new(WorldGenConfig::modern_surface(2));

        let mut diff = 0;
        for i in 0..32 {
            if g1.sample_surface_height(i * 7, i * -13) != g2.sample_surface_height(i * 7, i * -13)
            {
                diff += 1;
            }
        }

        assert!(
            diff >= 8,
            "expected enough variation across seeds, got {diff}"
        );
    }

    #[test]
    fn chunk_generation_produces_surface_columns() {
        let g = ModernSurfaceGenerator::new(WorldGenConfig::modern_surface(7));
        let out = g.generate_chunk(ChunkKey::ZERO);

        let GeneratedChunk::SurfaceColumns(cols) = out else {
            panic!("expected surface columns output");
        };

        assert_eq!(cols.len(), CHUNK_AREA);
        assert!(cols
            .iter()
            .any(|c| c.top == GRASS || c.top == SAND || c.top == SNOW));
    }
}
