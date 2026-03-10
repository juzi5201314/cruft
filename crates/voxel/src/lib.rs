#![forbid(unsafe_code)]

pub mod blocks;
pub mod coords;
pub mod meshing;
pub mod render;
pub mod storage;
pub mod terrain;
pub mod world;

pub const CHUNK_SIZE: i32 = 32;
pub const BRICK_SIZE: i32 = 8;
pub const BRICKS_PER_CHUNK_AXIS: i32 = 4;

pub use blocks::{
    BlockDef, BlockDefs, BlockMaterialBinding, BlockStateId, MaterialId, RenderLayer, AIR, DIRT,
    GRASS, STONE,
};
pub use coords::ChunkKey;
pub use meshing::{mesh, Face, MeshingInput, MeshingOutput, PackedQuad};
pub use render::VoxelRenderPlugin;
pub use storage::{PaddedChunk, Storage};
pub use terrain::Terrain;
pub use world::{
    ChunkBounds, ChunkDrawRange, ChunkGeneration, VoxelCenter, VoxelConfig, VoxelPhase,
    VoxelPlugin, VoxelWorld,
};
