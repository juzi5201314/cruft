use crate::blocks::{BlockDefs, RenderLayer, AIR};
use crate::coords::ChunkKey;
use crate::coords::PADDED_CHUNK_SIZE;
use crate::storage::PaddedChunk;
use crate::CHUNK_SIZE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Face {
    PosX = 0,
    NegX = 1,
    PosY = 2,
    NegY = 3,
    PosZ = 4,
    NegZ = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackedQuad(pub u64);

impl PackedQuad {
    /// `docs/voxel/meshing.md` 的 PackedQuad(u64) 写死布局：
    /// - low32: x(6) | y(6) | z(6) | w_minus1(5) | h_minus1(5) | face(3) | reserved1(1)
    /// - high32: material_key(8) | flags(8) | reserved(16)
    pub fn new(x: u8, y: u8, z: u8, w: u8, h: u8, face: Face, material_key: u8, flags: u8) -> Self {
        debug_assert!(x <= 32);
        debug_assert!(y <= 32);
        debug_assert!(z <= 32);
        debug_assert!((1..=32).contains(&w));
        debug_assert!((1..=32).contains(&h));

        let low32 = (x as u32)
            | ((y as u32) << 6)
            | ((z as u32) << 12)
            | (((w as u32) - 1) << 18)
            | (((h as u32) - 1) << 23)
            | ((face as u32) << 28);
        let high32 = (material_key as u32) | ((flags as u32) << 8);
        Self((low32 as u64) | ((high32 as u64) << 32))
    }

    pub fn decode(self) -> DecodedQuad {
        let low = self.0 as u32;
        let high = (self.0 >> 32) as u32;
        let x = (low & 0x3F) as u8;
        let y = ((low >> 6) & 0x3F) as u8;
        let z = ((low >> 12) & 0x3F) as u8;
        let w = (((low >> 18) & 0x1F) as u8) + 1;
        let h = (((low >> 23) & 0x1F) as u8) + 1;
        let face = ((low >> 28) & 0x07) as u8;
        let material_key = (high & 0xFF) as u8;
        let flags = ((high >> 8) & 0xFF) as u8;
        DecodedQuad {
            x,
            y,
            z,
            w,
            h,
            face,
            material_key,
            flags,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodedQuad {
    pub x: u8,
    pub y: u8,
    pub z: u8,
    pub w: u8,
    pub h: u8,
    pub face: u8,
    pub material_key: u8,
    pub flags: u8,
}

#[derive(Debug, Clone)]
pub struct MeshingInput {
    pub key: ChunkKey,
    pub generation: u32,
    pub padded: PaddedChunk,
}

#[derive(Debug, Clone)]
pub struct MeshingOutput {
    pub key: ChunkKey,
    pub generation: u32,
    pub opaque: Vec<PackedQuad>,
    pub cutout: Vec<PackedQuad>,
    pub transparent: Vec<PackedQuad>,
}

impl MeshingOutput {
    pub fn is_stale_against(&self, current_generation: u32) -> bool {
        self.generation != current_generation
    }
}

pub fn mesh(input: &MeshingInput, defs: &BlockDefs) -> MeshingOutput {
    let (opaque, cutout, transparent) = mesh_padded_chunk(&input.padded, defs);
    MeshingOutput {
        key: input.key,
        generation: input.generation,
        opaque,
        cutout,
        transparent,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FaceKey {
    render_layer: RenderLayer,
    material_key: u8,
}

const CHUNK_AXIS: usize = CHUNK_SIZE as usize;
const CHUNK_AREA: usize = CHUNK_AXIS * CHUNK_AXIS;

pub fn mesh_padded_chunk(
    padded: &PaddedChunk,
    defs: &BlockDefs,
) -> (Vec<PackedQuad>, Vec<PackedQuad>, Vec<PackedQuad>) {
    let mut opaque = Vec::new();
    let mut cutout = Vec::new();
    let mut transparent = Vec::new();

    let mut grid = vec![None; CHUNK_AREA];

    // +X / -X：w 沿 +Z，h 沿 +Y（u=z, v=y）
    for lx in 0..(CHUNK_SIZE as u8) {
        fill_x_plane_visibility(padded, defs, lx, true, &mut grid);
        greedy_merge_grid(&mut grid, |u, v, w, h, key| {
            push_quad_by_layer(
                &mut opaque,
                &mut cutout,
                &mut transparent,
                key,
                PackedQuad::new(
                    lx + 1,
                    v,
                    u,
                    w,
                    h,
                    Face::PosX,
                    key.material_key,
                    key.render_layer as u8,
                ),
            );
        });

        fill_x_plane_visibility(padded, defs, lx, false, &mut grid);
        greedy_merge_grid(&mut grid, |u, v, w, h, key| {
            push_quad_by_layer(
                &mut opaque,
                &mut cutout,
                &mut transparent,
                key,
                PackedQuad::new(
                    lx,
                    v,
                    u,
                    w,
                    h,
                    Face::NegX,
                    key.material_key,
                    key.render_layer as u8,
                ),
            );
        });
    }

    // +Y / -Y：w 沿 +X，h 沿 +Z（u=x, v=z）
    for ly in 0..(CHUNK_SIZE as u8) {
        fill_y_plane_visibility(padded, defs, ly, true, &mut grid);
        greedy_merge_grid(&mut grid, |u, v, w, h, key| {
            push_quad_by_layer(
                &mut opaque,
                &mut cutout,
                &mut transparent,
                key,
                PackedQuad::new(
                    u,
                    ly + 1,
                    v,
                    w,
                    h,
                    Face::PosY,
                    key.material_key,
                    key.render_layer as u8,
                ),
            );
        });

        fill_y_plane_visibility(padded, defs, ly, false, &mut grid);
        greedy_merge_grid(&mut grid, |u, v, w, h, key| {
            push_quad_by_layer(
                &mut opaque,
                &mut cutout,
                &mut transparent,
                key,
                PackedQuad::new(
                    u,
                    ly,
                    v,
                    w,
                    h,
                    Face::NegY,
                    key.material_key,
                    key.render_layer as u8,
                ),
            );
        });
    }

    // +Z / -Z：w 沿 +X，h 沿 +Y（u=x, v=y）
    for lz in 0..(CHUNK_SIZE as u8) {
        fill_z_plane_visibility(padded, defs, lz, true, &mut grid);
        greedy_merge_grid(&mut grid, |u, v, w, h, key| {
            push_quad_by_layer(
                &mut opaque,
                &mut cutout,
                &mut transparent,
                key,
                PackedQuad::new(
                    u,
                    v,
                    lz + 1,
                    w,
                    h,
                    Face::PosZ,
                    key.material_key,
                    key.render_layer as u8,
                ),
            );
        });

        fill_z_plane_visibility(padded, defs, lz, false, &mut grid);
        greedy_merge_grid(&mut grid, |u, v, w, h, key| {
            push_quad_by_layer(
                &mut opaque,
                &mut cutout,
                &mut transparent,
                key,
                PackedQuad::new(
                    u,
                    v,
                    lz,
                    w,
                    h,
                    Face::NegZ,
                    key.material_key,
                    key.render_layer as u8,
                ),
            );
        });
    }

    (opaque, cutout, transparent)
}

#[inline]
fn grid_index(u: usize, v: usize) -> usize {
    u + v * CHUNK_AXIS
}

#[inline]
fn face_key_for(
    padded: &PaddedChunk,
    defs: &BlockDefs,
    lx: u8,
    ly: u8,
    lz: u8,
    dx: i8,
    dy: i8,
    dz: i8,
) -> Option<FaceKey> {
    let px = lx + 1;
    let py = ly + 1;
    let pz = lz + 1;

    debug_assert!((px as usize) < PADDED_CHUNK_SIZE);
    debug_assert!((py as usize) < PADDED_CHUNK_SIZE);
    debug_assert!((pz as usize) < PADDED_CHUNK_SIZE);

    let a = padded.get(px, py, pz);
    if a == AIR {
        return None;
    }
    let a_def = defs.def(a);

    let nx = (px as i16 + dx as i16) as u8;
    let ny = (py as i16 + dy as i16) as u8;
    let nz = (pz as i16 + dz as i16) as u8;
    let b = padded.get(nx, ny, nz);
    let b_def = defs.def(b);
    if b_def.is_occluder {
        return None;
    }

    Some(FaceKey {
        render_layer: a_def.render_layer,
        material_key: a_def.material_key,
    })
}

fn fill_x_plane_visibility(
    padded: &PaddedChunk,
    defs: &BlockDefs,
    lx: u8,
    positive: bool,
    grid: &mut [Option<FaceKey>],
) {
    debug_assert_eq!(grid.len(), CHUNK_AREA);
    let dx = if positive { 1i8 } else { -1i8 };

    for ly in 0..(CHUNK_SIZE as u8) {
        for lz in 0..(CHUNK_SIZE as u8) {
            let key = face_key_for(padded, defs, lx, ly, lz, dx, 0, 0);
            grid[grid_index(lz as usize, ly as usize)] = key;
        }
    }
}

fn fill_y_plane_visibility(
    padded: &PaddedChunk,
    defs: &BlockDefs,
    ly: u8,
    positive: bool,
    grid: &mut [Option<FaceKey>],
) {
    debug_assert_eq!(grid.len(), CHUNK_AREA);
    let dy = if positive { 1i8 } else { -1i8 };

    for lz in 0..(CHUNK_SIZE as u8) {
        for lx in 0..(CHUNK_SIZE as u8) {
            let key = face_key_for(padded, defs, lx, ly, lz, 0, dy, 0);
            grid[grid_index(lx as usize, lz as usize)] = key;
        }
    }
}

fn fill_z_plane_visibility(
    padded: &PaddedChunk,
    defs: &BlockDefs,
    lz: u8,
    positive: bool,
    grid: &mut [Option<FaceKey>],
) {
    debug_assert_eq!(grid.len(), CHUNK_AREA);
    let dz = if positive { 1i8 } else { -1i8 };

    for ly in 0..(CHUNK_SIZE as u8) {
        for lx in 0..(CHUNK_SIZE as u8) {
            let key = face_key_for(padded, defs, lx, ly, lz, 0, 0, dz);
            grid[grid_index(lx as usize, ly as usize)] = key;
        }
    }
}

fn greedy_merge_grid(grid: &mut [Option<FaceKey>], mut emit: impl FnMut(u8, u8, u8, u8, FaceKey)) {
    debug_assert_eq!(grid.len(), CHUNK_AREA);

    for v in 0..CHUNK_AXIS {
        let mut u = 0usize;
        while u < CHUNK_AXIS {
            let idx = grid_index(u, v);
            let Some(key) = grid[idx] else {
                u += 1;
                continue;
            };

            let mut width = 1usize;
            while u + width < CHUNK_AXIS && grid[grid_index(u + width, v)] == Some(key) {
                width += 1;
            }

            let mut height = 1usize;
            'grow_h: while v + height < CHUNK_AXIS {
                for du in 0..width {
                    if grid[grid_index(u + du, v + height)] != Some(key) {
                        break 'grow_h;
                    }
                }
                height += 1;
            }

            for dv in 0..height {
                for du in 0..width {
                    grid[grid_index(u + du, v + dv)] = None;
                }
            }

            emit(u as u8, v as u8, width as u8, height as u8, key);

            u += width;
        }
    }
}

#[inline]
fn push_quad_by_layer(
    opaque: &mut Vec<PackedQuad>,
    cutout: &mut Vec<PackedQuad>,
    transparent: &mut Vec<PackedQuad>,
    key: FaceKey,
    quad: PackedQuad,
) {
    match key.render_layer {
        RenderLayer::Opaque => opaque.push(quad),
        RenderLayer::Cutout => cutout.push(quad),
        RenderLayer::Transparent => transparent.push(quad),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::{BlockDefs, STONE};
    use crate::coords::ChunkKey;
    use crate::storage::Storage;

    #[test]
    fn single_block_has_6_faces() {
        let storage = Storage::default();
        let defs = BlockDefs::default();

        let key = ChunkKey::ZERO;
        storage.set_voxel(key, 0, 0, 0, STONE);
        let (padded, _) = storage.padded_snapshot(key);

        let (opaque, cutout, transparent) = mesh_padded_chunk(&padded, &defs);
        assert_eq!(cutout.len(), 0);
        assert_eq!(transparent.len(), 0);
        assert_eq!(opaque.len(), 6);

        let mut got = opaque.iter().map(|q| q.decode()).collect::<Vec<_>>();
        got.sort_by_key(|q| (q.face, q.x, q.y, q.z));

        let expect = [
            // face, x, y, z
            (Face::PosX as u8, 1u8, 0u8, 0u8),
            (Face::NegX as u8, 0u8, 0u8, 0u8),
            (Face::PosY as u8, 0u8, 1u8, 0u8),
            (Face::NegY as u8, 0u8, 0u8, 0u8),
            (Face::PosZ as u8, 0u8, 0u8, 1u8),
            (Face::NegZ as u8, 0u8, 0u8, 0u8),
        ];

        let mut exp = expect
            .into_iter()
            .map(|(face, x, y, z)| DecodedQuad {
                x,
                y,
                z,
                w: 1,
                h: 1,
                face,
                material_key: defs.def(STONE).material_key,
                flags: RenderLayer::Opaque as u8,
            })
            .collect::<Vec<_>>();
        exp.sort_by_key(|q| (q.face, q.x, q.y, q.z));

        for (a, b) in got.into_iter().zip(exp.into_iter()) {
            assert_eq!(a, b);
        }
    }

    #[test]
    fn packed_quad_roundtrip_with_u8_material_key() {
        let quad = PackedQuad::new(1, 2, 3, 4, 5, Face::PosZ, 13, 7);
        let decoded = quad.decode();
        assert_eq!(decoded.material_key, 13);
        assert_eq!(decoded.flags, 7);
        assert_eq!(decoded.face, Face::PosZ as u8);
    }

    #[test]
    fn solid_chunk_collapses_to_6_quads() {
        let storage = Storage::default();
        let defs = BlockDefs::default();

        let key = ChunkKey::ZERO;
        let chunk = storage.get_or_create_chunk(key);
        chunk.fill_direct(|_, _, _| STONE);
        chunk.clear_dirty();

        let (padded, _) = storage.padded_snapshot(key);
        let (opaque, cutout, transparent) = mesh_padded_chunk(&padded, &defs);

        assert!(cutout.is_empty());
        assert!(transparent.is_empty());
        assert_eq!(opaque.len(), 6);

        for decoded in opaque.into_iter().map(|q| q.decode()) {
            assert_eq!(decoded.material_key, defs.def(STONE).material_key);
            assert_eq!(decoded.flags, RenderLayer::Opaque as u8);
            assert_eq!(decoded.w, 32);
            assert_eq!(decoded.h, 32);
        }
    }
}
