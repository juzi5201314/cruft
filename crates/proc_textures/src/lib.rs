//! 程序化贴图（GPU compute / texture array）生成服务。

mod plugin;

pub use plugin::{
    BlockTextureArray, BlockTextureFaceMapping, BlockTextureFaceMappings, ProcTexturesPlugin,
    ProcTexturesReady, ProcTexturesStatus,
};
