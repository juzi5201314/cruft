use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, RwLock};

use std::collections::HashMap;

use crate::blocks::{BlockStateId, AIR, DIRT, GRASS, STONE};
use crate::coords::{
    brick_coords, brick_index, brick_sub_index, padded_index, ChunkKey, PADDED_CHUNK_SIZE,
};
use crate::BRICKS_PER_CHUNK_AXIS;
use crate::CHUNK_SIZE;

pub const BRICKS_PER_CHUNK: usize = (BRICKS_PER_CHUNK_AXIS as usize)
    * (BRICKS_PER_CHUNK_AXIS as usize)
    * (BRICKS_PER_CHUNK_AXIS as usize);
pub const BRICK_VOXELS: usize = 8 * 8 * 8;

pub struct Storage {
    chunks: RwLock<HashMap<ChunkKey, Arc<Chunk>>>,
}

impl Default for Storage {
    fn default() -> Self {
        Self {
            chunks: RwLock::new(HashMap::new()),
        }
    }
}

impl Storage {
    pub fn get_chunk(&self, key: ChunkKey) -> Option<Arc<Chunk>> {
        self.chunks.read().ok()?.get(&key).cloned()
    }

    pub fn get_or_create_chunk(&self, key: ChunkKey) -> Arc<Chunk> {
        if let Some(existing) = self.get_chunk(key) {
            return existing;
        }
        let mut map = self.chunks.write().expect("storage map poisoned");
        map.entry(key)
            .or_insert_with(|| Arc::new(Chunk::new(key)))
            .clone()
    }

    pub fn clear(&self) {
        let mut map = self.chunks.write().expect("storage map poisoned");
        map.clear();
    }

    pub fn get_voxel(&self, chunk: ChunkKey, lx: u8, ly: u8, lz: u8) -> BlockStateId {
        self.get_chunk(chunk)
            .map(|c| c.get_voxel(lx, ly, lz))
            .unwrap_or(AIR)
    }

    pub fn set_voxel(
        &self,
        chunk: ChunkKey,
        lx: u8,
        ly: u8,
        lz: u8,
        new_state: BlockStateId,
    ) -> bool {
        let entry = self.get_or_create_chunk(chunk);
        let changed = entry.set_voxel(lx, ly, lz, new_state);
        if !changed {
            return false;
        }

        // chunk 边界写入需要同步标记邻居 dirty（用于 meshing 裂缝一致性）。
        if lx == 0 {
            self.mark_dirty(ChunkKey::new(chunk.x - 1, chunk.y, chunk.z));
        } else if lx == (CHUNK_SIZE as u8) - 1 {
            self.mark_dirty(ChunkKey::new(chunk.x + 1, chunk.y, chunk.z));
        }
        if ly == 0 {
            self.mark_dirty(ChunkKey::new(chunk.x, chunk.y - 1, chunk.z));
        } else if ly == (CHUNK_SIZE as u8) - 1 {
            self.mark_dirty(ChunkKey::new(chunk.x, chunk.y + 1, chunk.z));
        }
        if lz == 0 {
            self.mark_dirty(ChunkKey::new(chunk.x, chunk.y, chunk.z - 1));
        } else if lz == (CHUNK_SIZE as u8) - 1 {
            self.mark_dirty(ChunkKey::new(chunk.x, chunk.y, chunk.z + 1));
        }

        true
    }

    pub fn mark_dirty(&self, key: ChunkKey) {
        if let Some(chunk) = self.get_chunk(key) {
            chunk.mark_dirty();
        }
    }

    pub fn padded_snapshot(&self, center: ChunkKey) -> (PaddedChunk, u32) {
        // docs/voxel/storage_blocks.md：中心 + 6 轴向邻居，共 7 把锁；按 (cx,cy,cz) 字典序升序获取。
        let mut keys = Vec::with_capacity(7);
        keys.push(center);
        keys.extend(center.neighbors_6());
        keys.sort();
        keys.dedup();

        let map = self.chunks.read().expect("storage map poisoned");
        let mut present = Vec::new();
        for key in keys {
            if let Some(chunk) = map.get(&key) {
                present.push((key, Arc::clone(chunk)));
            }
        }
        drop(map);

        present.sort_by_key(|(k, _)| *k);

        let mut locks = Vec::with_capacity(present.len());
        for (_, chunk) in &present {
            locks.push(chunk.blocks.read().expect("chunk blocks poisoned"));
        }

        let mut by_key: HashMap<ChunkKey, &ChunkBlocks> = HashMap::new();
        for (i, (key, _)) in present.iter().enumerate() {
            by_key.insert(*key, &locks[i]);
        }

        let mut padded = PaddedChunk::new();
        let center_generation = self.get_chunk(center).map(|c| c.generation()).unwrap_or(0);

        // 只保证 6 邻居平面；边/角（需要对角 chunk）在当前契约下不会被内部采样使用，写死为空气。
        for py in 0..(PADDED_CHUNK_SIZE as i32) {
            for pz in 0..(PADDED_CHUNK_SIZE as i32) {
                for px in 0..(PADDED_CHUNK_SIZE as i32) {
                    let lx = px - 1;
                    let ly = py - 1;
                    let lz = pz - 1;

                    let dx = if lx < 0 {
                        -1
                    } else if lx >= CHUNK_SIZE {
                        1
                    } else {
                        0
                    };
                    let dy = if ly < 0 {
                        -1
                    } else if ly >= CHUNK_SIZE {
                        1
                    } else {
                        0
                    };
                    let dz = if lz < 0 {
                        -1
                    } else if lz >= CHUNK_SIZE {
                        1
                    } else {
                        0
                    };

                    let out_axes = (dx != 0) as u8 + (dy != 0) as u8 + (dz != 0) as u8;
                    if out_axes > 1 {
                        continue;
                    }

                    let ck = ChunkKey::new(center.x + dx, center.y + dy, center.z + dz);
                    let Some(blocks) = by_key.get(&ck).copied() else {
                        continue;
                    };

                    let nlx = if dx == -1 {
                        (CHUNK_SIZE - 1) as u8
                    } else if dx == 1 {
                        0
                    } else {
                        lx as u8
                    };
                    let nly = if dy == -1 {
                        (CHUNK_SIZE - 1) as u8
                    } else if dy == 1 {
                        0
                    } else {
                        ly as u8
                    };
                    let nlz = if dz == -1 {
                        (CHUNK_SIZE - 1) as u8
                    } else if dz == 1 {
                        0
                    } else {
                        lz as u8
                    };

                    padded.blocks[padded_index(px as u8, py as u8, pz as u8)] =
                        blocks.get_voxel(nlx, nly, nlz);
                }
            }
        }

        (padded, center_generation)
    }
}

pub struct Chunk {
    #[allow(dead_code)]
    key: ChunkKey,
    blocks: RwLock<ChunkBlocks>,
    generation: AtomicU32,
    dirty: AtomicBool,
}

impl Chunk {
    pub fn new(key: ChunkKey) -> Self {
        Self {
            key,
            blocks: RwLock::new(ChunkBlocks::new()),
            generation: AtomicU32::new(1),
            dirty: AtomicBool::new(true),
        }
    }

    pub fn generation(&self) -> u32 {
        self.generation.load(Ordering::Relaxed)
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::Relaxed)
    }

    pub fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    pub fn clear_dirty(&self) {
        self.dirty.store(false, Ordering::Relaxed);
    }

    pub fn get_voxel(&self, lx: u8, ly: u8, lz: u8) -> BlockStateId {
        let guard = self.blocks.read().expect("chunk blocks poisoned");
        guard.get_voxel(lx, ly, lz)
    }

    pub fn set_voxel(&self, lx: u8, ly: u8, lz: u8, new_state: BlockStateId) -> bool {
        let mut guard = self.blocks.write().expect("chunk blocks poisoned");
        let changed = guard.set_voxel(lx, ly, lz, new_state);
        if changed {
            self.mark_dirty();
        }
        changed
    }

    pub fn fill_direct<F>(&self, mut f: F)
    where
        F: FnMut(u8, u8, u8) -> BlockStateId,
    {
        let mut blocks = self.blocks.write().expect("chunk blocks poisoned");
        blocks.fill_direct(|lx, ly, lz| f(lx, ly, lz));
        self.mark_dirty();
    }

    pub fn fill_terrain_heightmap(&self, base_y: i32, heights: &[i32; 32 * 32]) {
        let mut blocks = self.blocks.write().expect("chunk blocks poisoned");
        blocks.fill_terrain_heightmap(base_y, heights);
        self.mark_dirty();
    }
}

#[derive(Debug, Clone)]
pub struct PaddedChunk {
    pub(crate) blocks: Vec<BlockStateId>,
}

impl PaddedChunk {
    pub fn new() -> Self {
        Self {
            blocks: vec![AIR; PADDED_CHUNK_SIZE * PADDED_CHUNK_SIZE * PADDED_CHUNK_SIZE],
        }
    }

    pub fn get(&self, px: u8, py: u8, pz: u8) -> BlockStateId {
        self.blocks[padded_index(px, py, pz)]
    }
}

impl Default for PaddedChunk {
    fn default() -> Self {
        Self::new()
    }
}

struct ChunkBlocks {
    bricks: Vec<Brick>,
    brick_solid_counts: [u16; BRICKS_PER_CHUNK],
}

impl ChunkBlocks {
    fn new() -> Self {
        let mut bricks = Vec::with_capacity(BRICKS_PER_CHUNK);
        for _ in 0..BRICKS_PER_CHUNK {
            bricks.push(Brick::Single(AIR));
        }
        Self {
            bricks,
            brick_solid_counts: [0; BRICKS_PER_CHUNK],
        }
    }

    fn get_voxel(&self, lx: u8, ly: u8, lz: u8) -> BlockStateId {
        let (bx, by, bz, sx, sy, sz) = brick_coords(lx, ly, lz);
        let bidx = brick_index(bx, by, bz);
        let sidx = brick_sub_index(sx, sy, sz);
        self.bricks[bidx].get(sidx)
    }

    fn set_voxel(&mut self, lx: u8, ly: u8, lz: u8, new_state: BlockStateId) -> bool {
        let (bx, by, bz, sx, sy, sz) = brick_coords(lx, ly, lz);
        let bidx = brick_index(bx, by, bz);
        let sidx = brick_sub_index(sx, sy, sz);

        let old_state = self.bricks[bidx].get(sidx);
        if old_state == new_state {
            return false;
        }

        let old_solid = old_state != AIR;
        let new_solid = new_state != AIR;
        if old_solid && !new_solid {
            self.brick_solid_counts[bidx] = self.brick_solid_counts[bidx].saturating_sub(1);
        } else if !old_solid && new_solid {
            self.brick_solid_counts[bidx] = self.brick_solid_counts[bidx].saturating_add(1);
        }

        self.bricks[bidx].set(sidx, new_state);
        true
    }

    fn fill_direct<F>(&mut self, mut f: F)
    where
        F: FnMut(u8, u8, u8) -> BlockStateId,
    {
        // 简化：生成阶段直接使用 Direct（或 Single），避免在生成阶段构建 palette/reverse_map。
        for by in 0..(BRICKS_PER_CHUNK_AXIS as u8) {
            for bz in 0..(BRICKS_PER_CHUNK_AXIS as u8) {
                for bx in 0..(BRICKS_PER_CHUNK_AXIS as u8) {
                    let bidx = brick_index(bx, by, bz);
                    let mut values = Box::new([AIR; BRICK_VOXELS]);
                    let mut solid = 0u16;
                    for sy in 0..8u8 {
                        for sz in 0..8u8 {
                            for sx in 0..8u8 {
                                let lx = (bx << 3) | sx;
                                let ly = (by << 3) | sy;
                                let lz = (bz << 3) | sz;
                                let v = f(lx, ly, lz);
                                if v != AIR {
                                    solid += 1;
                                }
                                values[brick_sub_index(sx, sy, sz)] = v;
                            }
                        }
                    }
                    if solid == 0 {
                        self.bricks[bidx] = Brick::Single(AIR);
                        self.brick_solid_counts[bidx] = 0;
                    } else if solid == (BRICK_VOXELS as u16) {
                        let any = values[0];
                        self.bricks[bidx] = Brick::Single(any);
                        self.brick_solid_counts[bidx] = BRICK_VOXELS as u16;
                    } else {
                        self.bricks[bidx] = Brick::Direct(values);
                        self.brick_solid_counts[bidx] = solid;
                    }
                }
            }
        }
    }

    fn fill_terrain_heightmap(&mut self, base_y: i32, heights: &[i32; 32 * 32]) {
        fn height_at(heights: &[i32; 32 * 32], lx: u8, lz: u8) -> i32 {
            heights[(lx as usize) + (lz as usize) * 32]
        }

        for bz in 0..(BRICKS_PER_CHUNK_AXIS as u8) {
            for bx in 0..(BRICKS_PER_CHUNK_AXIS as u8) {
                // brick 覆盖的 8×8 列的高度范围。
                let mut min_h = i32::MAX;
                let mut max_h = i32::MIN;
                for sz in 0..8u8 {
                    for sx in 0..8u8 {
                        let lx = (bx << 3) | sx;
                        let lz = (bz << 3) | sz;
                        let h = height_at(heights, lx, lz);
                        min_h = min_h.min(h);
                        max_h = max_h.max(h);
                    }
                }

                for by in 0..(BRICKS_PER_CHUNK_AXIS as u8) {
                    let bidx = brick_index(bx, by, bz);

                    let brick_min_y = base_y + (by as i32) * 8;
                    let brick_max_y = brick_min_y + 7;

                    if brick_min_y > max_h {
                        self.bricks[bidx] = Brick::Single(AIR);
                        self.brick_solid_counts[bidx] = 0;
                        continue;
                    }

                    // 若该 brick 的最高 y 也 <= 所有列的 (height-4)，则整 brick 都是 STONE。
                    // 规则：wy==height 为 GRASS，wy>=height-3 为 DIRT，否则 STONE。
                    if brick_max_y <= min_h - 4 {
                        self.bricks[bidx] = Brick::Single(STONE);
                        self.brick_solid_counts[bidx] = BRICK_VOXELS as u16;
                        continue;
                    }

                    let mut values = Box::new([AIR; BRICK_VOXELS]);
                    let mut solid = 0u16;

                    for sy in 0..8u8 {
                        let ly = (by << 3) | sy;
                        let wy = base_y + (ly as i32);
                        for sz in 0..8u8 {
                            let lz = (bz << 3) | sz;
                            for sx in 0..8u8 {
                                let lx = (bx << 3) | sx;
                                let height = height_at(heights, lx, lz);

                                let v = if wy > height {
                                    AIR
                                } else if wy == height {
                                    GRASS
                                } else if wy >= height - 3 {
                                    DIRT
                                } else {
                                    STONE
                                };

                                if v != AIR {
                                    solid += 1;
                                }
                                values[brick_sub_index(sx, sy, sz)] = v;
                            }
                        }
                    }

                    if solid == 0 {
                        self.bricks[bidx] = Brick::Single(AIR);
                        self.brick_solid_counts[bidx] = 0;
                    } else if solid == (BRICK_VOXELS as u16) {
                        let any = values[0];
                        self.bricks[bidx] = Brick::Single(any);
                        self.brick_solid_counts[bidx] = BRICK_VOXELS as u16;
                    } else {
                        self.bricks[bidx] = Brick::Direct(values);
                        self.brick_solid_counts[bidx] = solid;
                    }
                }
            }
        }
    }

    #[allow(dead_code)]
    fn may_contain_solid_in_brick(&self, bx: u8, by: u8, bz: u8) -> bool {
        let idx = brick_index(bx, by, bz);
        self.brick_solid_counts[idx] != 0
    }
}

enum Brick {
    Single(BlockStateId),
    Paletted(PalettedBrick),
    Direct(Box<[BlockStateId; BRICK_VOXELS]>),
}

impl Brick {
    fn get(&self, sub_index: usize) -> BlockStateId {
        match self {
            Brick::Single(v) => *v,
            Brick::Paletted(p) => p.palette[p.indices[sub_index] as usize],
            Brick::Direct(values) => values[sub_index],
        }
    }

    fn set(&mut self, sub_index: usize, new_state: BlockStateId) {
        match self {
            Brick::Single(old) => {
                if *old == new_state {
                    return;
                }
                let mut pal = PalettedBrick::from_single(*old);
                pal.set(sub_index, new_state);
                *self = Brick::Paletted(pal);
            }
            Brick::Paletted(p) => {
                if p.palette_len() >= 384 && !p.contains(new_state) {
                    let mut direct = p.to_direct();
                    direct[sub_index] = new_state;
                    *self = Brick::Direct(direct);
                    return;
                }
                p.set(sub_index, new_state);
                if p.palette_len() > 384 {
                    *self = Brick::Direct(p.to_direct());
                }
            }
            Brick::Direct(values) => {
                values[sub_index] = new_state;
            }
        }
    }
}

struct PalettedBrick {
    palette: Vec<BlockStateId>,
    indices: Box<[u16; BRICK_VOXELS]>,
    reverse_map: HashMap<BlockStateId, u16>,
}

impl PalettedBrick {
    fn from_single(value: BlockStateId) -> Self {
        let mut palette = Vec::with_capacity(2);
        palette.push(value);
        let mut reverse_map = HashMap::new();
        reverse_map.insert(value, 0);
        Self {
            palette,
            indices: Box::new([0u16; BRICK_VOXELS]),
            reverse_map,
        }
    }

    fn palette_len(&self) -> usize {
        self.palette.len()
    }

    fn contains(&self, value: BlockStateId) -> bool {
        self.reverse_map.contains_key(&value)
    }

    fn set(&mut self, sub_index: usize, new_state: BlockStateId) {
        let idx = if let Some(&i) = self.reverse_map.get(&new_state) {
            i
        } else {
            let i = self.palette.len() as u16;
            self.palette.push(new_state);
            self.reverse_map.insert(new_state, i);
            i
        };
        self.indices[sub_index] = idx;
    }

    fn to_direct(&self) -> Box<[BlockStateId; BRICK_VOXELS]> {
        let mut values = Box::new([AIR; BRICK_VOXELS]);
        for i in 0..BRICK_VOXELS {
            values[i] = self.palette[self.indices[i] as usize];
        }
        values
    }
}
