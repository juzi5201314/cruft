#!/usr/bin/env md

# crates/app

## Overview

二进制入口 crate：只负责 CLI 参数/环境变量解析与插件装配，不承载子系统实现。

## Where to look

| 任务 | 位置 | 说明 |
|---|---|---|
| 应用入口/插件装配 | `crates/app/src/main.rs` | 唯一 `fn main()`；组装 `GameFlow/Save/Voxel/ProcTextures/Screens/Gameplay` |
| 环境变量 | `crates/app/src/main.rs` | `CRUFT_SAVE_DIR` → `SaveRootDir` |
| legacy plugins | `crates/app/src/plugins/` | 旧/样例实现；当前不在 `main.rs` 插件链中 |

## Conventions

- 保持 thin：这里只做 “装配 + 参数注入 + DefaultPlugins 配置”。
- 新子系统优先新增 workspace crate（`crates/<subsystem>`）并在 `main.rs` 装配。

## Gotchas

- legacy procedural texture 示例已移除；运行时主路径仅保留 `crates/proc_textures/`（见 `crates/app/src/main.rs`）。

## Commands

```bash
just dev
```
