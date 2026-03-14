#!/usr/bin/env md

# crates/game_flow

## Overview

全局流程状态机 crate：`FlowRequest` 是唯一“意图入口”，`apply_flow_requests` 是唯一 `NextState<_>` 写入点。

## Where to look

| 任务 | 位置 | 说明 |
|---|---|---|
| 状态定义 | `crates/game_flow/src/state.rs` | `AppState` + `FrontEndState` + `InGameState` + `PendingGameStart` |
| 请求与 reducer | `crates/game_flow/src/request.rs` | `FlowRequest` + `reduce()`（转移规则表） |
| 唯一状态切换写入 | `crates/game_flow/src/request.rs` | `apply_flow_requests()` 写入 `NextState<_>` |
| Boot readiness/progress | `crates/game_flow/src/boot.rs` | `BootReady` bitflags + `BootProgress` + `update_boot_progress()` |
| 插件装配 | `crates/game_flow/src/lib.rs` | 安装 states/messages/sets |

## Rules (project-specific)

- 其他 crate **只能**发 `FlowRequest`，不要在外部系统里写 `NextState<_>`。
- BootLoading 任务集合是固定 `bitflags`（`BootReady`），不做注册表/动态扩展。

## Testing

- reducer 单测放在 `crates/game_flow/src/request.rs` 的 `#[cfg(test)] mod tests`。

## Common changes

- 新增流程：扩展 `FlowRequest` → 更新 `reduce()` → 补/改 reducer tests → 在 screens/gameplay/save 等请求源处发消息。
- 新建世界参数（如世界生成 preset）必须经 `FlowRequest::StartNewSave` → `PendingGameStart` 透传，避免在 save crate 内硬编码。
