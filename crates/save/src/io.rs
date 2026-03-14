use std::cmp::Reverse;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use cruft_worldgen_spec::{
    WorldGenConfig, WorldGenPreset, WorldHeaderV2, WORLD_FORMAT_VERSION_V2, WORLD_HEADER_FILE,
};
use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::types::{LoadedSave, SaveId, SaveMeta};

pub const META_FILE: &str = "meta.json";
pub const WORLD_FILE: &str = "world.bin";
pub const PLAYER_FILE: &str = "player.json";
pub const WORLD_FORMAT_VERSION: u32 = WORLD_FORMAT_VERSION_V2;

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

pub fn read_world_header(dir: &Path) -> io::Result<WorldHeaderV2> {
    let p = dir.join(WORLD_HEADER_FILE);
    let bytes = fs::read(p)?;
    let header: WorldHeaderV2 = serde_json::from_slice(&bytes)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    header
        .validate()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(header)
}

pub fn write_world_header(dir: &Path, header: &WorldHeaderV2) -> io::Result<()> {
    let p = dir.join(WORLD_HEADER_FILE);
    let bytes = serde_json::to_vec_pretty(header)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    fs::write(p, bytes)?;
    Ok(())
}

pub fn new_save_id() -> SaveId {
    SaveId(Uuid::new_v4().to_string())
}

pub fn create_new_save(
    root: &Path,
    display_name: String,
    generation: u64,
    generator_preset: WorldGenPreset,
) -> io::Result<LoadedSave> {
    ensure_root(root)?;
    let id = new_save_id();
    let dir = save_dir(root, &id);
    fs::create_dir_all(&dir)?;

    let now = OffsetDateTime::now_utc().unix_timestamp();
    let meta = SaveMeta {
        id: id.0.clone(),
        display_name: display_name.clone(),
        created_at: now,
        last_played_at: now,
        world_format_version: WORLD_FORMAT_VERSION,
    };

    let seed = derive_world_seed(display_name.as_bytes(), generation);
    let generator = match generator_preset {
        WorldGenPreset::ModernSurface => WorldGenConfig::modern_surface(seed),
        WorldGenPreset::Superflat => WorldGenConfig::superflat(seed),
    };
    let header = WorldHeaderV2::new(Uuid::new_v4().to_string(), now, generator);

    write_meta(&dir, &meta)?;
    write_world_header(&dir, &header)?;
    fs::write(dir.join(WORLD_FILE), [])?;

    Ok(LoadedSave {
        meta,
        world_header: header,
    })
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

    out.sort_by_key(|m| Reverse(m.last_played_at));
    Ok(out)
}

pub fn load_save(root: &Path, id: &SaveId) -> io::Result<LoadedSave> {
    let dir = save_dir(root, id);
    let mut meta = read_meta(&dir)?;

    if meta.world_format_version != WORLD_FORMAT_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported world format: {}, expected {}",
                meta.world_format_version, WORLD_FORMAT_VERSION
            ),
        ));
    }

    let world_path = dir.join(WORLD_FILE);
    if !world_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("missing {WORLD_FILE} for save {}", id.0),
        ));
    }

    let header = read_world_header(&dir)?;

    let player_path = dir.join(PLAYER_FILE);
    if player_path.exists() {
        let bytes = fs::read(player_path)?;
        let _: Value = serde_json::from_slice(&bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    }

    meta.last_played_at = OffsetDateTime::now_utc().unix_timestamp();
    write_meta(&dir, &meta)?;

    Ok(LoadedSave {
        meta,
        world_header: header,
    })
}

fn derive_world_seed(bytes: &[u8], generation: u64) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h ^ generation.wrapping_mul(0x9E37_79B9_7F4A_7C15)
}
