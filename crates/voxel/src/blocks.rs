use cruft_proc_textures::{AlphaMode, TextureRegistry};

pub type BlockStateId = u32;

pub const AIR: BlockStateId = 0;
pub const GRASS: BlockStateId = 1;
pub const DIRT: BlockStateId = 2;
pub const STONE: BlockStateId = 3;
pub const SAND: BlockStateId = 4;
pub const GRAVEL: BlockStateId = 5;
pub const SNOW: BlockStateId = 6;

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
    pub texture_name: &'static str,
    pub material_id: u16,
}

#[derive(Debug, Clone)]
pub struct BlockDefs {
    defs: Vec<BlockDef>,
    resolved: bool,
}

impl Default for BlockDefs {
    fn default() -> Self {
        // 当前只硬编码最小集合：Air/Grass/Dirt/Stone/Sand/Gravel/Snow。
        let defs = vec![
            BlockDef {
                render_layer: RenderLayer::Transparent,
                is_occluder: false,
                texture_name: "",
                material_id: 0,
            }, // AIR
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "grass_block",
                material_id: 1,
            }, // GRASS
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "dirt_block",
                material_id: 2,
            }, // DIRT
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "stone_block",
                material_id: 3,
            }, // STONE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "sand_block",
                material_id: 4,
            }, // SAND
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "gravel_block",
                material_id: 5,
            }, // GRAVEL
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "snow_block",
                material_id: 6,
            }, // SNOW
        ];
        Self {
            defs,
            resolved: false,
        }
    }
}

impl BlockDefs {
    pub fn def(&self, id: BlockStateId) -> BlockDef {
        let idx = id as usize;
        self.defs.get(idx).copied().unwrap_or(BlockDef {
            render_layer: RenderLayer::Opaque,
            is_occluder: true,
            texture_name: "grass_block",
            material_id: 1,
        })
    }

    pub fn is_air(&self, id: BlockStateId) -> bool {
        id == AIR
    }

    pub fn iter(&self) -> impl Iterator<Item = (usize, BlockDef)> + '_ {
        self.defs.iter().copied().enumerate()
    }

    pub fn is_resolved(&self) -> bool {
        self.resolved
    }

    pub fn resolve_from_registry(&mut self, registry: &TextureRegistry) -> Result<(), String> {
        for (index, def) in self.defs.iter_mut().enumerate() {
            if index == AIR as usize || def.texture_name.is_empty() {
                def.material_id = 0;
                def.render_layer = RenderLayer::Transparent;
                continue;
            }
            let texture = registry.get(def.texture_name).ok_or_else(|| {
                format!(
                    "unknown texture `{}` for block state {index}",
                    def.texture_name
                )
            })?;
            def.material_id = index as u16;
            def.render_layer = match texture.alpha_mode {
                AlphaMode::Opaque => RenderLayer::Opaque,
                AlphaMode::Mask => RenderLayer::Cutout,
                AlphaMode::Blend => RenderLayer::Transparent,
            };
        }
        self.resolved = true;
        Ok(())
    }
}
