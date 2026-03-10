#!/usr/bin/env md

# crates/save

## Overview

存档子系统：索引扫描、管理操作（new/copy/rename/delete/rescan）与最小加载任务；通过 message 与 `FlowRequest` 与前台/状态机对接。

## Where to look

| 任务 | 位置 | 说明 |
|---|---|---|
| 插件/消息类型/系统集 | `crates/save/src/api.rs` | `SavePlugin`、`SaveOp*`、`SaveLoad*`、`SaveSet` |
| IO 实现/磁盘格式 | `crates/save/src/io.rs` | 实际 scan/create/copy/rename/delete/load；当前格式仍是 v1 |
| 类型 | `crates/save/src/types.rs` | `SaveRootDir`、`SaveId`、`SaveMeta`、`LoadedSave` |

## Conventions

- IO 一律跑在 `IoTaskPool`：spawn `Task` → 作为 `Resource` 持有（`ScanTask/OpTask/LoadTask`）→ Update 阶段用 `poll_once` 轮询。
- Ops 处理是“本帧最后一个请求生效”：`start_save_ops()` 会 drain reader 只取最后一个 request。

## Flow integration

- BootLoading：`OnEnter(AppState::BootLoading)` 触发 `start_scan_index`，完成后写入 `BootReady::SAVE_INDEX`。
- InGame Loading：`OnEnter(InGameState::Loading)` 读取 `PendingGameStart` 发起请求（并移除该资源），`SaveLoadResult` 在 Loading 内统一驱动 `FlowRequest::FinishGameLoading`/`QuitToMainMenu`。

## Testing

- 单测内联在 `crates/save/src/api.rs`；用到临时目录（见 `crates/save/Cargo.toml` 的 `tempfile`）。

## Gotchas

- `docs/voxel/persistence.md` 规定世界格式 v2；当前 `crates/save/src/io.rs` 仍是 v1（不要在这里引入“第三套格式”，迁移应以 voxel spec 为准）。
