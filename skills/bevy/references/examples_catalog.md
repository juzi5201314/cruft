# Examples 目录速查（Bevy v0.18）

Bevy 的 `examples/` 是最接近用户项目代码的“用法权威”。当你不知道某个 API 怎么用时，优先找一个同类 example 抄骨架。

- 官方索引（包含所有示例分类表）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/README.md`
- 本仓库示例文件数量（仅供感知规模）：`https://github.com/bevyengine/bevy/tree/v0.18.0/examples` 下约 300+ 个 `.rs`

下面按主题列“最常用/最能当模板”的 example（不是全量）。

## 1) App / 插件 / 调度

- `examples/hello_world.rs`
- `examples/app/plugin.rs`
- `examples/app/plugin_group.rs`
- `examples/app/headless.rs`
- `examples/app/custom_loop.rs`
- `examples/ecs/ecs_guide.rs`（系统顺序/资源/查询/命令 的讲解型大全）

## 2) ECS（进阶概念）

- `examples/ecs/message.rs`（Messages：writer/reader/mutator、chain 顺序）
- `examples/ecs/observers.rs`（Observers：Event/EntityEvent、Add/Remove 生命周期事件）
- `examples/ecs/hierarchy.rs`（ChildOf/Children、with_children/add_child）
- `examples/ecs/relationships.rs`（自定义 Relationship/RelationshipTarget、遍历、循环检测）
- `examples/ecs/run_conditions.rs`（常见 run_if 条件）
- `examples/ecs/parallel_query.rs`（并行查询）

## 3) States

- `examples/state/states.rs`（init_state、OnEnter/OnExit、run_if(in_state)）
- `examples/state/sub_states.rs`（SubStates + scoped_entities + DespawnOnExit）
- `examples/state/computed_states.rs`（ComputedStates：多源 state 推导）

## 4) Assets / Scene / glTF

- `examples/asset/asset_loading.rs`
- `examples/asset/hot_asset_reloading.rs`
- `examples/asset/custom_asset.rs`
- `examples/scene/scene.rs`
- `examples/gltf/*`（glTF 加载、动画、材质等）

## 5) Rendering（2D/3D）

2D：

- `examples/2d/sprite.rs`（最简单 Sprite）
- `examples/2d/mesh2d_manual.rs`（更底层的 2D 渲染 API）
- `examples/2d/bloom_2d.rs`（后处理）

3D：

- `examples/3d/3d_scene.rs`（3D 场景骨架）
- `examples/3d/pbr.rs`（StandardMaterial 参数展示）
- `examples/3d/tonemapping.rs`（色调映射对比）
- `examples/3d/ssao.rs` / `examples/3d/ssr.rs`（屏幕空间效果）

## 6) Shaders / Materials / GPU compute

- `examples/shader/shader_material.rs`（自定义 `Material` + WGSL）
- `examples/shader/shader_material_2d.rs`
- `examples/shader/extended_material.rs`
- `examples/shader/array_texture.rs`（`texture_2d_array` / texture array）
- `examples/shader/compute_shader_game_of_life.rs`
- `examples/shader/gpu_readback.rs`
- `examples/shader_advanced/custom_post_processing.rs`（自定义 RenderGraph 后处理 pass）
- `examples/shader_advanced/render_depth_to_texture.rs`（depth-only camera → 深度纹理采样）
- `examples/shader_advanced/specialized_mesh_pipeline.rs`（自定义 mesh pipeline）
- `examples/shader_advanced/texture_binding_array.rs`（bindless / `binding_array<texture_2d<f32>>`）

## 7) UI

入门骨架：

- `examples/ui/button.rs`
- `examples/ui/text.rs`
- `examples/ui/flex_layout.rs`

交互与导航：

- `examples/ui/standard_widgets.rs`
- `examples/ui/standard_widgets_observers.rs`
- `examples/ui/tab_navigation.rs`
- `examples/ui/directional_navigation.rs`
- `examples/ui/auto_directional_navigation.rs`

渲染到纹理 / Viewport：

- `examples/ui/render_ui_to_texture.rs`
- `examples/ui/viewport_node.rs`

## 8) Input / Window

- `examples/input/keyboard_input.rs` / `keyboard_input_events.rs`
- `examples/input/mouse_input.rs` / `mouse_input_events.rs` / `mouse_grab.rs`
- `examples/input/gamepad_input.rs` / `gamepad_rumble.rs`
- `examples/window/*`（多窗口、viewport_to_world、缩放等）

## 9) Audio / Animation

Audio：

- `examples/audio/audio.rs`
- `examples/audio/spatial_audio_3d.rs`

Animation：

- `examples/animation/animated_mesh.rs`
- `examples/animation/animation_graph.rs`

## 10) Async tasks

- `examples/async_tasks/async_compute.rs`（AsyncComputeTaskPool + Task<CommandQueue>）
- `examples/async_tasks/async_channel_pattern.rs`

## 11) Picking / Gizmos / Dev tools / Remote

Picking：

- `examples/picking/simple_picking.rs`（Pointer events + `.observe(...)`）
- `examples/picking/dragdrop_picking.rs`
- `examples/picking/mesh_picking.rs`

Gizmos：

- `examples/gizmos/2d_gizmos.rs` / `3d_gizmos.rs`

Remote（BRP）：

- `examples/remote/server.rs`
- `examples/remote/client.rs`

Dev tools / Diagnostics：

- `examples/diagnostics/*`、`examples/dev_tools/*`（视实际目录为准）
