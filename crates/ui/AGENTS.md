#!/usr/bin/env md

# crates/ui

## Overview

Geist UI：语义组件（Button/Card/Progress/TextInput/Modal）+ 皮肤系统 + Observer 交互事件；通过 `bevy_embedded_assets` 把 `assets/` 与本 crate 的字体资源嵌入二进制。

## Where to look

| 任务 | 位置 | 说明 |
|---|---|---|
| UI 插件/皮肤系统 | `crates/ui/src/geist_ui.rs` | `CruftUiPlugin` + `GeistGridMaterial` + skin systems |
| 组件定义 | `crates/ui/src/components.rs` | `UiButton/UiCard/UiProgress/UiTextInput/...` |
| 事件 | `crates/ui/src/events.rs` | `UiClick/UiSubmit/UiCancel` |
| Builder API | `crates/ui/src/ui.rs` | `UiBuilder` + `UiEntityCommandsExt`（组合式 UI） |
| Theme/字体 | `crates/ui/src/theme.rs` | `UiTheme` + `UiFontResources` |
| 对外导出 | `crates/ui/src/lib.rs` | `prelude` + `CruftUiAssetsPlugin` |

## Conventions

- UI 构建优先走 `UiBuilder`，避免到处散落“手写 Node 样式常量”。
- 交互事件：`Interaction::Pressed` → `UiClick` trigger；文本输入 focus/键盘处理都在本 crate 内部完成。

## Assets

- 字体：`crates/ui/assets/fonts/*`
- 网格背景 shader：`assets/shaders/geist_grid.wgsl`（以 `shaders/geist_grid.wgsl` 路径加载）

## Gotchas

- `CruftUiAssetsPlugin` 使用 ReplaceDefault 模式嵌入资产：运行时仍用普通 `AssetServer` 路径（不需要 `embedded://` 前缀）。
