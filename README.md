# Cruft

基于 [Bevy](https://bevyengine.org/) 开发的类 Minecraft 体素沙盒生存游戏。

## 概述

Cruft 是一个体素沙盒游戏，具有以下特性：

- **程序化贴图**：使用 GPU compute shader 生成纹理，无需外部图片资源
- **体素世界**：基于区块的世界，每个区块 32×32 体素
- **模块化架构**：通过 workspace crate 清晰分离关注点

## 项目结构

```
cruft/
├── crates/
│   └── app/                    # 主应用 crate
│       └── src/
│           ├── main.rs         # 入口点
│           └── plugins/        # Bevy 插件
├── assets/
│   ├── shaders/                # WGSL shader
│   └── texture_data/           # 程序化贴图配置 (JSON)
├── docs/
│   ├── textures.md             # 程序化贴图系统文档
│   └── voxel/                  # 体素引擎方案文档
└── Cargo.toml                  # Workspace 配置
```

## 快速开始

### 环境要求

- Rust 1.85+ (edition 2021)
- 支持 Vulkan 的 GPU
- `just`（命令管理器）
- `bun`（用于运行 `scripts/cargo.cjs`）

### 运行

```bash
just dev
```

`dev` 在 WSL 环境下会使用 `x86_64-pc-windows-gnu` target，非 WSL 环境使用本机默认 target；并设置 `CRUFT_SAVE_DIR=./.dev/run`。

启动后将显示一个测试场景，展示程序化生成的方块贴图。

## 架构原则

本项目遵循以下架构准则：

1. **Workspace Crate**：独立子系统应提取为 `crates/` 下的独立 crate
2. **插件化设计**：主要功能通过 Bevy 插件实现
3. **数据驱动**：内容（贴图、方块定义）在数据文件中定义，而非代码
4. **GPU 优先**：程序化贴图通过 GPU compute shader 生成；体素网格化主线走 CPU 异步（详见 `docs/voxel/overview.md`）

## 程序化贴图

贴图系统在启动时使用 WGSL compute shader 生成所有方块贴图。贴图通过 JSON 配置文件定义：

完整规范见 [docs/textures.md](docs/textures.md)。

## 体素引擎文档

体素（voxel）引擎方案与取舍记录见：

- [docs/voxel/overview.md](docs/voxel/overview.md)

## 开发

### 代码风格

- 遵循 Rust 命名规范 (P.NAM, G.FMT)
- 使用 `just clippy` 检查代码
- 使用 `just fmt` 格式化代码

### 测试

```bash
just test
```

## 许可证

MIT / Apache-2.0
