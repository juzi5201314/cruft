//! 程序化贴图：严格解析、规范化编译、CPU 参考生成、运行时上传与导出。

mod compiler;
mod error;
mod export;
mod generator;
mod plugin;
mod schema;

pub use compiler::{
    compile_texture_set, load_and_compile_texture_set, AlphaMode, CanonicalTextureSet,
    ChannelLayerRef, ChannelPackKind, CompiledTextureSet, CubeFace, FaceChannels, ResolvedFace,
    ResolvedTexture, TextureFingerprint, TextureRegistry, TextureRuntimePack, TextureRuntimePacks,
    TextureSamplerSpec,
};
pub use error::{TextureDataError, TextureDataErrorKind};
pub use export::{export_compiled_texture_set_to_dir, ExportManifest, ExportedTextureSet};
pub use generator::{
    build_runtime_texture_assets, GeneratedFaceLayer, GeneratedTextureSet, MipLevel,
    RuntimeTextureBuild, RuntimeTexturePackData,
};
pub use plugin::{ProcTexturesPlugin, ProcTexturesStatus};
