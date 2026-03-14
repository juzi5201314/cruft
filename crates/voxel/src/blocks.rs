use cruft_proc_textures::{AlphaMode, TextureRegistry};

pub type BlockStateId = u32;

pub const AIR: BlockStateId = 0;
pub const GRASS: BlockStateId = 1;
pub const DIRT: BlockStateId = 2;
pub const STONE: BlockStateId = 3;
pub const SAND: BlockStateId = 4;
pub const GRAVEL: BlockStateId = 5;
pub const SNOW: BlockStateId = 6;
pub const OAK_LOG: BlockStateId = 7;
pub const OAK_PLANKS: BlockStateId = 8;
pub const OAK_LEAVES: BlockStateId = 9;
pub const BIRCH_LOG: BlockStateId = 10;
pub const BIRCH_PLANKS: BlockStateId = 11;
pub const BIRCH_LEAVES: BlockStateId = 12;
pub const SPRUCE_LOG: BlockStateId = 13;
pub const SPRUCE_PLANKS: BlockStateId = 14;
pub const SPRUCE_LEAVES: BlockStateId = 15;
pub const JUNGLE_LOG: BlockStateId = 16;
pub const JUNGLE_PLANKS: BlockStateId = 17;
pub const JUNGLE_LEAVES: BlockStateId = 18;
pub const ACACIA_LOG: BlockStateId = 19;
pub const ACACIA_PLANKS: BlockStateId = 20;
pub const ACACIA_LEAVES: BlockStateId = 21;
pub const DARK_OAK_LOG: BlockStateId = 22;
pub const DARK_OAK_PLANKS: BlockStateId = 23;
pub const DARK_OAK_LEAVES: BlockStateId = 24;
pub const COAL_ORE: BlockStateId = 25;
pub const IRON_ORE: BlockStateId = 26;
pub const GOLD_ORE: BlockStateId = 27;
pub const DIAMOND_ORE: BlockStateId = 28;
pub const REDSTONE_ORE: BlockStateId = 29;
pub const EMERALD_ORE: BlockStateId = 30;
pub const LAPIS_ORE: BlockStateId = 31;
pub const COPPER_ORE: BlockStateId = 32;
pub const DEEPSLATE_COAL_ORE: BlockStateId = 33;
pub const DEEPSLATE_IRON_ORE: BlockStateId = 34;
pub const DEEPSLATE_GOLD_ORE: BlockStateId = 35;
pub const DEEPSLATE_DIAMOND_ORE: BlockStateId = 36;
pub const DEEPSLATE_REDSTONE_ORE: BlockStateId = 37;
pub const DEEPSLATE_EMERALD_ORE: BlockStateId = 38;
pub const DEEPSLATE_LAPIS_ORE: BlockStateId = 39;
pub const DEEPSLATE_COPPER_ORE: BlockStateId = 40;
pub const NETHERRACK: BlockStateId = 41;
pub const SOUL_SAND: BlockStateId = 42;
pub const SOUL_SOIL: BlockStateId = 43;
pub const NETHER_QUARTZ_ORE: BlockStateId = 44;
pub const NETHER_GOLD_ORE: BlockStateId = 45;
pub const COBBLESTONE: BlockStateId = 46;
pub const MOSSY_COBBLESTONE: BlockStateId = 47;
pub const DIORITE: BlockStateId = 48;
pub const ANDESITE: BlockStateId = 49;
pub const GRANITE: BlockStateId = 50;
pub const SANDSTONE: BlockStateId = 51;
pub const OBSIDIAN: BlockStateId = 52;
pub const BEDROCK: BlockStateId = 53;
pub const DEEPSLATE: BlockStateId = 54;
pub const END_STONE: BlockStateId = 55;
pub const BRICKS: BlockStateId = 56;
pub const STONE_BRICKS: BlockStateId = 57;
pub const GLASS: BlockStateId = 58;
pub const CLAY: BlockStateId = 59;
pub const MYCELIUM: BlockStateId = 60;
pub const PODZOL: BlockStateId = 61;
pub const WHITE_WOOL: BlockStateId = 62;
pub const BLACK_WOOL: BlockStateId = 63;
pub const RED_WOOL: BlockStateId = 64;
pub const TNT: BlockStateId = 65;
pub const BOOKSHELF: BlockStateId = 66;
pub const SPONGE: BlockStateId = 67;
pub const WHITE_TERRACOTTA: BlockStateId = 68;
pub const ORANGE_TERRACOTTA: BlockStateId = 69;
pub const MAGENTA_TERRACOTTA: BlockStateId = 70;
pub const LIGHT_BLUE_TERRACOTTA: BlockStateId = 71;
pub const YELLOW_TERRACOTTA: BlockStateId = 72;
pub const LIME_TERRACOTTA: BlockStateId = 73;
pub const PINK_TERRACOTTA: BlockStateId = 74;
pub const GRAY_TERRACOTTA: BlockStateId = 75;
pub const WHITE_CONCRETE: BlockStateId = 76;
pub const ORANGE_CONCRETE: BlockStateId = 77;
pub const MAGENTA_CONCRETE: BlockStateId = 78;
pub const LIGHT_BLUE_CONCRETE: BlockStateId = 79;
pub const YELLOW_CONCRETE: BlockStateId = 80;
pub const LIME_CONCRETE: BlockStateId = 81;
pub const PINK_CONCRETE: BlockStateId = 82;
pub const GRAY_CONCRETE: BlockStateId = 83;

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
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "oak_log_block",
                material_id: 7,
            }, // OAK_LOG
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "oak_planks_block",
                material_id: 8,
            }, // OAK_PLANKS
            BlockDef {
                render_layer: RenderLayer::Transparent,
                is_occluder: false,
                texture_name: "oak_leaves_block",
                material_id: 9,
            }, // OAK_LEAVES
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "birch_log_block",
                material_id: 10,
            }, // BIRCH_LOG
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "birch_planks_block",
                material_id: 11,
            }, // BIRCH_PLANKS
            BlockDef {
                render_layer: RenderLayer::Transparent,
                is_occluder: false,
                texture_name: "birch_leaves_block",
                material_id: 12,
            }, // BIRCH_LEAVES
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "spruce_log_block",
                material_id: 13,
            }, // SPRUCE_LOG
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "spruce_planks_block",
                material_id: 14,
            }, // SPRUCE_PLANKS
            BlockDef {
                render_layer: RenderLayer::Transparent,
                is_occluder: false,
                texture_name: "spruce_leaves_block",
                material_id: 15,
            }, // SPRUCE_LEAVES
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "jungle_log_block",
                material_id: 16,
            }, // JUNGLE_LOG
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "jungle_planks_block",
                material_id: 17,
            }, // JUNGLE_PLANKS
            BlockDef {
                render_layer: RenderLayer::Transparent,
                is_occluder: false,
                texture_name: "jungle_leaves_block",
                material_id: 18,
            }, // JUNGLE_LEAVES
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "acacia_log_block",
                material_id: 19,
            }, // ACACIA_LOG
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "acacia_planks_block",
                material_id: 20,
            }, // ACACIA_PLANKS
            BlockDef {
                render_layer: RenderLayer::Transparent,
                is_occluder: false,
                texture_name: "acacia_leaves_block",
                material_id: 21,
            }, // ACACIA_LEAVES
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "dark_oak_log_block",
                material_id: 22,
            }, // DARK_OAK_LOG
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "dark_oak_planks_block",
                material_id: 23,
            }, // DARK_OAK_PLANKS
            BlockDef {
                render_layer: RenderLayer::Transparent,
                is_occluder: false,
                texture_name: "dark_oak_leaves_block",
                material_id: 24,
            }, // DARK_OAK_LEAVES
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "coal_ore_block",
                material_id: 25,
            }, // COAL_ORE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "iron_ore_block",
                material_id: 26,
            }, // IRON_ORE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "gold_ore_block",
                material_id: 27,
            }, // GOLD_ORE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "diamond_ore_block",
                material_id: 28,
            }, // DIAMOND_ORE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "redstone_ore_block",
                material_id: 29,
            }, // REDSTONE_ORE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "emerald_ore_block",
                material_id: 30,
            }, // EMERALD_ORE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "lapis_ore_block",
                material_id: 31,
            }, // LAPIS_ORE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "copper_ore_block",
                material_id: 32,
            }, // COPPER_ORE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "deepslate_coal_ore_block",
                material_id: 33,
            }, // DEEPSLATE_COAL_ORE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "deepslate_iron_ore_block",
                material_id: 34,
            }, // DEEPSLATE_IRON_ORE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "deepslate_gold_ore_block",
                material_id: 35,
            }, // DEEPSLATE_GOLD_ORE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "deepslate_diamond_ore_block",
                material_id: 36,
            }, // DEEPSLATE_DIAMOND_ORE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "deepslate_redstone_ore_block",
                material_id: 37,
            }, // DEEPSLATE_REDSTONE_ORE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "deepslate_emerald_ore_block",
                material_id: 38,
            }, // DEEPSLATE_EMERALD_ORE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "deepslate_lapis_ore_block",
                material_id: 39,
            }, // DEEPSLATE_LAPIS_ORE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "deepslate_copper_ore_block",
                material_id: 40,
            }, // DEEPSLATE_COPPER_ORE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "netherrack_block",
                material_id: 41,
            }, // NETHERRACK
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "soul_sand_block",
                material_id: 42,
            }, // SOUL_SAND
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "soul_soil_block",
                material_id: 43,
            }, // SOUL_SOIL
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "nether_quartz_ore_block",
                material_id: 44,
            }, // NETHER_QUARTZ_ORE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "nether_gold_ore_block",
                material_id: 45,
            }, // NETHER_GOLD_ORE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "cobblestone_block",
                material_id: 46,
            }, // COBBLESTONE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "mossy_cobblestone_block",
                material_id: 47,
            }, // MOSSY_COBBLESTONE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "diorite_block",
                material_id: 48,
            }, // DIORITE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "andesite_block",
                material_id: 49,
            }, // ANDESITE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "granite_block",
                material_id: 50,
            }, // GRANITE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "sandstone_block",
                material_id: 51,
            }, // SANDSTONE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "obsidian_block",
                material_id: 52,
            }, // OBSIDIAN
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "bedrock_block",
                material_id: 53,
            }, // BEDROCK
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "deepslate_block",
                material_id: 54,
            }, // DEEPSLATE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "end_stone_block",
                material_id: 55,
            }, // END_STONE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "bricks_block",
                material_id: 56,
            }, // BRICKS
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "stone_bricks_block",
                material_id: 57,
            }, // STONE_BRICKS
            BlockDef {
                render_layer: RenderLayer::Transparent,
                is_occluder: false,
                texture_name: "glass_block",
                material_id: 58,
            }, // GLASS
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "clay_block",
                material_id: 59,
            }, // CLAY
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "mycelium_block",
                material_id: 60,
            }, // MYCELIUM
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "podzol_block",
                material_id: 61,
            }, // PODZOL
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "white_wool_block",
                material_id: 62,
            }, // WHITE_WOOL
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "black_wool_block",
                material_id: 63,
            }, // BLACK_WOOL
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "red_wool_block",
                material_id: 64,
            }, // RED_WOOL
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "tnt_block",
                material_id: 65,
            }, // TNT
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "bookshelf_block",
                material_id: 66,
            }, // BOOKSHELF
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "sponge_block",
                material_id: 67,
            }, // SPONGE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "white_terracotta_block",
                material_id: 68,
            }, // WHITE_TERRACOTTA
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "orange_terracotta_block",
                material_id: 69,
            }, // ORANGE_TERRACOTTA
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "magenta_terracotta_block",
                material_id: 70,
            }, // MAGENTA_TERRACOTTA
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "light_blue_terracotta_block",
                material_id: 71,
            }, // LIGHT_BLUE_TERRACOTTA
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "yellow_terracotta_block",
                material_id: 72,
            }, // YELLOW_TERRACOTTA
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "lime_terracotta_block",
                material_id: 73,
            }, // LIME_TERRACOTTA
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "pink_terracotta_block",
                material_id: 74,
            }, // PINK_TERRACOTTA
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "gray_terracotta_block",
                material_id: 75,
            }, // GRAY_TERRACOTTA
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "white_concrete_block",
                material_id: 76,
            }, // WHITE_CONCRETE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "orange_concrete_block",
                material_id: 77,
            }, // ORANGE_CONCRETE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "magenta_concrete_block",
                material_id: 78,
            }, // MAGENTA_CONCRETE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "light_blue_concrete_block",
                material_id: 79,
            }, // LIGHT_BLUE_CONCRETE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "yellow_concrete_block",
                material_id: 80,
            }, // YELLOW_CONCRETE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "lime_concrete_block",
                material_id: 81,
            }, // LIME_CONCRETE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "pink_concrete_block",
                material_id: 82,
            }, // PINK_CONCRETE
            BlockDef {
                render_layer: RenderLayer::Opaque,
                is_occluder: true,
                texture_name: "gray_concrete_block",
                material_id: 83,
            }, // GRAY_CONCRETE

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
