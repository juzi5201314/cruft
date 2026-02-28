use std::path::PathBuf;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// 存档根目录。
#[derive(Resource, Debug, Clone)]
pub struct SaveRootDir(pub PathBuf);

impl SaveRootDir {
    /// 平台默认目录；如果无法解析则回落到 `./saves`。
    pub fn default_path() -> PathBuf {
        directories::ProjectDirs::from("dev.soeur", "BoxCat", "Cruft")
            .map(|d| d.data_dir().join("saves"))
            .unwrap_or_else(|| PathBuf::from("./saves"))
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
