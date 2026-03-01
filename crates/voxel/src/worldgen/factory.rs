use cruft_worldgen_spec::{WorldGenConfig, WorldGenPreset};

use super::modern_surface::ModernSurfaceGenerator;
use super::WorldGenerator;

pub fn build_generator(config: &WorldGenConfig) -> Box<dyn WorldGenerator> {
    match config.preset {
        WorldGenPreset::ModernSurface => Box::new(ModernSurfaceGenerator::new(config.clone())),
    }
}
