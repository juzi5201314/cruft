# Dynamic Linking（bevy/dynamic_linking, Bevy v0.18）

这篇聚焦 Bevy 的“动态链接模式”：让 Bevy 作为动态库链接，从而显著加快开发期的增量编译。

结论先说清：

- **开发期可以用**（更快的增量编译）
- **发布版不要用**（需要随游戏一起分发额外的动态库）

## 0) 权威入口

- `bevy_dylib` crate 文档（写得非常直接）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_dylib/src/lib.rs`
- bevy 顶层 feature 列表里包含 `dynamic_linking`（可用于理解 feature 名称来源）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/Cargo.toml`

## 1) 推荐方式：命令行临时开启（不要写死在 Cargo.toml）

按 `bevy_dylib` 文档，推荐用 feature flag 临时开启：

- `cargo run --features bevy/dynamic_linking`

这样你不会忘记在 release 时移除它。

## 2) 不推荐方式：写进 `Cargo.toml`

把 `dynamic_linking` 写进 `Cargo.toml` 的坏处是：

- 你每次打 release 都要记得移除，否则会要求你分发 `libstd.so` / `libbevy_dylib.so` 等额外文件

## 3) 手动方式：显式依赖 `bevy_dylib`（debug-only）

如果你确实要手动启用，文档给的写法是：

```rust
#[allow(unused_imports)]
#[cfg(debug_assertions)]
use bevy_dylib;
```

权威说明见：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_dylib/src/lib.rs`

## 4) 与精炼文档的对照（Context7）

Context7 `/websites/rs_bevy` 一般不如源码文档直观；动态链接以 `bevy_dylib` crate 文档为准。

