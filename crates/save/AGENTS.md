#!/usr/bin/env md

# crates/save

## Overview

存档子系统：索引扫描、管理操作（new/copy/rename/delete/rescan）与最小加载任务；通过 message 与 `FlowRequest` 与前台/状态机对接。

## Where to look

| 任务 | 位置 | 说明 |
|---|---|---|
| 插件/消息类型/系统集 | `crates/save/src/api.rs` | `SavePlugin`、`SaveOp*`、`SaveLoad*`、`SaveSet` |
| IO 实现/磁盘格式 | `crates/save/src/io.rs` | 实际 scan/create/copy/rename/delete/load；world header v2 |
| 类型 | `crates/save/src/types.rs` | `SaveRootDir`、`SaveId`、`SaveMeta`、`LoadedSave(+WorldHeaderV2)` |

## Conventions

- IO 一律跑在 `IoTaskPool`：spawn `Task` → 作为 `Resource` 持有（`ScanTask/OpTask/LoadTask`）→ Update 阶段用 `poll_once` 轮询。
- Ops 处理是“本帧最后一个请求生效”：`start_save_ops()` 会 drain reader 只取最后一个 request。

## Flow integration

- BootLoading：`OnEnter(AppState::BootLoading)` 触发 `start_scan_index`，完成后写入 `BootReady::SAVE_INDEX`。
- InGame Loading：由 `PendingGameStart` 驱动加载/新建，并在失败时回退到菜单（写 `FlowRequest`）。
- 新建存档所需的世界生成参数（例如 `WorldGenPreset`）来自 `PendingGameStart`，不要在 `io::create_new_save` 调用点以固定 preset 写死。

## Testing

- 单测内联在 `crates/save/src/api.rs`；用到临时目录（见 `crates/save/Cargo.toml` 的 `tempfile`）。

## Gotchas

- 世界格式以 v2 为准（`header.cruft` + `meta.json`）；不要再引入并行旧格式。
