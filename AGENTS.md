## Git 提交

当用户让你进行提交时，*必须*按照`prompt/commit.md`来进行规范的提交。


## 项目概述

Cruft 是一个基于 Bevy 的类 Minecraft 沙盒生存建造游戏。

- **体素世界**：区块单位为 32×32 体素
- **程序化贴图**：所有方块贴图由 GPU compute shader 在启动时生成，零外部图片资产
- **架构原则**：清晰优雅、低耦合、模块化

## 架构规范

### Crate 划分

不同的内容应分模块，完全不同的内容应分 sub crate：

```
crates/
├── app/          # 主应用入口、插件组装
├── voxel/        # 体素世界逻辑（如后续需要）
├── terrain/      # 地形生成（如后续需要）
└── ...           # 按需扩展
```

### 模块规范

- **太长的函数和代码要分文件**
- **不同的内容要分模块**
- 使用 Bevy Plugin 组织功能

### 代码风格

编写 Rust 代码时参考 skill: `coding-guidelines`、`bevy`

- 使用 `cargo clippy` 检查代码
- 使用 `cargo fmt` 格式化代码
- 遵循 Rust 命名规范 (P.NAM, G.FMT)

## 关键文件

| 文件 | 说明 |
|------|------|
| `docs/textures.md` | 程序化贴图系统完整规范 |
| `assets/texture_data/*.texture.json` | 贴图配置数据 |
| `assets/shaders/*.wgsl` | WGSL shader 文件 |

## 测试

使用 `cargo nextest run --color=never` 进行测试，而非 `cargo test`。

## 平台

- 如果当前环境在wsl(windows subsystem for linux)中，则使用`--target x86_64-pc-windows-gnu`，不要在wsl中使用linux目标。
