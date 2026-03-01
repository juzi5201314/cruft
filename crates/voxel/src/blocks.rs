pub type BlockStateId = u32;

pub const AIR: BlockStateId = 0;
pub const GRASS: BlockStateId = 1;
pub const DIRT: BlockStateId = 2;
pub const STONE: BlockStateId = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderLayer {
    Opaque = 0,
    Cutout = 1,
    Transparent = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockDef {
    pub render_layer: RenderLayer,
    pub is_occluder: bool,
    pub material_key: u8,
}

#[derive(Debug, Clone)]
pub struct BlockDefs {
    defs: Vec<BlockDef>,
}

impl Default for BlockDefs {
    fn default() -> Self {
        // 当前只硬编码最小集合：Air/Grass/Dirt/Stone。
        let mut defs = Vec::new();
        defs.push(BlockDef {
            render_layer: RenderLayer::Transparent,
            is_occluder: false,
            material_key: 0,
        });
        defs.push(BlockDef {
            render_layer: RenderLayer::Opaque,
            is_occluder: true,
            // legacy material_key：grass primary face layer=0
            material_key: 0,
        });
        defs.push(BlockDef {
            render_layer: RenderLayer::Opaque,
            is_occluder: true,
            // legacy material_key：dirt primary face layer=1
            material_key: 1,
        });
        defs.push(BlockDef {
            render_layer: RenderLayer::Opaque,
            is_occluder: true,
            // legacy material_key：stone primary face layer=3（snowy_dirt=2）
            material_key: 3,
        });
        Self { defs }
    }
}

impl BlockDefs {
    pub fn def(&self, id: BlockStateId) -> BlockDef {
        let idx = id as usize;
        self.defs.get(idx).copied().unwrap_or(BlockDef {
            render_layer: RenderLayer::Opaque,
            is_occluder: true,
            material_key: 0,
        })
    }

    pub fn is_air(&self, id: BlockStateId) -> bool {
        id == AIR
    }
}
