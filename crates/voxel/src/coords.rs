use bevy::prelude::*;
use bevy::render::extract_component::ExtractComponent;

use crate::{BRICKS_PER_CHUNK_AXIS, BRICK_SIZE, CHUNK_SIZE};

pub fn floor_div(a: i32, b: i32) -> i32 {
    a.div_euclid(b)
}

pub fn floor_mod(a: i32, b: i32) -> i32 {
    a.rem_euclid(b)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Component, ExtractComponent)]
pub struct ChunkKey {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl ChunkKey {
    pub const ZERO: Self = Self { x: 0, y: 0, z: 0 };

    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    pub fn from_world_voxel(world: IVec3) -> (Self, LocalVoxel) {
        let cx = floor_div(world.x, CHUNK_SIZE);
        let cy = floor_div(world.y, CHUNK_SIZE);
        let cz = floor_div(world.z, CHUNK_SIZE);

        let lx = floor_mod(world.x, CHUNK_SIZE) as u8;
        let ly = floor_mod(world.y, CHUNK_SIZE) as u8;
        let lz = floor_mod(world.z, CHUNK_SIZE) as u8;

        (Self::new(cx, cy, cz), LocalVoxel::new(lx, ly, lz))
    }

    pub fn min_world_voxel(self) -> IVec3 {
        IVec3::new(
            self.x * CHUNK_SIZE,
            self.y * CHUNK_SIZE,
            self.z * CHUNK_SIZE,
        )
    }

    pub fn neighbors_6(self) -> [Self; 6] {
        [
            Self::new(self.x + 1, self.y, self.z),
            Self::new(self.x - 1, self.y, self.z),
            Self::new(self.x, self.y + 1, self.z),
            Self::new(self.x, self.y - 1, self.z),
            Self::new(self.x, self.y, self.z + 1),
            Self::new(self.x, self.y, self.z - 1),
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalVoxel {
    pub x: u8,
    pub y: u8,
    pub z: u8,
}

impl LocalVoxel {
    pub fn new(x: u8, y: u8, z: u8) -> Self {
        debug_assert!(x < CHUNK_SIZE as u8);
        debug_assert!(y < CHUNK_SIZE as u8);
        debug_assert!(z < CHUNK_SIZE as u8);
        Self { x, y, z }
    }
}

pub fn chunk_index(lx: u8, ly: u8, lz: u8) -> usize {
    // docs/voxel/storage.md 约定：x 最快、其次 z、最后 y
    lx as usize
        + (lz as usize) * (CHUNK_SIZE as usize)
        + (ly as usize) * (CHUNK_SIZE as usize) * (CHUNK_SIZE as usize)
}

pub fn brick_coords(lx: u8, ly: u8, lz: u8) -> (u8, u8, u8, u8, u8, u8) {
    let bx = lx >> 3;
    let by = ly >> 3;
    let bz = lz >> 3;
    let sx = lx & 7;
    let sy = ly & 7;
    let sz = lz & 7;
    (bx, by, bz, sx, sy, sz)
}

pub fn brick_index(bx: u8, by: u8, bz: u8) -> usize {
    // docs/voxel/storage_blocks.md 约定：x 最快、其次 z、最后 y
    debug_assert!(bx < BRICKS_PER_CHUNK_AXIS as u8);
    debug_assert!(by < BRICKS_PER_CHUNK_AXIS as u8);
    debug_assert!(bz < BRICKS_PER_CHUNK_AXIS as u8);
    bx as usize
        + (bz as usize) * (BRICKS_PER_CHUNK_AXIS as usize)
        + (by as usize) * (BRICKS_PER_CHUNK_AXIS as usize) * (BRICKS_PER_CHUNK_AXIS as usize)
}

pub fn brick_sub_index(sx: u8, sy: u8, sz: u8) -> usize {
    // docs/voxel/storage.md 约定：x 最快、其次 z、最后 y
    debug_assert!(sx < BRICK_SIZE as u8);
    debug_assert!(sy < BRICK_SIZE as u8);
    debug_assert!(sz < BRICK_SIZE as u8);
    sx as usize
        + (sz as usize) * (BRICK_SIZE as usize)
        + (sy as usize) * (BRICK_SIZE as usize) * (BRICK_SIZE as usize)
}

pub const PADDED_CHUNK_SIZE: usize = (CHUNK_SIZE as usize) + 2;

pub fn padded_index(px: u8, py: u8, pz: u8) -> usize {
    debug_assert!((px as usize) < PADDED_CHUNK_SIZE);
    debug_assert!((py as usize) < PADDED_CHUNK_SIZE);
    debug_assert!((pz as usize) < PADDED_CHUNK_SIZE);
    px as usize
        + (pz as usize) * PADDED_CHUNK_SIZE
        + (py as usize) * PADDED_CHUNK_SIZE * PADDED_CHUNK_SIZE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floor_div_handles_negative() {
        assert_eq!(floor_div(0, 32), 0);
        assert_eq!(floor_div(31, 32), 0);
        assert_eq!(floor_div(32, 32), 1);

        assert_eq!(floor_div(-1, 32), -1);
        assert_eq!(floor_div(-32, 32), -1);
        assert_eq!(floor_div(-33, 32), -2);
    }

    #[test]
    fn chunk_index_is_x_z_y_order() {
        assert_eq!(chunk_index(0, 0, 0), 0);
        assert_eq!(chunk_index(1, 0, 0), 1);
        assert_eq!(chunk_index(0, 0, 1), 32);
        assert_eq!(chunk_index(0, 1, 0), 32 * 32);
        assert_eq!(chunk_index(31, 31, 31), 32 * 32 * 32 - 1);
    }

    #[test]
    fn brick_index_is_x_z_y_order() {
        assert_eq!(brick_index(0, 0, 0), 0);
        assert_eq!(brick_index(1, 0, 0), 1);
        assert_eq!(brick_index(0, 0, 1), 4);
        assert_eq!(brick_index(0, 1, 0), 16);
        assert_eq!(brick_index(3, 3, 3), 63);
    }

    #[test]
    fn padded_index_is_x_z_y_order() {
        assert_eq!(padded_index(0, 0, 0), 0);
        assert_eq!(padded_index(1, 0, 0), 1);
        assert_eq!(padded_index(0, 0, 1), PADDED_CHUNK_SIZE);
        assert_eq!(padded_index(0, 1, 0), PADDED_CHUNK_SIZE * PADDED_CHUNK_SIZE);
    }
}
