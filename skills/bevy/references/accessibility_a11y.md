# Accessibility / a11y（Bevy v0.18）

这篇聚焦 Bevy 的可访问性（accessibility）集成：`bevy_a11y`（AccessKit primitives）+ `bevy_winit`（平台适配）+ `bevy_ui`（UI 节点生成与状态同步）。

目标是让你的 UI/工具在屏幕阅读器等辅助技术下可用，并且在交互状态（disabled/checked 等）变化时正确更新 a11y 树。

## 0) 入口与 examples

源码入口：

- `bevy_a11y` crate（核心类型与插件）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_a11y/src/lib.rs`
- DefaultPlugins 中的安装条件（`bevy_window` 启用时安装 `AccessibilityPlugin`）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_internal/src/default_plugins.rs`
- `bevy_ui` 的 AccessKit 集成（把 Button/Label/Image 变成 AccessibilityNode，并更新 bounds）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_ui/src/accessibility.rs`
- `bevy_ui` 交互状态与 a11y 同步（disabled/toggled 等）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_ui/src/interaction_states.rs`
- `bevy_winit` 的平台层适配（accesskit_winit Adapter、ActionRequest 转发等）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_winit/src/accessibility.rs`

Examples（看“怎么在真实 UI 树里挂节点/角色”）：

- Scroll UI 示例包含 a11y 节点：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ui/scroll.rs`
- 完整 UI testbed 也包含 a11y：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/testbed/full_ui.rs`

## 1) 核心概念：`AccessibilityNode` 是“把 ECS 实体映射到 AccessKit 节点”

`bevy_a11y` 提供：

- `AccessibilityNode(accesskit::Node)`：组件，描述该实体在可访问性树中的角色与属性
- `AccessibilityPlugin`：初始化必要资源、系统集等
- `AccessibilityRequested` / `ManageAccessibilityUpdates`：控制是否需要推送更新（当辅助技术请求时）
- `ActionRequest` wrapper：把平台层 action request 变成 Bevy 消息/事件

权威说明在 crate 顶部文档：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_a11y/src/lib.rs`

## 2) “我需要自己加 AccessibilityNode 吗？”

结论：**看你用什么 UI/控件体系**。

- 如果你使用 Bevy UI 的基础组件（Button/Label/ImageNode 等），`bevy_ui` 的内部 accessibility 系统会在变更时尝试插入/更新 `AccessibilityNode`：
  - `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_ui/src/accessibility.rs`
- 如果你写的是自定义 widget（多个实体组成一个控件），你通常需要：
  - 确保“根实体”有合适的 `AccessibilityNode`（Role/label/value 等）
  - 并把交互状态（disabled/checked）更新同步到该节点

Bevy UI 的交互状态组件文档里甚至明确提醒：`InteractionDisabled` 应该加在“包含 AccessibilityNode 的根实体”上，保证树更新正确：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_ui/src/interaction_states.rs`

## 3) a11y 与 UI Widgets / Feathers 的关系（工具链方向）

Bevy 的 `bevy_ui_widgets` 与 `bevy_feathers`（偏 editor/inspector/tooling）会主动在控件上设置 `AccessibilityNode`（例如 checkbox/radio/slider 的 role）。

作为参考入口：

- `bevy_ui_widgets`：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_ui_widgets/src/lib.rs`
- `bevy_feathers`：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_feathers/src/lib.rs`
- 对应示例（Feathers）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ui/feathers.rs`

（更系统的 tooling/UI 架构见 `ui_tooling_widgets_focus.md`）

## 4) 平台层：`bevy_winit` 如何把 a11y 树交给 OS

在桌面平台上，`bevy_winit` 使用 `accesskit_winit` 的 `Adapter` 把 Bevy 的 `AccessibilityNode` 图同步到 OS，并把用户的 action request 回传到 Bevy。

入口：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_winit/src/accessibility.rs`

工程含义：

- 如果你禁用了窗口后端（无 `bevy_winit` / 无 `bevy_window`），a11y 的平台集成自然也不会工作。
- a11y 更新是有成本的：当没有辅助技术请求时，通常不会持续推送（`AccessibilityRequested`）。

## 5) 与精炼文档的对照（Context7）

Context7 `/websites/rs_bevy` 适合快速查：

- `AccessibilityPlugin` / `AccessibilityNode`

但 UI 自动生成节点、winit adapter、disabled/checked 同步等行为细节，以源码与 examples 为准。

