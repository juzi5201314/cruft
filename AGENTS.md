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

### 规范
- 具体项目实现方案请阅读`@docs/`
- 读取`AGENTS.local.md` (如果有)。

### 代码风格

编写 Rust 代码时参考 skill: `coding-guidelines`、`bevy`

- 使用 `just clippy` 检查代码
- 使用 `just fmt` 格式化代码
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

测试只允许使用 `just test`。

## 命令（Just）

- 常用命令统一通过根目录 `justfile` 管理
- 开发运行使用 `just dev`
- `just` 的 cargo 命令通过 `bun` 运行 `scripts/cargo.cjs`

## 平台

- WSL 环境下仅 `just dev` / `just dev-release` 会通过 `scripts/cargo.cjs` 使用 `x86_64-pc-windows-gnu`；其余命令默认使用本机 native target。
- native `just build` / `just test` / `just check` / `just clippy` 的结果不等于 Windows 运行路径已验证；涉及窗口/输入/渲染/平台行为时，仍需额外验证 `just dev` 或 `just dev-release`。

## 技能
- 在编写涉及到bevy的代码时，必须查看 $bevy skill
- 在编写涉及到webgpu(wgsl)代码时，必须查看 $wgsl skill
