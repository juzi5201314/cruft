#!/usr/bin/env md

# docs/voxel

## Overview

体素引擎规范（normative spec）。`docs/voxel/*` 是实现**唯一准绳**：不要在代码里发明替代路线/二号方案。

## Where to look

| 主题 | 规范 | 说明 |
|---|---|---|
| 总览/分层/数据流 | `docs/voxel/overview.md` | 唯一路线、模块边界、固定参数 |
| WorldGen 地表 | `docs/voxel/worldgen_surface.md` | 可插拔生成器接口 + Modern Surface 参数与规则 |
| Storage v2 | `docs/voxel/storage.md` | `Chunk(32^3) -> Brick(8^3)`、热/冷分离、索引顺序 |
| Blocks 热路径 | `docs/voxel/storage_blocks.md` | brick 三态、palette 规则、`PaddedChunk` 契约 |
| 分层数据 | `docs/voxel/storage_layers.md` | BlockEntity/Entities/WorldState 与 blocks 解耦 |
| Meshing | `docs/voxel/meshing.md` | 输入快照、Binary Greedy、`PackedQuad(u64)` 输出 |
| Rendering | `docs/voxel/rendering.md` | vertex pulling + MDI + Frustum/HZB；禁止 `Mesh` 顶点资产路径 |
| Far Volumetric | `docs/voxel/far_volumetric.md` | 3 级 clipmap + 固定预算；禁止 GPU 反读 |
| Lighting | `docs/voxel/lighting.md` | 禁止 voxel GI |
| Persistence v2 | `docs/voxel/persistence.md` | 世界格式 v2（硬切换、不做向后兼容） |

## Hard invariants (copy/paste safe)

- `CHUNK_SIZE = 32`
- 线性索引顺序写死：`x` 最快、其次 `z`、最后 `y`（storage + meshing 必须一致）
- Meshing 输入必须是 `PaddedChunk(34×34×34)` 快照；任务内禁止回读 world storage
- Meshing 输出是 `PackedQuad(u64)` 流，按 `Opaque/Cutout/Transparent` 分桶
- Rendering 固定走：CPU quad buffer + GPU vertex pulling + MDI + HZB；体素几何禁止走 Bevy `Mesh`
- Persistence：世界格式 v2 only；不做向后兼容/双写

## Implementation status

- `crates/voxel` 已有部分 voxel rendering 实现：自定义 RenderApp / ViewNode / compute culling / indirect draw。
- 当前实现正按 wave-1 规范（temporal HZB, per-view ownership, Opaque+Cutout only）继续对齐；storage/meshing 仍在实现阶段。
- 实现阶段应以此规范目录为准对齐，避免“先写代码再补文档”的漂移。
