use std::io;
use std::path::{Path, PathBuf};

use redb::{Database, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::types::{SaveId, SaveMeta};

pub const FORMAT_VERSION: u32 = 3;
const DB_FILE: &str = "saves.redb";
// 使用 zstd fast mode（负级别）优先写入速度，适合频繁存档场景。
const ZSTD_LEVEL: i32 = -7;

const META_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("meta");
const PAYLOAD_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("payload");
const TRASH_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("trash");

#[derive(Debug, Serialize, Deserialize)]
struct SavePayload {
    world: Vec<u8>,
    player_json: Option<Vec<u8>>,
}

fn io_other<E: std::fmt::Display>(e: E) -> io::Error {
    io::Error::other(e.to_string())
}

pub fn ensure_root(root: &Path) -> io::Result<()> {
    std::fs::create_dir_all(root)?;
    Ok(())
}

fn db_path(root: &Path) -> PathBuf {
    root.join(DB_FILE)
}

fn open_db(root: &Path) -> io::Result<Database> {
    ensure_root(root)?;
    let db = Database::create(db_path(root)).map_err(io_other)?;
    let write = db.begin_write().map_err(io_other)?;
    {
        let _ = write.open_table(META_TABLE).map_err(io_other)?;
        let _ = write.open_table(PAYLOAD_TABLE).map_err(io_other)?;
        let _ = write.open_table(TRASH_TABLE).map_err(io_other)?;
    }
    write.commit().map_err(io_other)?;
    Ok(db)
}

fn serialize_meta(meta: &SaveMeta) -> io::Result<Vec<u8>> {
    postcard::to_stdvec(meta).map_err(io_other)
}

fn deserialize_meta(bytes: &[u8]) -> io::Result<SaveMeta> {
    postcard::from_bytes(bytes).map_err(io_other)
}

fn compress_bytes(raw: &[u8]) -> io::Result<Vec<u8>> {
    let mut compressor = zstd::bulk::Compressor::new(ZSTD_LEVEL).map_err(io_other)?;
    compressor.compress(raw).map_err(io_other)
}

fn decompress_bytes(bytes: &[u8]) -> io::Result<Vec<u8>> {
    let mut decompressor = zstd::bulk::Decompressor::new().map_err(io_other)?;
    decompressor.decompress(bytes, usize::MAX).map_err(io_other)
}

fn serialize_payload(payload: &SavePayload) -> io::Result<Vec<u8>> {
    let raw = postcard::to_stdvec(payload).map_err(io_other)?;
    compress_bytes(&raw)
}

fn deserialize_payload(bytes: &[u8]) -> io::Result<SavePayload> {
    let raw = decompress_bytes(bytes)?;
    postcard::from_bytes(&raw).map_err(io_other)
}

pub fn new_save_id() -> SaveId {
    SaveId(Uuid::new_v4().to_string())
}

pub fn create_new_save(root: &Path, display_name: String) -> io::Result<SaveMeta> {
    let db = open_db(root)?;
    let id = new_save_id();
    let now = OffsetDateTime::now_utc().unix_timestamp();

    let meta = SaveMeta {
        id: id.0.clone(),
        display_name,
        created_at: now,
        last_played_at: now,
        format_version: FORMAT_VERSION,
    };
    let payload = SavePayload {
        world: Vec::new(),
        player_json: None,
    };

    let meta_bytes = serialize_meta(&meta)?;
    let payload_bytes = serialize_payload(&payload)?;

    let write = db.begin_write().map_err(io_other)?;
    {
        let mut metas = write.open_table(META_TABLE).map_err(io_other)?;
        let mut payloads = write.open_table(PAYLOAD_TABLE).map_err(io_other)?;
        metas.insert(meta.id.as_str(), meta_bytes.as_slice())
            .map_err(io_other)?;
        payloads
            .insert(meta.id.as_str(), payload_bytes.as_slice())
            .map_err(io_other)?;
    }
    write.commit().map_err(io_other)?;

    Ok(meta)
}

fn read_save_from_db(db: &Database, id: &SaveId) -> io::Result<(SaveMeta, SavePayload)> {
    let read = db.begin_read().map_err(io_other)?;
    let metas = read.open_table(META_TABLE).map_err(io_other)?;
    let payloads = read.open_table(PAYLOAD_TABLE).map_err(io_other)?;

    let meta_bytes = metas
        .get(id.0.as_str())
        .map_err(io_other)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "save meta not found"))?;
    let payload_bytes = payloads
        .get(id.0.as_str())
        .map_err(io_other)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "save payload not found"))?;

    let meta = deserialize_meta(meta_bytes.value())?;
    let payload = deserialize_payload(payload_bytes.value())?;
    Ok((meta, payload))
}

fn write_save_to_db(db: &Database, meta: &SaveMeta, payload: &SavePayload) -> io::Result<()> {
    let meta_bytes = serialize_meta(meta)?;
    let payload_bytes = serialize_payload(payload)?;

    let write = db.begin_write().map_err(io_other)?;
    {
        let mut metas = write.open_table(META_TABLE).map_err(io_other)?;
        let mut payloads = write.open_table(PAYLOAD_TABLE).map_err(io_other)?;
        metas.insert(meta.id.as_str(), meta_bytes.as_slice())
            .map_err(io_other)?;
        payloads
            .insert(meta.id.as_str(), payload_bytes.as_slice())
            .map_err(io_other)?;
    }
    write.commit().map_err(io_other)
}

pub fn rename_save(root: &Path, id: &SaveId, new_name: String) -> io::Result<SaveMeta> {
    let db = open_db(root)?;
    let (mut meta, payload) = read_save_from_db(&db, id)?;
    meta.display_name = new_name;
    write_save_to_db(&db, &meta, &payload)?;
    Ok(meta)
}

pub fn copy_save(root: &Path, id: &SaveId) -> io::Result<SaveMeta> {
    let db = open_db(root)?;
    let (mut meta, payload) = read_save_from_db(&db, id)?;

    let new_id = new_save_id();
    meta.id = new_id.0;
    meta.display_name = format!("{} Copy", meta.display_name);
    meta.last_played_at = OffsetDateTime::now_utc().unix_timestamp();
    meta.format_version = FORMAT_VERSION;

    write_save_to_db(&db, &meta, &payload)?;
    Ok(meta)
}

pub fn soft_delete_save(root: &Path, id: &SaveId) -> io::Result<()> {
    let db = open_db(root)?;
    let read = db.begin_read().map_err(io_other)?;
    let metas = read.open_table(META_TABLE).map_err(io_other)?;
    let payloads = read.open_table(PAYLOAD_TABLE).map_err(io_other)?;

    let meta = metas.get(id.0.as_str()).map_err(io_other)?;
    let payload = payloads.get(id.0.as_str()).map_err(io_other)?;
    drop(metas);
    drop(payloads);
    drop(read);

    let Some(meta) = meta else {
        return Ok(());
    };
    let Some(payload) = payload else {
        return Ok(());
    };

    let deleted_at = OffsetDateTime::now_utc().unix_timestamp();
    let trash_key = format!("{}:{}", id.0, deleted_at);

    let write = db.begin_write().map_err(io_other)?;
    {
        let mut meta_table = write.open_table(META_TABLE).map_err(io_other)?;
        let mut payload_table = write.open_table(PAYLOAD_TABLE).map_err(io_other)?;
        let mut trash_table = write.open_table(TRASH_TABLE).map_err(io_other)?;

        trash_table
            .insert(trash_key.as_str(), meta.value())
            .map_err(io_other)?;
        trash_table
            .insert(format!("{trash_key}:payload").as_str(), payload.value())
            .map_err(io_other)?;

        meta_table.remove(id.0.as_str()).map_err(io_other)?;
        payload_table.remove(id.0.as_str()).map_err(io_other)?;
    }
    write.commit().map_err(io_other)
}

pub fn scan_index(root: &Path) -> io::Result<Vec<SaveMeta>> {
    let db = open_db(root)?;
    let read = db.begin_read().map_err(io_other)?;
    let metas = read.open_table(META_TABLE).map_err(io_other)?;

    let mut out = Vec::new();
    for item in metas.iter().map_err(io_other)? {
        let (_, value) = item.map_err(io_other)?;
        if let Ok(meta) = deserialize_meta(value.value()) {
            out.push(meta);
        }
    }

    out.sort_by(|a, b| b.last_played_at.cmp(&a.last_played_at));
    Ok(out)
}

pub fn load_save_minimal(root: &Path, id: &SaveId) -> io::Result<SaveMeta> {
    let db = open_db(root)?;
    let (meta, payload) = read_save_from_db(&db, id)?;

    if let Some(player_json) = payload.player_json {
        let _: Value = serde_json::from_slice(&player_json)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    }

    Ok(meta)
}

pub fn touch_last_played(root: &Path, id: &SaveId) -> io::Result<SaveMeta> {
    let db = open_db(root)?;
    let (mut meta, payload) = read_save_from_db(&db, id)?;
    meta.last_played_at = OffsetDateTime::now_utc().unix_timestamp();
    write_save_to_db(&db, &meta, &payload)?;
    Ok(meta)
}
