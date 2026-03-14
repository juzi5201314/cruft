use cruft_worldgen_spec::{WorldGenConfig, WorldGenPreset};

use crate::blocks::{DIRT, GRASS};
use crate::coords::ChunkKey;
use crate::worldgen::{column_index, ColumnSurface, GeneratedChunk, CHUNK_AREA};
use crate::CHUNK_SIZE;

use super::WorldGenerator;

pub struct SuperflatGenerator {
    config: WorldGenConfig,
}

impl SuperflatGenerator {
    pub fn new(config: WorldGenConfig) -> Self {
        Self { config }
    }

    fn column_surface(&self) -> ColumnSurface {
        let p = &self.config.superflat;
        ColumnSurface {
            height: p.base_height,
            top: GRASS,
            filler: DIRT,
            stone_depth: p.filler_thickness.saturating_add(1),
        }
    }
}

impl WorldGenerator for SuperflatGenerator {
    fn preset(&self) -> WorldGenPreset {
        self.config.preset
    }

    fn algo_version(&self) -> u32 {
        self.config.algo_version
    }

    fn sample_surface_height(&self, _wx: i32, _wz: i32) -> i32 {
        self.config.superflat.base_height
    }

    fn generate_chunk(&self, key: ChunkKey) -> GeneratedChunk {
        let mut columns = [ColumnSurface::default(); CHUNK_AREA];
        let _ = key;

        for lz in 0..CHUNK_SIZE {
            for lx in 0..CHUNK_SIZE {
                columns[column_index(lx as u8, lz as u8)] = self.column_surface();
            }
        }

        GeneratedChunk::SurfaceColumns(Box::new(columns))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn superflat_surface_height_is_constant() {
        let g = SuperflatGenerator::new(WorldGenConfig::superflat(5));
        assert_eq!(g.sample_surface_height(-4096, 2048), 16);
        assert_eq!(g.sample_surface_height(0, 0), 16);
    }

    #[test]
    fn chunk_generation_produces_surface_columns() {
        let g = SuperflatGenerator::new(WorldGenConfig::superflat(7));
        let out = g.generate_chunk(ChunkKey::ZERO);
        let GeneratedChunk::SurfaceColumns(cols) = out else {
            panic!("expected surface columns output");
        };

        assert_eq!(cols.len(), CHUNK_AREA);
        assert!(cols.iter().all(|c| c.height == 16));
        assert!(cols.iter().all(|c| c.top == GRASS));
    }

    #[test]
    fn same_seed_is_deterministic() {
        let g1 = SuperflatGenerator::new(WorldGenConfig::superflat(99));
        let g2 = SuperflatGenerator::new(WorldGenConfig::superflat(99));

        let h1 = g1.sample_surface_height(123, -456);
        let h2 = g2.sample_surface_height(123, -456);
        assert_eq!(h1, h2);
    }
}
