use crate::meshing::Face;
use cruft_proc_textures::TextureRegistry;

pub type BlockStateId = u32;
pub type MaterialId = u16;

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
pub enum BlockMaterialBinding {
    Single(MaterialId),
    ByFace {
        top: MaterialId,
        bottom: MaterialId,
        sides: MaterialId,
    },
    Explicit {
        pos_x: MaterialId,
        neg_x: MaterialId,
        pos_y: MaterialId,
        neg_y: MaterialId,
        pos_z: MaterialId,
        neg_z: MaterialId,
    },
}

impl BlockMaterialBinding {
    pub fn material_key_for(self, face: Face) -> MaterialId {
        match self {
            Self::Single(id) => id,
            Self::ByFace { top, bottom, sides } => match face {
                Face::PosY => top,
                Face::NegY => bottom,
                Face::PosX | Face::NegX | Face::PosZ | Face::NegZ => sides,
            },
            Self::Explicit {
                pos_x,
                neg_x,
                pos_y,
                neg_y,
                pos_z,
                neg_z,
            } => match face {
                Face::PosX => pos_x,
                Face::NegX => neg_x,
                Face::PosY => pos_y,
                Face::NegY => neg_y,
                Face::PosZ => pos_z,
                Face::NegZ => neg_z,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockDef {
    pub render_layer: RenderLayer,
    pub is_occluder: bool,
    pub material_binding: BlockMaterialBinding,
}

#[derive(Debug, Clone)]
pub struct BlockDefs {
    defs: Vec<BlockDef>,
}

impl Default for BlockDefs {
    fn default() -> Self {
        // Fallback used before texture registry resolution.
        let mut defs = Vec::new();
        defs.push(BlockDef {
            render_layer: RenderLayer::Transparent,
            is_occluder: false,
            material_binding: BlockMaterialBinding::Single(0),
        });
        defs.push(BlockDef {
            render_layer: RenderLayer::Opaque,
            is_occluder: true,
            material_binding: BlockMaterialBinding::ByFace {
                top: 0,
                bottom: 1,
                sides: 1,
            },
        });
        defs.push(BlockDef {
            render_layer: RenderLayer::Opaque,
            is_occluder: true,
            material_binding: BlockMaterialBinding::Single(1),
        });
        defs.push(BlockDef {
            render_layer: RenderLayer::Opaque,
            is_occluder: true,
            material_binding: BlockMaterialBinding::Single(3),
        });
        Self { defs }
    }
}

impl BlockDefs {
    pub fn from_registry(registry: &TextureRegistry) -> Result<Self, String> {
        let grass = registry.layer_index("minecraft_grass")?;
        let dirt = registry.layer_index("minecraft_dirt")?;
        let stone = registry.layer_index("minecraft_stone")?;

        Ok(Self {
            defs: vec![
                BlockDef {
                    render_layer: RenderLayer::Transparent,
                    is_occluder: false,
                    material_binding: BlockMaterialBinding::Single(0),
                },
                BlockDef {
                    render_layer: RenderLayer::Opaque,
                    is_occluder: true,
                    material_binding: BlockMaterialBinding::ByFace {
                        top: grass,
                        bottom: dirt,
                        sides: dirt,
                    },
                },
                BlockDef {
                    render_layer: RenderLayer::Opaque,
                    is_occluder: true,
                    material_binding: BlockMaterialBinding::Single(dirt),
                },
                BlockDef {
                    render_layer: RenderLayer::Opaque,
                    is_occluder: true,
                    material_binding: BlockMaterialBinding::Single(stone),
                },
            ],
        })
    }

    pub fn def(&self, id: BlockStateId) -> BlockDef {
        let idx = id as usize;
        self.defs.get(idx).copied().unwrap_or(BlockDef {
            render_layer: RenderLayer::Opaque,
            is_occluder: true,
            material_binding: BlockMaterialBinding::Single(0),
        })
    }

    pub fn is_air(&self, id: BlockStateId) -> bool {
        id == AIR
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_binding_resolves_per_face() {
        let binding = BlockMaterialBinding::ByFace {
            top: 10,
            bottom: 11,
            sides: 12,
        };

        assert_eq!(binding.material_key_for(Face::PosY), 10);
        assert_eq!(binding.material_key_for(Face::NegY), 11);
        assert_eq!(binding.material_key_for(Face::PosX), 12);
    }
    #[test]
    fn block_defs_from_registry_missing_name_fails() {
        let registry = TextureRegistry::default();
        assert!(BlockDefs::from_registry(&registry).is_err());
    }

}
