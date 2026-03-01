#![forbid(unsafe_code)]

pub mod blocks;
pub mod coords;
pub mod meshing;
pub mod render;
pub mod storage;
pub mod world;
pub mod worldgen;

pub const CHUNK_SIZE: i32 = 32;
pub const BRICK_SIZE: i32 = 8;
pub const BRICKS_PER_CHUNK_AXIS: i32 = 4;

pub use blocks::{
    BlockDef, BlockDefs, BlockStateId, RenderLayer, AIR, DIRT, GRASS, GRAVEL, SAND, SNOW, STONE,
};
pub use coords::ChunkKey;
pub use meshing::{mesh, Face, MeshingInput, MeshingOutput, PackedQuad};
pub use render::VoxelRenderPlugin;
pub use storage::{PaddedChunk, Storage};
pub use world::{
    ChunkBounds, ChunkDrawRange, ChunkGeneration, VoxelCenter, VoxelConfig, VoxelPhase,
    VoxelPlugin, VoxelWorld,
};
pub use worldgen::{
    build_generator, ColumnSurface, GeneratedChunk, ModernSurfaceGenerator, WorldGenerator,
};
