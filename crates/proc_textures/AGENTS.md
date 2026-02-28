#!/usr/bin/env md

# crates/proc_textures

## Overview

程序化贴图“纯服务”crate：启动时在 RenderApp 内 dispatch WGSL compute，一次性生成 `texture_2d_array`，并通过 `BootReadiness` 汇报 ready。

## Where to look

| 任务 | 位置 | 说明 |
|---|---|---|
| 主实现 | `crates/proc_textures/src/plugin.rs` | asset loader + 目标纹理创建 + RenderApp pipeline + RenderGraph node |
| 计算 shader | `assets/shaders/procedural_texture.wgsl` | storage write 到 texture array |
| 材质 shader | `assets/shaders/procedural_array_material.wgsl` | array 采样（fragment） |
| 数据源 | `assets/texture_data/blocks.texture.json` | 纹理 layer specs |
| 运行时装配 | `crates/app/src/main.rs` | `ProcTexturesPlugin` 在这里启用 |

## Rules (project-specific)

- 纯服务：不要在此 crate 默认 spawn 预览实体（相机/灯光/方块）。
- 只生成一次：RenderGraph node 状态机 `Loading -> Dispatching -> Done`，Done 后不再 dispatch。
- storage texture 不要用 sRGB 格式：当前主路径用 `TextureFormat::Rgba8Unorm`。
- 主路径避免 GPU→CPU readback（只允许测试/导出场景）。

## Boot readiness contract

- 主世界 `poll_ready_signal()` drain channel 后写 `BootReady::PROC_TEXTURES`（BootLoading 聚合用）。

## Parameters / limits

- `CANONICAL_TEXTURE_SIZE = 64`，`MAX_LAYERS = 256`。
- JSON 的 `size` 通过 “语义缩放” 归一到 64×64（`TextureSpec::to_layer_params()` 调整 `noise_scale/warp_strength`）。

## Gotchas

- `crates/app/src/plugins/procedural_texture.rs` 是旧/样例实现（与此 crate 高度重复）；修改生产逻辑优先改这里。
