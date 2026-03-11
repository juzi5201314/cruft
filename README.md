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

`dev` / `dev-release` 在 WSL 环境下会使用 `x86_64-pc-windows-gnu` target，便于从 WSL 调试 Windows 渲染路径；其余 `just build` / `just test` / `just check` / `just clippy` 默认使用本机 native target。开发运行仍会设置 `CRUFT_SAVE_DIR=./.dev/run`。

注意：native `build` / `test` / `check` / `clippy` 通过，只能说明本机 target 通过校验；如果改动涉及窗口、输入、渲染、平台行为，仍需要额外用 `just dev` 或 `just dev-release` 验证 Windows 运行路径。

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
- 默认 dev/test profile 只保留 `line-tables-only` 级别的 debuginfo；如果需要深度调试依赖栈，可临时在 `Cargo.toml` 中提高 debug 配置后再复现问题

### 测试

```bash
just test
```

如果在 WSL 下偶发看到
`UtilAcceptVsock: accept4 failed 110` 这类 vsock/interop 超时，
默认并发测试可能会表现为 flaky。此时优先使用仓库内置的
保守 profile 复跑：

```bash
just test --profile wsl-stable
```

这个 profile 只用于 WSL 环境兜底：它会降低 nextest 并发并
启用有限重试，不会改变默认 `just test` 的行为。

## 许可证

MIT / Apache-2.0
