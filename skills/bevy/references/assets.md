# Assets（Bevy v0.18）：AssetServer / Handle / 热重载 / 自定义资产

Bevy 的资产系统解决两个核心问题：

- 大资源去重与共享（通过 `Handle` 引用）
- 异步加载（不阻塞主线程）

建议先读 crate 顶部文档（写得很完整）：

- Source：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_asset/src/lib.rs`
- `AssetServer` 实现：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_asset/src/server/mod.rs`

## 1) 基础：`AssetServer::load` 返回 `Handle<T>`

典型用法：

```rust
fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let icon: Handle<Image> = asset_server.load("branding/icon.png");
    commands.spawn(Sprite::from_image(icon));
}
```

示例：

- 最简单 sprite：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/2d/sprite.rs`

要点：

- `load` 立即返回 `Handle<T>`，并不会阻塞等待文件读完。
- asset 真正的数据存放在资源 `Assets<T>` 中（例如 `Assets<Image>`）。
- 只要同一路径仍有 handle 活着，重复 `load` 会返回同一个 handle（避免重复加载）。

## 2) 等待资产加载完成：`load_state` / `is_loaded_with_dependencies`

常见策略：

- 在 Update 里轮询 `asset_server.load_state(&handle)` 或更高级的 “with dependencies” 判断
- 配合 States 做 loading → ingame 的切换

3D 环境贴图示例里有 `load_state(...).is_loaded()` 的用法：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/pbr.rs`（`environment_map_load_finish`）

## 3) 热重载（hot reload）

开发期非常有用：改文件 → 运行中的程序自动更新。

启用方式：

- 桌面平台：Cargo feature `file_watcher`
  - 见：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/docs/cargo_features.md`（`file_watcher`）
- 运行期可用 `AssetPlugin.watch_for_changes_override` 控制开关（见下节）

示例：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/asset/hot_asset_reloading.rs`

## 4) `AssetPlugin`：模式、路径、processor、meta、安全

`AssetPlugin` 提供大量可配项（节选）：

- file_path / processed_file_path：未处理与处理后资产根目录
- watch_for_changes_override：热重载开关
- use_asset_processor_override / mode：是否使用 asset processor、Processed/Unprocessed 模式
- meta_check：meta 文件检查策略
- unapproved_path_mode：访问“非允许目录”时的行为

- Source：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_asset/src/lib.rs`（`pub struct AssetPlugin { ... }`）

建议：

- 普通项目先用默认；当你确实需要“处理后资产管线”（压缩/转换/构建产物）再研究 processor。
- 如果你启用 http/https 资产源（WebAssetPlugin），注意安全影响（源码注释里明确 warning）。

## 5) 自定义资产类型：`Asset` + `AssetLoader`

讲解型示例（包含两种 loader，RON 与 blob）：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/asset/custom_asset.rs`

要点：

- 资产类型一般 `#[derive(Asset, TypePath, ...)]`
- loader 实现 `AssetLoader`：
  - `type Asset = MyAsset`
  - `type Settings = ...`（可配置）
  - `async fn load(reader, settings, load_context) -> Result<Asset, Error>`
  - `fn extensions() -> &[&str]` 指定扩展名
- 注册：
  - `app.init_asset::<MyAsset>()`
  - `app.init_asset_loader::<MyAssetLoader>()`

## 6) 资产来源（Asset sources）：本地/嵌入/额外目录/HTTP

常见需求：

- “把资源打进二进制”：embedded assets
- “额外资源目录”：extra asset source
- “通过 HTTP/HTTPS 下载”：web asset source

对应 examples：

- `examples/asset/embedded_asset.rs`
- `examples/asset/extra_source.rs`
- `examples/asset/asset_loading.rs`（综合加载方式）

## 7) 与 Scenes/glTF 的关系

- glTF 的 `#Scene0`、命名子资源等，本质上是“带 label 的 asset path”
- Scenes（`Scene` / `DynamicScene`）本身也是资产（在 `serialize` feature 下）

下一步：

- `scenes_gltf.md`
