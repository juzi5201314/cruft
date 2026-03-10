use crate::blocks::{BlockDefs, BlockStateId, RenderLayer, AIR};
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
    /// - high32: material_id(16) | flags(8) | reserved(8)
    pub fn new(x: u8, y: u8, z: u8, w: u8, h: u8, face: Face, material_id: u16, flags: u8) -> Self {
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
        let high32 = (material_id as u32) | ((flags as u32) << 16);
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
        let material_id = (high & 0xFFFF) as u16;
        let flags = ((high >> 16) & 0xFF) as u8;
        DecodedQuad {
            x,
            y,
            z,
            w,
            h,
            face,
            material_id,
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
    pub material_id: u16,
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

pub fn mesh_padded_chunk(
    padded: &PaddedChunk,
    defs: &BlockDefs,
) -> (Vec<PackedQuad>, Vec<PackedQuad>, Vec<PackedQuad>) {
    // 当前实现：先满足 face 编码/起点语义，暂不实现 bitmask + greedy 合并。
    let mut opaque = Vec::new();
    let mut cutout = Vec::new();
    let mut transparent = Vec::new();

    for ly in 0..(CHUNK_SIZE as u8) {
        for lz in 0..(CHUNK_SIZE as u8) {
            for lx in 0..(CHUNK_SIZE as u8) {
                let px = lx + 1;
                let py = ly + 1;
                let pz = lz + 1;
                debug_assert!((px as usize) < PADDED_CHUNK_SIZE);
                debug_assert!((py as usize) < PADDED_CHUNK_SIZE);
                debug_assert!((pz as usize) < PADDED_CHUNK_SIZE);

                let a = padded.get(px, py, pz);
                if a == AIR {
                    continue;
                }
                let a_def = defs.def(a);

                emit_faces_for_voxel(
                    a,
                    a_def,
                    (lx, ly, lz),
                    (px, py, pz),
                    padded,
                    defs,
                    &mut opaque,
                    &mut cutout,
                    &mut transparent,
                );
            }
        }
    }

    (opaque, cutout, transparent)
}

fn emit_faces_for_voxel(
    _a: BlockStateId,
    a_def: crate::blocks::BlockDef,
    local: (u8, u8, u8),
    padded_pos: (u8, u8, u8),
    padded: &PaddedChunk,
    defs: &BlockDefs,
    opaque: &mut Vec<PackedQuad>,
    cutout: &mut Vec<PackedQuad>,
    transparent: &mut Vec<PackedQuad>,
) {
    let (lx, ly, lz) = local;
    let (px, py, pz) = padded_pos;

    let layer_flags = a_def.render_layer as u8;

    // +X
    try_emit_face(
        padded,
        defs,
        (px + 1, py, pz),
        a_def,
        PackedQuad::new(
            lx + 1,
            ly,
            lz,
            1,
            1,
            Face::PosX,
            a_def.material_binding.material_key_for(Face::PosX),
            layer_flags,
        ),
        opaque,
        cutout,
        transparent,
    );
    // -X
    try_emit_face(
        padded,
        defs,
        (px - 1, py, pz),
        a_def,
        PackedQuad::new(
            lx,
            ly,
            lz,
            1,
            1,
            Face::NegX,
            a_def.material_binding.material_key_for(Face::NegX),
            layer_flags,
        ),
        opaque,
        cutout,
        transparent,
    );
    // +Y
    try_emit_face(
        padded,
        defs,
        (px, py + 1, pz),
        a_def,
        PackedQuad::new(
            lx,
            ly + 1,
            lz,
            1,
            1,
            Face::PosY,
            a_def.material_binding.material_key_for(Face::PosY),
            layer_flags,
        ),
        opaque,
        cutout,
        transparent,
    );
    // -Y
    try_emit_face(
        padded,
        defs,
        (px, py - 1, pz),
        a_def,
        PackedQuad::new(
            lx,
            ly,
            lz,
            1,
            1,
            Face::NegY,
            a_def.material_binding.material_key_for(Face::NegY),
            layer_flags,
        ),
        opaque,
        cutout,
        transparent,
    );
    // +Z
    try_emit_face(
        padded,
        defs,
        (px, py, pz + 1),
        a_def,
        PackedQuad::new(
            lx,
            ly,
            lz + 1,
            1,
            1,
            Face::PosZ,
            a_def.material_binding.material_key_for(Face::PosZ),
            layer_flags,
        ),
        opaque,
        cutout,
        transparent,
    );
    // -Z
    try_emit_face(
        padded,
        defs,
        (px, py, pz - 1),
        a_def,
        PackedQuad::new(
            lx,
            ly,
            lz,
            1,
            1,
            Face::NegZ,
            a_def.material_binding.material_key_for(Face::NegZ),
            layer_flags,
        ),
        opaque,
        cutout,
        transparent,
    );
}

fn try_emit_face(
    padded: &PaddedChunk,
    defs: &BlockDefs,
    neighbor_padded: (u8, u8, u8),
    a_def: crate::blocks::BlockDef,
    quad: PackedQuad,
    opaque: &mut Vec<PackedQuad>,
    cutout: &mut Vec<PackedQuad>,
    transparent: &mut Vec<PackedQuad>,
) {
    let (nx, ny, nz) = neighbor_padded;
    let b = padded.get(nx, ny, nz);
    let b_def = defs.def(b);

    // docs/voxel/meshing.md：若邻居是 occluder，则不生成面。
    if b_def.is_occluder {
        return;
    }

    match a_def.render_layer {
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
            // face, x, y, z (w/h/material/flags 在下方验证)
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
                material_id: defs.def(STONE).material_binding.material_key_for(Face::PosX),
                flags: RenderLayer::Opaque as u8,
            })
            .collect::<Vec<_>>();
        exp.sort_by_key(|q| (q.face, q.x, q.y, q.z));

        for (a, b) in got.into_iter().zip(exp.into_iter()) {
            assert_eq!(a, b);
        }
    }
    #[test]
    fn packed_quad_roundtrip_with_u16_material() {
        let quad = PackedQuad::new(1, 2, 3, 4, 5, Face::PosZ, 513, 7);
        let decoded = quad.decode();
        assert_eq!(decoded.material_id, 513);
        assert_eq!(decoded.flags, 7);
        assert_eq!(decoded.face, Face::PosZ as u8);
    }

}
