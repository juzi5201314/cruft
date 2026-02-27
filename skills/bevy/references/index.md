# Bevy v0.18 References 索引（按需加载）

本目录把 Bevy v0.18 拆成可独立加载的主题文档。遇到 Bevy 相关问题时，先在这里做“分诊”，再加载最相关的 1~3 篇文档。

## 快速分诊（选择你要解决的问题）

1. **我在搭 App / 插件 / 系统调度**
   - `app_plugins_schedules.md`
   - 需要 feature/编译选型：`project_setup_features.md`
   - 开发期增量编译加速（dynamic linking）：`dynamic_linking_dylib.md`
   - 跨平台/窗口后端（Winit/Android/Wasm）：`platforms_winit_android_wasm.md`

2. **我在写 ECS（组件/资源/查询/命令）**
   - 入门与常用写法：`ecs_fundamentals.md`
   - 进阶（SystemState/Local/并行/Hook/错误处理等）：`ecs_advanced_patterns.md`

3. **我在做状态机 / 场景切换 / Loading**
   - `states.md`
   - 等资产再切状态：`assets.md`

4. **我在做资产加载 / 热重载 / 自定义资产格式**
   - `assets.md`
   - 场景/序列化/glTF：`scenes_gltf.md`

5. **我在做渲染（2D/3D/材质/自定义 shader）**
   - 先理解渲染子世界与调度：`rendering_architecture.md`
   - 相机/视图/渲染到纹理（viewport/split-screen/minimap）：`camera_views_render_targets.md`
   - Mesh/纹理/颜色空间（Mesh/Image/Color）：`mesh_image_color.md`
   - 光照/阴影/雾/探针：`lighting_shadows_fog.md`
   - 抗锯齿/后处理：`anti_alias_post_process.md`
   - 自定义渲染管线（RenderGraph/RenderPhase/PipelineCache）：`render_pipeline_custom_rendering.md`
   - 2D：`rendering_2d.md`
   - 3D/PBR/灯光：`rendering_3d_pbr.md`
   - 自定义材质与 shader：`shaders_materials.md`
   - 实验性光追照明（Solari）：`ray_tracing_solari.md`

6. **我在做 UI（布局/按钮/文本/交互/导航）**
   - `ui.md`
   - 工具型 UI（widgets/focus/feathers）：`ui_tooling_widgets_focus.md`
   - 可访问性（a11y/AccessKit）：`accessibility_a11y.md`

7. **我在处理输入/窗口（键鼠/手柄/触摸/多窗口）**
   - `input_window.md`

8. **我在处理时间（FixedUpdate / 手动推进 / 虚拟时间）**
   - `time_fixed_timestep.md`

9. **我在做动画 / 音频**
   - 动画：`animation.md`
   - 音频：`audio.md`

10. **我在并发/后台任务（AsyncComputeTaskPool/IoTaskPool）**
   - `async_tasks.md`

11. **我在做点击/拖拽/射线命中（Picking/Pointer events）**
   - `picking.md`
   - 若涉及 Observers：`events_messages_observers.md`

12. **我在做调试/性能/远程检查**
   - `debug_profiling_devtools_remote.md`

13. **我在配置日志 / tracing**
   - `logging_tracing.md`

14. **我想先知道 Bevy 的整体模块边界**
   - `overview_crates_plugins.md`

15. **我需要“应该看哪个 example”**
   - `examples_catalog.md`

## 约定：如何对照“源码 + examples + 文档”

每篇 reference 都包含三类指针：

- Source：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/...`（权威实现）
- Examples：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/...`（最接近用户代码的用法）
- Docs（精炼）：Context7 `/websites/rs_bevy`（概念与常见写法）

优先级建议：Examples → Source → Docs。
