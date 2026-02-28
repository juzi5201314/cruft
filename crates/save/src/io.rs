use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::types::{SaveId, SaveMeta};

pub const META_FILE: &str = "meta.json";
pub const WORLD_FILE: &str = "world.bin";
pub const PLAYER_FILE: &str = "player.json";
pub const FORMAT_VERSION: u32 = 1;

pub fn ensure_root(root: &Path) -> io::Result<()> {
    fs::create_dir_all(root)?;
    Ok(())
}

pub fn save_dir(root: &Path, id: &SaveId) -> PathBuf {
    root.join(&id.0)
}

pub fn read_meta(dir: &Path) -> io::Result<SaveMeta> {
    let p = dir.join(META_FILE);
    let bytes = fs::read(p)?;
    let meta: SaveMeta = serde_json::from_slice(&bytes)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(meta)
}

pub fn write_meta(dir: &Path, meta: &SaveMeta) -> io::Result<()> {
    let p = dir.join(META_FILE);
    let bytes = serde_json::to_vec_pretty(meta)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    fs::write(p, bytes)?;
    Ok(())
}

pub fn new_save_id() -> SaveId {
    SaveId(Uuid::new_v4().to_string())
}

pub fn create_new_save(root: &Path, display_name: String) -> io::Result<SaveMeta> {
    ensure_root(root)?;
    let id = new_save_id();
    let dir = save_dir(root, &id);
    fs::create_dir_all(&dir)?;

    let now = OffsetDateTime::now_utc().unix_timestamp();
    let meta = SaveMeta {
        id: id.0.clone(),
        display_name,
        created_at: now,
        last_played_at: now,
        format_version: FORMAT_VERSION,
    };

    write_meta(&dir, &meta)?;
    fs::write(dir.join(WORLD_FILE), [])?;

    Ok(meta)
}

pub fn rename_save(root: &Path, id: &SaveId, new_name: String) -> io::Result<SaveMeta> {
    let dir = save_dir(root, id);
    let mut meta = read_meta(&dir)?;
    meta.display_name = new_name;
    write_meta(&dir, &meta)?;
    Ok(meta)
}

pub fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else if file_type.is_file() {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

pub fn copy_save(root: &Path, id: &SaveId) -> io::Result<SaveMeta> {
    ensure_root(root)?;
    let src_dir = save_dir(root, id);
    let src_meta = read_meta(&src_dir)?;

    let new_id = new_save_id();
    let dst_dir = save_dir(root, &new_id);
    copy_dir_recursive(&src_dir, &dst_dir)?;

    let mut meta = src_meta;
    meta.id = new_id.0.clone();
    meta.display_name = format!("{} Copy", meta.display_name);
    write_meta(&dst_dir, &meta)?;

    Ok(meta)
}

pub fn soft_delete_save(root: &Path, id: &SaveId) -> io::Result<()> {
    let dir = save_dir(root, id);
    if !dir.exists() {
        return Ok(());
    }

    let trash = root.join(".trash");
    fs::create_dir_all(&trash)?;

    let ts = OffsetDateTime::now_utc().unix_timestamp();
    let target = trash.join(format!("{}_{}", id.0, ts));

    fs::rename(dir, target)?;
    Ok(())
}

pub fn scan_index(root: &Path) -> io::Result<Vec<SaveMeta>> {
    ensure_root(root)?;

    let mut out = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if !file_type.is_dir() {
            continue;
        }
        let name = entry.file_name();
        if name == OsStr::new(".trash") {
            continue;
        }

        let dir = entry.path();
        let meta_path = dir.join(META_FILE);
        if !meta_path.exists() {
            continue;
        }

        match read_meta(&dir) {
            Ok(meta) => out.push(meta),
            Err(_) => {
                // 忽略损坏 meta（后续可加修复/提示）
            }
        }
    }

    out.sort_by(|a, b| b.last_played_at.cmp(&a.last_played_at));
    Ok(out)
}

pub fn load_save_minimal(root: &Path, id: &SaveId) -> io::Result<SaveMeta> {
    let dir = save_dir(root, id);
    let meta = read_meta(&dir)?;

    // world.bin 至少应存在（第一版可为空占位）。
    let world_path = dir.join(WORLD_FILE);
    if !world_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("missing {WORLD_FILE} for save {}", id.0),
        ));
    }

    // 可选 player.json，第一版只校验 JSON 可解析。
    let player_path = dir.join(PLAYER_FILE);
    if player_path.exists() {
        let bytes = fs::read(player_path)?;
        let _: Value = serde_json::from_slice(&bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    }

    Ok(meta)
}

