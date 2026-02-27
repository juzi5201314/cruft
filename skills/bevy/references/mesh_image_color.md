# Mesh / Image / Color（Bevy v0.18）

这篇覆盖“渲染数据的基础材料”三块：

- `Mesh`：几何、顶点属性、skinning/morph、生成切线、Topology 等
- `Image`：纹理资产、图集、采样/寻址、加载设置与常见格式
- `Color`：sRGB vs Linear、色彩空间转换与常用 API

> 2D/3D 渲染与材质系统的用户侧入口见 `rendering_2d.md`、`rendering_3d_pbr.md`、`shaders_materials.md`。

## 0) 入口与 examples

源码入口：

- `bevy_mesh` crate：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_mesh/src/lib.rs`
  - 更深入请直接在目录内找 `mesh.rs`/`skinning`/`morph` 等：`https://github.com/bevyengine/bevy/tree/v0.18.0/crates/bevy_mesh/src`
- `bevy_image` crate：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_image/src/lib.rs`
  - 图集/加载器/格式支持在目录内：`https://github.com/bevyengine/bevy/tree/v0.18.0/crates/bevy_image/src`
- `bevy_color` crate（强烈建议读顶部文档：颜色空间解释 + 转换图）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_color/src/lib.rs`

关键 examples：

- 生成自定义 Mesh（UV/indices/attributes，工程化闭环）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/generate_custom_mesh.rs`
- 顶点色（`Mesh::ATTRIBUTE_COLOR`）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/vertex_colors.rs`
- 纹理配置（StandardMaterial 的常见贴图用法）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/texture.rs`
- Array texture / texture array（`texture_2d_array<f32>`）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/shader/array_texture.rs`
- Texture atlas（2D 图集）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/2d/texture_atlas.rs`
- UI 图集：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ui/ui_texture_atlas.rs`
- 2D repeated texture（贴图重复/UV）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/2d/mesh2d_repeated_texture.rs`

## 1) Mesh：Bevy 的几何资产模型（你需要掌握的“用户级 API”）

### 1.1 `Mesh` 是资产，实体上挂的是 `Mesh3d`/`Mesh2d`

- `Mesh` 数据在 `Assets<Mesh>` 资源里
- 实体上是 `Mesh3d(handle)` / `Mesh2d(handle)`（分别走 3D/2D 管线）

对应插件在 DefaultPlugins 中由 `MeshPlugin` 安装（见 `default_plugins.rs`）：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_internal/src/default_plugins.rs`

### 1.2 手搓 Mesh 的标准路线：attributes + indices + topology

最推荐抄的模板就是 `generate_custom_mesh.rs`：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/generate_custom_mesh.rs`

你会在里面看到：

- `Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::...)`
- `with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, ...)`
- `with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, ...)`
- `with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, ...)`
- `insert_indices(Indices::U32(...))`
- 运行时通过 `meshes.get_mut(handle)` 修改 attribute（动态 UV/顶点色等）

这是“把 Mesh 当数据结构”正确且可维护的方式。

### 1.3 顶点色：注意 `base_color` 的乘法

`vertex_colors.rs` 展示了一个重要细节：

- 顶点色会与材质 `base_color` 相乘
- 因此当你用顶点色时，通常把 `base_color` 设为白色（否则会整体变暗/偏色）

示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/vertex_colors.rs`

### 1.4 切线/法线与 PBR：别忘了 tangents

很多 PBR 特性（法线贴图等）依赖切线空间。你会在多个示例里看到：

- `mesh().build().with_generated_tangents().unwrap()`

当你“手搓 mesh”或“运行时改顶点”时，要把 tangents 的正确性当成一等公民：否则材质会看起来“怪”但你很难从 shader 层定位。

（真实工程里建议直接参考内建 primitives 的实现与 `with_generated_tangents` 的用法）

## 2) Image：纹理资产、图集与采样

### 2.1 `Image` 也是资产：加载与引用路径

最常见的路径：

- `AssetServer::load("path.png")` 得到 `Handle<Image>`
- 把 handle 塞进 `StandardMaterial.base_color_texture` 或 UI 节点

纹理在 3D 材质里的最小示例：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/texture.rs`

### 2.2 Texture atlas：2D 与 UI 都会用到

2D 图集示例：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/2d/texture_atlas.rs`

UI 图集示例：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ui/ui_texture_atlas.rs`

### 2.3 采样与寻址：重复贴图/边缘处理

当你需要：

- UV 超出 0..1 的重复/镜像/夹取
- 最近邻/线性过滤
- mipmap 相关质量控制

优先从示例入手，例如 repeated texture：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/2d/mesh2d_repeated_texture.rs`

以及 Bevy 的 `Image`/loader settings API（示例可见 `solari.rs` 中的 `ImageLoaderSettings` 用法）：

- `ImageLoaderSettings` 在示例里出现：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/solari.rs`

### 2.4 Array texture / Texture2DArray：`ImageArrayLayout` + `load_with_settings`

当你想把一张图当成“多层纹理数组”（给 WGSL 的 `texture_2d_array<f32>` 用），Bevy 推荐用 loader settings 在加载期完成切分/布局解释：

- `ImageLoaderSettings.array_layout: Option<ImageArrayLayout>`
- `ImageArrayLayout::RowCount { rows: N }`（把图按垂直方向切成 N 层）

权威定义在 image loader：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_image/src/image_loader.rs`

最完整模板（包含：加载设置 + 材质绑定 `dimension = "2d_array"` + runtime 切 layer）：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/shader/array_texture.rs`
- WGSL：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/assets/shaders/array_texture.wgsl`

## 3) Color：sRGB vs Linear（不搞清楚会导致“材质看起来不对”）

Bevy 的 `Color` 相关 API 很多看起来“只是构造颜色”，但背后涉及颜色空间：

- 贴图/美术资产通常是 sRGB（非线性）
- 光照计算需要 linear RGBA（线性）

`bevy_color` 的 crate 文档把这个讲得非常清楚，并且包含转换图：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_color/src/lib.rs`

工程建议：

- 写代码时尽量用 `Color::srgb(...)` / `Color::srgba(...)` 这种“显式标记 sRGB”的构造（而不是隐式把线性值当 sRGB）
- 写自定义 shader 时，明确你的输入（纹理采样/常量）处于哪个空间：必要时做线性化转换

## 4) 与精炼文档的对照（Context7）

Context7 `/websites/rs_bevy` 适合快速查：

- `Mesh` / `Mesh::ATTRIBUTE_*` / `PrimitiveTopology`
- `Image` / `TextureAtlas` / loader settings
- `Color::srgb` / `Color::srgba` / `LinearRgba`

但“实际 pipeline 需要哪些顶点属性、哪些属性会被哪些材质/预处理使用”，更建议以 examples 与相关 crate 源码为准。
