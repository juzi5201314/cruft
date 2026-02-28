#!/usr/bin/env md

# Cruft — Agent Guide

本仓库采用分层 `AGENTS.md`（只在重要/复杂目录放子文件）。根目录只放全局约束；进入子系统目录时，再补充读取该目录下的 `AGENTS.md`。修改代码时请同步修改相应`AGENTS.md`中(如果有)的内容。

## 分层导航

- `crates/app/AGENTS.md`：二进制入口（CLI + 插件装配），以及 legacy plugin 说明
- `crates/game_flow/AGENTS.md`：全局状态机与 `FlowRequest`（唯一 `NextState<_>` 写入点）
- `crates/save/AGENTS.md`：存档索引/操作/加载任务（IO 线程池 + 消息契约）
- `crates/proc_textures/AGENTS.md`：程序化贴图服务（RenderApp compute 一次性生成 + boot readiness）
- `crates/screens/AGENTS.md`：应用层屏幕 UI（BootLoading/MainMenu/SaveSelect/Pause）
- `crates/ui/AGENTS.md`：Geist UI（语义组件 + 皮肤 + observer 交互）
- `docs/voxel/AGENTS.md`：体素引擎规范索引（`docs/voxel/*` 是实现唯一准绳）

## 噪音目录（通常不需要改）

- `.dev/`：开发辅助与 vendored 源码（例如 `.dev/sources/bevy`），多数情况下应忽略
- `skills/`：agent skills 文档，不参与游戏运行时代码

## Git 提交

当用户让你进行提交时，*必须*按照`commit.md`来进行规范的提交。


## 项目概述

Cruft 是一个基于 Bevy 的类 Minecraft 沙盒生存建造游戏。

- **体素世界**：chunk 单位为 `32×32×32` voxel（详见 `docs/voxel/overview.md`）
- **程序化贴图**：所有方块贴图由 GPU compute shader 在启动时生成，零外部图片资产
- **架构原则**：清晰优雅、低耦合、模块化

## 架构规范

### Crate 划分

当前 workspace 以“子系统=独立 crate”为准（与实际代码一致）：

```
crates/
├── app/           # 二进制入口：组装插件、解析 CLI
├── ui/            # Geist UI：主题/组件/交互事件 + 嵌入资源
├── screens/       # 应用层屏幕：BootLoading/MainMenu/SaveSelect/Pause 等
├── game_flow/     # 全局状态机/请求/boot readiness（唯一状态切换写入点）
├── save/          # 存档：索引扫描、管理操作、最小加载流程
├── proc_textures/ # 程序化贴图：GPU compute + texture array
└── gameplay/      # InGame 骨架：WorldRoot、相机/灯光、暂停 gating
```

体素引擎（未来实现）必须以 `docs/voxel/*` 为唯一规范，并按分层拆 crate（建议但不强制的命名示例）：

- `cruft-voxel-storage`：Storage v2（Chunk/Brick/Hot+Cold/Layers）
- `cruft-voxel-meshing`：Binary Greedy Meshing（bitmask → Packed-Quad）
- `cruft-voxel-rendering`：Vertex Pulling + MDI + HZB
- `cruft-voxel-far`：Occupancy Clipmap + Raymarch（远景雾化地平线）
- `cruft-voxel-lighting`：无 GI 的固定光照/合成规则

### 模块规范

- **太长的函数和代码要分文件**
- **不同的内容要分模块**
- 使用 Bevy Plugin 组织功能

### 体素引擎硬约束（以 docs 为准）

下列约束不讨论替代方案；实现必须与文档一致（只列最关键的“写死点”，细节以文档为准）：

- chunk：`CHUNK_SIZE = 32`（三维无限高度流式）
- Storage v2（硬切换，不做向后兼容/双写）：
  - 热路径：`Chunk → Brick(8×8×8)`（每 chunk 64 brick）
  - brick 三态：`Single | Paletted(u16 indices) | Direct(u32 values)`
  - 线性索引顺序写死：`x` 最快、其次 `z`、最后 `y`（见 `docs/voxel/storage.md`）
- Meshing：
  - 输入必须是 `PaddedChunk(34×34×34)` 快照（禁止任务内再读 world storage）
  - 输出为 `PackedQuad(u64)` 流，按 `Opaque/Cutout/Transparent` 分桶（见 `docs/voxel/meshing.md`）
- Rendering：
  - CPU quad buffer 驱动几何；GPU 端 vertex pulling
  - 绘制固定走 MDI；剔除固定走 Frustum + HZB（见 `docs/voxel/rendering.md`）
- Far Volumetric：
  - occupancy clipmap 固定 3 级、固定分辨率与步数预算（见 `docs/voxel/far_volumetric.md`）
- Lighting：
  - 不做 voxel GI，性能优先（见 `docs/voxel/lighting.md`）

### 代码风格

编写 Rust 代码时参考 skill: `coding-guidelines`、`bevy`

- 使用 `cargo clippy` 检查代码
- 使用 `cargo fmt` 格式化代码
- 遵循 Rust 命名规范 (P.NAM, G.FMT)

## 关键文件

| 文件 | 说明 |
|------|------|
| `crates/app/src/main.rs` | 应用入口：插件组装、CLI 参数 |
| `docs/geist_ui_guide.md` | Geist UI 使用指南（与 `cruft-ui` 对齐） |
| `docs/textures.md` | 程序化贴图管线完整规范 |
| `docs/voxel/overview.md` | 体素引擎唯一方案总览（分层与数据流） |
| `docs/voxel/storage.md` | Storage v2：Chunk/Brick/Hot+Cold/Layers |
| `docs/voxel/meshing.md` | Binary Greedy Meshing（bitmask → Packed-Quad） |
| `docs/voxel/rendering.md` | 渲染管线（Vertex Pulling + MDI + HZB） |
| `docs/voxel/far_volumetric.md` | 远景雾化地平线（Occupancy Clipmap + Raymarch） |
| `docs/voxel/persistence.md` | 世界格式 v2：WorldHeader/Region/ChunkRecord |
| `assets/texture_data/*.texture.json` | 贴图配置数据 |
| `assets/shaders/*.wgsl` | WGSL shader 文件 |

## 测试

使用 `cargo nextest run --color=never` 进行测试，而非 `cargo test`。

## 平台

- 如果当前环境在wsl(windows subsystem for linux)中，则使用`--target x86_64-pc-windows-gnu`，不要在wsl中使用linux目标。
