/// 极简 2D heightmap：不引入外部依赖，使用确定性 hash 噪声。
#[derive(Debug, Clone, Copy)]
pub struct Terrain {
    pub seed: u64,
    pub base_height: i32,
    pub amplitude: i32,
}

impl Default for Terrain {
    fn default() -> Self {
        Self {
            seed: 0xC2B2_AE3D_27D4_EB4F,
            base_height: 32,
            amplitude: 20,
        }
    }
}

impl Terrain {
    pub fn height_at(&self, wx: i32, wz: i32) -> i32 {
        let n = hash2(wx, wz, self.seed);
        let t = (n & 0xFFFF) as i32; // 0..65535
        let centered = t - 32768;
        self.base_height + (centered * self.amplitude) / 32768
    }
}

fn hash2(x: i32, z: i32, seed: u64) -> u64 {
    // splitmix64 风格混合；足够做地形占位。
    let mut v = seed ^ ((x as u32 as u64) << 32) ^ (z as u32 as u64);
    v = v.wrapping_add(0x9E37_79B9_7F4A_7C15);
    v = (v ^ (v >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    v = (v ^ (v >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    v ^ (v >> 31)
}
