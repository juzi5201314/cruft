# Camera / Views / Render Targets（Bevy v0.18）

这篇聚焦“相机与视图系统”本身：`Camera` 组件的关键字段、投影、viewport、多相机叠加、RenderLayers、渲染到纹理（Render to Texture）、以及把屏幕像素/光标位置转换为世界射线（viewport_to_world）。

> 注意：渲染子世界/RenderGraph 的整体架构见 `rendering_architecture.md`；材质与 shader 见 `shaders_materials.md`。

## 0) 入口与 examples（先从这里下手）

源码入口：

- `bevy_camera` crate：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_camera/src/lib.rs`
- `Camera` / `RenderTarget` / `Viewport` / `SubCameraView`：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_camera/src/camera.rs`
- 投影（`Projection`/`PerspectiveProjection`/`OrthographicProjection`）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_camera/src/projection.rs`
- 清屏与 MSAA writeback（`ClearColorConfig` / `MsaaWriteback`）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_camera/src/clear_color.rs`
- 可见性/渲染层/距离裁剪：`https://github.com/bevyengine/bevy/tree/v0.18.0/crates/bevy_camera/src/visibility`

强相关 examples（“能抄骨架”）：

- 相机目录总览：`https://github.com/bevyengine/bevy/tree/v0.18.0/examples/camera`
  - 轨道相机：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/camera/camera_orbit.rs`
  - 自定义投影：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/camera/custom_projection.rs`
  - 投影缩放：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/camera/projection_zoom.rs`
- Split screen（多相机 + viewport）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/split_screen.rs`
- Sub-camera view（多显示器/大画面裁切）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/camera_sub_view.rs`
- Render to texture（渲染到 `Image`）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/render_to_texture.rs`
- viewport_to_world（像素→射线）：
  - 2D：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/2d/2d_viewport_to_world.rs`
  - 3D：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/3d_viewport_to_world.rs`
- 距离裁剪（`VisibilityRange`）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/visibility_range.rs`
- UI 绑定到指定相机（`UiTargetCamera`）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ui/ui_target_camera.rs`
- UI 渲染到纹理：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ui/render_ui_to_texture.rs`

可选：第一方相机控制器（用于快速原型/示例与工具）：

- `bevy_camera_controller` crate：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_camera_controller/src/lib.rs`
  - free camera / pan camera 等在该 crate 子模块（按 feature gate）：`https://github.com/bevyengine/bevy/tree/v0.18.0/crates/bevy_camera_controller/src`
  - 对应示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/camera/free_camera_controller.rs`、`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/camera/pan_camera_controller.rs`

## 1) `Camera`：必须记住的字段与“叠加规则”

`Camera` 是渲染视图的核心组件（不是 `Camera2d/Camera3d`），它决定：

- 渲染到哪里：`RenderTarget`（默认是主窗口）
- 渲染哪一块：`viewport: Option<Viewport>`（物理像素区域）
- 渲染顺序：`order: isize`（越大越后画、越在上层）
- 是否渲染：`is_active`
- 是否清屏：`clear_color: ClearColorConfig`
  - `Default`（用世界资源 `ClearColor`）
  - `Custom(Color)`
  - `None`（**不清屏**：典型用于多相机叠加同一 target）
- 特殊渲染：`invert_culling`（镜像等需要翻转面剔除）
- “子视图裁切”：`sub_camera_view: Option<SubCameraView>`（多显示器/大画面切片）

权威定义与注释见：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_camera/src/camera.rs`

## 2) Viewport：同一窗口的多相机布局（Split screen / 小地图 / 角色预览）

`Viewport` 使用**物理像素**描述 `Camera` 在 `RenderTarget` 上渲染的矩形：

- `physical_position: UVec2`（左上角）
- `physical_size: UVec2`
- `depth: Range<f32>`（0..1 深度范围）

常见套路：

- 2 个 `Camera3d` 共用同一个 window target
- 分别给它们设置不同 viewport（左右分屏/上下分屏）
- 用 `Camera.order` + `ClearColorConfig::None` 决定覆盖层（例如 UI overlay 相机）

最小模板直接看：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/split_screen.rs`

## 3) `SubCameraView`：把“一个大画面”切成多块投影到多个相机

`SubCameraView` 解决的不是“画在哪”，而是“投影哪一部分”：当你希望多个相机共同构成一个**逻辑一致**的大画面时（多显示器拼接、超宽视角切片），用它来指定：

- `full_size`：大画面的总尺寸
- `offset`/`size`：当前相机对应的子块

示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/camera_sub_view.rs`

## 4) Render to Texture：把相机输出写进 `Image`

核心点：

- `RenderTarget` 可以指向 `Image`（而不是 Window）
- 你需要创建/配置一张 `Image` 作为渲染目标（尺寸、格式、`TextureUsages`）
- 再把该 `Image` 作为材质贴图/UI 纹理来显示

示例：

- 3D render to texture：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/render_to_texture.rs`
- UI render to texture：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ui/render_ui_to_texture.rs`

与渲染架构的连接点（RenderGraph/主世界与渲染世界同步）见：`rendering_architecture.md`

## 5) 投影与自定义 projection：Perspective / Ortho / Custom

Bevy 用 `Projection`（enum）统一表达投影，并提供常见实现：

- `PerspectiveProjection`：典型 3D
- `OrthographicProjection`：典型 2D / 3D 正交

源码入口：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_camera/src/projection.rs`

示例（强烈建议直接抄）：

- 自定义投影：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/camera/custom_projection.rs`
- 投影缩放：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/camera/projection_zoom.rs`

## 6) 从像素到世界：`viewport_to_world`（鼠标射线/点击命中）

当你要做：

- 鼠标点选（mesh/sprite/UI picking）
- 视口坐标→世界坐标（射线投射/地面点）

优先使用 `Camera` 自带的转换方法（内部用相机 computed 矩阵与 viewport 信息）。

示例：

- 2D：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/2d/2d_viewport_to_world.rs`
- 3D：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/3d_viewport_to_world.rs`

如果你的目标是“交互系统”（Click/Drag/Bubbling），优先走 Bevy picking：`picking.md`

## 7) RenderLayers / VisibilityRange：控制“哪些东西被哪个相机看到”

### 7.1 RenderLayers：相机与实体的交集过滤（非常常用）

`RenderLayers` 的语义非常简单：

- 相机渲染实体，当且仅当两者 layers 有交集
- 默认没有 `RenderLayers` 组件也等价于在 layer 0
- 空 layers 表示“永远不可见”

源码入口：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_camera/src/visibility/render_layers.rs`

典型应用：

- 第一人称“武器 view model”只被武器相机看到（与主相机分离）
- 小地图相机只渲染特定标记层
- UI/世界分层渲染（配合 `clear_color: None` 叠加）

### 7.2 VisibilityRange：按距离裁剪可见性（性能与可读性）

Bevy v0.18 提供 `VisibilityRange`（以及对应插件），可按距离裁剪实体在视图中的可见性。

示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/3d/visibility_range.rs`

源码入口（range 模块）：`https://github.com/bevyengine/bevy/tree/v0.18.0/crates/bevy_camera/src/visibility`

## 8) 进阶：Main pass 分辨率、主纹理 usages、MSAA writeback

当你需要更底层的控制：

- **降分辨率渲染主 pass**：`MainPassResolutionOverride`（不影响后处理）
  - 定义在：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_camera/src/camera.rs`
- **让主纹理支持额外用途（例如 compute/storage/readback）**：`CameraMainTextureUsages(TextureUsages::...)`
  - 定义在：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_camera/src/camera.rs`
  - 典型案例：Solari 需要 `STORAGE_BINDING`，且常配合 `Msaa::Off`（见 `ray_tracing_solari.md` 与示例 `examples/3d/solari.rs`）
- **多相机 + MSAA 的叠加**：`MsaaWriteback` 控制写回策略
  - 定义在：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_camera/src/clear_color.rs`

## 9) 与精炼文档的对照（Context7）

Context7 `/websites/rs_bevy` 适合快速查这些 API 的签名与示例：

- `Camera` / `Camera2d` / `Camera3d`
- `Viewport` / `RenderTarget`
- `Projection` / `OrthographicProjection` / `PerspectiveProjection`
- `RenderLayers`

当你需要“真实行为/时序/必需组件”（例如 TAA 需要哪些 prepass、相机 computed 值什么时候可用）时，以 v0.18.0 源码与 examples 为准。

