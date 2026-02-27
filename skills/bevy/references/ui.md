# UI（Bevy v0.18）：Node / Text / Button / 布局 / 交互

Bevy UI 是一套基于 ECS 的 UI 树系统：UI 元素也是实体，布局由系统在每帧（PostUpdate）计算。

## 0) 入口与 examples

- UI crate 入口（含 `UiSystems` 与 prelude）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_ui/src/lib.rs`
- 常用 examples（挑几个先跑起来）：
  - `examples/ui/button.rs`（按钮交互骨架）
  - `examples/ui/text.rs`（文本）
  - `examples/ui/flex_layout.rs`（Flexbox）
  - `examples/ui/grid.rs`（Grid）
  - `examples/ui/standard_widgets.rs`（更完整的 widgets 组合）
  - `examples/ui/standard_widgets_observers.rs`（用 observers 写 UI 交互）
  - `examples/ui/render_ui_to_texture.rs`（渲染到纹理）

## 1) UI 的“基本组件集合”

常见 UI 实体会包含：

- `Node`：布局与样式（尺寸、对齐、flex/grid 等）
- `Text`（UI Text）：文字内容（`bevy_ui::widget::Text`）
- `Button`：按钮标记组件
- `ImageNode`：图片节点（以及 slicing/atlas 等）
- `Interaction`：交互状态（Pressed/Hovered/None）
- `BackgroundColor` / `BorderColor` / gradients 等视觉组件

UI prelude 会导出大部分常用类型：

- Source：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_ui/src/lib.rs`（`pub mod prelude`）

## 2) 生成 UI 树：`children![ ... ]` 与父子结构

Bevy UI 常用“在一个 spawn 里带孩子”的写法：

- 示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/state/states.rs`（`setup_menu`）

你会看到：

- 父节点 `Node { width: percent(100), ... }`
- `children![ (Button, Node {...}, BackgroundColor(...), children![ (Text::new("Play"), ... ) ]) ]`

这让 UI 树结构非常直观，适合当模板。

## 3) 交互：查询 `Interaction` 并用 `Changed<Interaction>` 降低开销

经典按钮交互模式：

```rust
fn menu(
  mut interaction_query: Query<(&Interaction, &mut BackgroundColor), (Changed<Interaction>, With<Button>)>,
) {
  for (interaction, mut color) in &mut interaction_query {
    match *interaction {
      Interaction::Pressed => { /* ... */ }
      Interaction::Hovered => { /* ... */ }
      Interaction::None => { /* ... */ }
    }
  }
}
```

这段在 state 示例里就是完整可用版本：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/state/states.rs`

## 4) 布局：Flexbox 与 Grid

Bevy UI 支持：

- Flexbox（最常用）
- CSS Grid（更结构化）

入口示例：

- Flex：`examples/ui/flex_layout.rs`
- Grid：`examples/ui/grid.rs`

建议：

- 先用 Flex 把 UI 搭出来，再考虑 Grid 优化结构。
- 多做“组件化”：把 UI 的 spawn 封装成函数/插件，避免所有 UI 写在一个 setup system 里。

## 5) 文本渲染与测量（TextPipeline）

UI Text 的渲染流程（测量 → layout → glyph atlas）在 `bevy_text` 文档里解释得很清楚：

- Source：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_text/src/lib.rs`

如果你遇到：

- 文本测量不符合预期
- 字体 atlas/性能问题

先去读这份文档，再看 UI crate 里 `measure_text_system` / `text_system` 的调度位置：

- Source：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_ui/src/lib.rs`（`build_text_interop`）

## 6) Focus / 导航 / 输入焦点

Bevy UI 内置 focus 系统与导航示例：

- `examples/ui/tab_navigation.rs`
- `examples/ui/directional_navigation.rs`
- `examples/ui/auto_directional_navigation.rs`

如果你需要更强的“输入焦点管理”（比如文本输入框/快捷键路由），也可能会涉及 `bevy_input_focus`。

## 7) UI 与 Picking/Observers 的组合

UI 可以与 picking 体系集成（按 feature）：

- Source：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_ui/src/lib.rs`（`picking_backend`）
- picking 总览：`picking.md`

写交互时也可以用 Observers（更接近“事件驱动 UI”）：

- `examples/ui/standard_widgets_observers.rs`
- Observer 概念：`events_messages_observers.md`

## 8) UI 渲染到纹理 / Viewport

进阶玩法：

- UI 作为某个 render target（例如做 3D 世界里的屏幕）
  - `examples/ui/render_ui_to_texture.rs`
- Viewport 节点
  - `examples/ui/viewport_node.rs`

这类需求通常还要配合相机/渲染目标设置（参见 `input_window.md` 与 `rendering_architecture.md`）。
