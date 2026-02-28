use std::path::PathBuf;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// 存档根目录。
#[derive(Resource, Debug, Clone)]
pub struct SaveRootDir(pub PathBuf);

impl SaveRootDir {
    /// 默认目录：体素引擎阶段的内存存档占位实现不再解析平台目录。
    pub fn default_path() -> PathBuf {
        PathBuf::from("./saves")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SaveId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveMeta {
    pub id: String,
    pub display_name: String,
    pub created_at: i64,
    pub last_played_at: i64,
    pub format_version: u32,
}

#[derive(Debug, Clone)]
pub struct LoadedSave {
    pub meta: SaveMeta,
}
