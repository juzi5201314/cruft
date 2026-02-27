# UI Tooling：Widgets / Feathers / Input Focus（Bevy v0.18）

这篇聚焦“更像应用/编辑器”的 UI 体系，而不是游戏 HUD：

- `bevy_input_focus`：输入焦点与焦点导航（Tab/方向键/手柄）
- `bevy_ui_widgets`：标准控件（无样式，强调可组合与外部状态管理）
- `bevy_feathers`：带主题/风格的一套工具型 widgets（实验性，面向未来 Bevy Editor）

> UI 基础概念与布局（Node/Text/Button/Flex/Grid）见 `ui.md`；事件/Observers 机制见 `events_messages_observers.md`；可访问性见 `accessibility_a11y.md`。

## 0) 入口与 examples

源码入口：

- 输入焦点（`bevy_input_focus` crate 顶部文档很关键）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_input_focus/src/lib.rs`
- 标准 widgets（`bevy_ui_widgets`）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_ui_widgets/src/lib.rs`
- Feathers（`bevy_feathers`）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_feathers/src/lib.rs`

Examples（建议按顺序跑）：

- 标准 widgets 组合：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ui/standard_widgets.rs`
- 用 observers 写 widgets 交互：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ui/standard_widgets_observers.rs`
- Tab 导航（焦点）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ui/tab_navigation.rs`
- 方向导航（焦点）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ui/directional_navigation.rs`
- 自动方向导航：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ui/auto_directional_navigation.rs`
- Feathers 示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ui/feathers.rs`
- 虚拟键盘（移动端输入）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ui/virtual_keyboard.rs`

## 1) `bevy_input_focus`：焦点是资源（`InputFocus`），输入事件可冒泡

`bevy_input_focus` 的设计要点：

- `InputFocus` 是一个 `Resource(Option<Entity>)`：你改它就等于切焦点
- 提供 `FocusedInput<M>` 这类“从焦点实体开始冒泡”的输入事件模型
- 通过 navigation plugins（Tab/Directional）把焦点移动规则模块化

权威入口（含示例代码）：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_input_focus/src/lib.rs`

实践建议：

- 做表单/工具 UI 时，把“当前焦点实体”当成一等状态（跟 states/资源一样去管理）
- 对键盘/手柄交互优先走 focus 系统，而不是到处写 `ButtonInput<KeyCode>` 分支

## 2) `bevy_ui_widgets`：控件是“无样式 + 外部状态管理”的

`bevy_ui_widgets` 的关键原则（官方注释写得很清楚）：

- 控件不自带风格：样式由你在 UI 树上自己加 `BackgroundColor`/`BorderColor`/layout 等实现
- 多数控件使用“外部状态管理”：控件发出变更事件，你在系统里更新资源/组件，再反映回 UI
- 交互模型以 Observers/EntityEvent 为主（例如 `Activate`、`ValueChange<T>`）

入口：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_ui_widgets/src/lib.rs`

建议学习顺序：

1. 跑 `standard_widgets.rs` 看“全家桶”
2. 跑 `standard_widgets_observers.rs` 看“事件驱动 UI”长什么样

## 3) `bevy_feathers`：带主题的工具型 widgets（实验性）

Feathers 的定位是“编辑器/检查器风格的组件库”，并且明确说了：

- **不建议用于游戏 UI**
- 你可以把它当作学习材料，必要时 copy 代码进项目再改
- crate 仍是 experimental，会 break

入口与警告：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_feathers/src/lib.rs`

示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/ui/feathers.rs`

工程化建议：

- 若你在做工具/编辑器：可以把 Feathers 当“快速起步 + 参考实现”
- 若你在做游戏 UI：优先用基础 Bevy UI + 你自己的样式系统，或仅借鉴 Feathers 的布局/组件化结构

## 4) a11y（可访问性）与 focus/widgets 的连接点

工具 UI 往往要认真对待 a11y：

- 某些 widgets 由多个实体组成：要确保根实体有 `AccessibilityNode`
- disabled/checked 等状态要同步到 a11y 节点（Bevy UI 的 interaction_states 已给出明确建议）

见：`accessibility_a11y.md`

## 5) 与精炼文档的对照（Context7）

Context7 `/websites/rs_bevy` 适合快速查：

- `InputFocus` / tab navigation / directional navigation
- `UiWidgetsPlugins` / `Activate` / `ValueChange<T>`

但“控件的状态管理哲学、observers 事件模型、Feathers 的定位与约束”，以源码注释与 examples 为准。

