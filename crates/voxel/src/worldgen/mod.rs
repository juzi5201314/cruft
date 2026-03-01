mod factory;
mod modern_surface;

use crate::blocks::BlockStateId;
use crate::coords::ChunkKey;
use crate::CHUNK_SIZE;

pub use factory::build_generator;
pub use modern_surface::ModernSurfaceGenerator;

use cruft_worldgen_spec::WorldGenPreset;

pub const CHUNK_AREA: usize = (CHUNK_SIZE as usize) * (CHUNK_SIZE as usize);
pub const CHUNK_VOXELS: usize = CHUNK_AREA * (CHUNK_SIZE as usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColumnSurface {
    pub height: i32,
    pub top: BlockStateId,
    pub filler: BlockStateId,
    /// 含地表层在内的“表层厚度”。其下统一回落为 STONE。
    pub stone_depth: u8,
}

impl Default for ColumnSurface {
    fn default() -> Self {
        Self {
            height: 0,
            top: 0,
            filler: 0,
            stone_depth: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeneratedChunk {
    SurfaceColumns(Box<[ColumnSurface; CHUNK_AREA]>),
    Dense(Box<[BlockStateId; CHUNK_VOXELS]>),
}

pub trait WorldGenerator: Send + Sync {
    fn preset(&self) -> WorldGenPreset;
    fn algo_version(&self) -> u32;

    /// 供出生点、调试 HUD 等快速查询地表高度。
    fn sample_surface_height(&self, wx: i32, wz: i32) -> i32;

    /// 按 chunk 生成方块输入。主路径返回 `SurfaceColumns`，未来可切换到 `Dense`。
    fn generate_chunk(&self, key: ChunkKey) -> GeneratedChunk;
}

#[inline]
pub fn column_index(lx: u8, lz: u8) -> usize {
    (lx as usize) + (lz as usize) * (CHUNK_SIZE as usize)
}
