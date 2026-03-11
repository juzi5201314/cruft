#!/usr/bin/env md

# crates/proc_textures

## Overview

程序化贴图子系统现在是 **严格 parser + canonical compiler + CPU 参考生成器 + 运行时上传 + 导出器**。

- 规范唯一来源：`docs/textures.md`
- 运行时入口：`crates/proc_textures/src/plugin.rs`
- 数据源：`assets/texture_data/blocks.texture.json`
- 对外主契约：`TextureRegistry`、`TextureRuntimePacks`、`ProcTexturesStatus`

## Where to look

| 任务 | 位置 | 说明 |
|---|---|---|
| schema / 严格 JSON | `crates/proc_textures/src/schema.rs` | 重复键检测、原始 schema 类型 |
| 编译 / canonical / fingerprint | `crates/proc_textures/src/compiler.rs` | defaults、face 解析、signal DAG、canonical form |
| CPU 参考生成 / 运行时 pack | `crates/proc_textures/src/generator.rs` | surface 求值、mip、GPU 上传字节布局 |
| 导出器 | `crates/proc_textures/src/export.rs` | PNG 导出与 manifest |
| 启动状态机 | `crates/proc_textures/src/plugin.rs` | `Loading/Ready/Failed` 与 `BootReady::PROC_TEXTURES` |

## Rules

- 不要恢复旧数组 schema、旧 `style` 枚举、旧 `name -> layer_index` 契约。
- 不要恢复旧 WGSL compute 生成主路径；当前唯一语义实现是 CPU 参考生成器。
- 失败必须进入 `ProcTexturesStatus::Failed(String)`，不能卡在 `Loading`。
- registry 只有整套编译/生成/上传成功后才能插入 world。
- 运行时物理打包固定为五套 array：`albedo`、`normal`、`orm`、`emissive`、`height`。

## Testing

- 优先补 parser/compiler/generator/exporter 单测。
- golden / canonical / registry / failure-path 相关测试都放在 crate 内。
