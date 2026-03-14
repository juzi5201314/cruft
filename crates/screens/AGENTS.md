#!/usr/bin/env md

# crates/screens

## Overview

应用层屏幕 UI：BootLoading/MainMenu/SaveSelect/Pause/InGameLoading。只发消息（`FlowRequest` / `Save*Request`），不直接切 `NextState<_>`。

## Where to look

| 任务 | 位置 | 说明 |
|---|---|---|
| 插件聚合 | `crates/screens/src/lib.rs` | `ScreensPlugin` 安装 `CruftUiPlugin` + 各 ScreenPlugin |
| 通用背景 | `crates/screens/src/common.rs` | `spawn_grid_background()` |
| BootLoading | `crates/screens/src/boot_loading.rs` | 监听 `BootProgress` + `ProcTexturesStatus`，更新进度条/文案并显示纹理失败信息 |
| MainMenu | `crates/screens/src/main_menu.rs` | Start 点击发 `FlowRequest::EnterSaveSelect` |
| SaveSelect | `crates/screens/src/save_select.rs` | 存档列表 + 模态 + 操作请求（热点文件） |
| Pause | `crates/screens/src/pause_menu.rs` | 发 `Resume` / `QuitToMainMenu` |
| InGameLoading | `crates/screens/src/in_game_loading.rs` | 加载 overlay + 失败回退 |

## Conventions

- 生命周期：每个 screen root 都用 `DespawnOnExit(State)`（scoped entities）。
- UI 组合：用 `cruft_ui::ui::UiBuilder` + 语义组件；事件处理用 observer（例如 `.click(on_*)`）。

## Anti-patterns

- 不要写 `NextState<_>`；只通过 `FlowRequest` 触发状态机。
- 不要把 domain 逻辑塞进 UI：存档 IO/操作逻辑在 `crates/save`，流程转移在 `crates/game_flow`。

## Gotchas

- `crates/screens/src/save_select.rs` 变更面很宽（列表/模态/事件/消息全在一处）；一次改动跨多个职责时优先先拆模块再改行为。
