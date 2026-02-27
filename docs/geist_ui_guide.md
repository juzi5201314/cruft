# Bevy Geist UI 使用指南 (v0.18)

本套 UI 组件旨在 Bevy 0.18 中还原 Vercel (Geist) 与 shadcn/ui 的设计语言：高对比度、细边框、现代字体排版以及柔和的程序化背景。

## 1. 快速开始

### 注册插件
在 `main.rs` 中添加 `CruftUiPlugin`：

```rust
use cruft_ui::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(CruftUiPlugin) // 自动注册材质与主题资源
        .add_systems(Startup, setup)
        .run();
}
```

### 基础布局
使用 `UiTheme` 资源来保持视觉一致性：

```rust
fn setup(mut commands: Commands, theme: Res<UiTheme>) {
    commands.spawn(Camera2d);
    
    commands.spawn(Node {
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        ..default()
    }).with_children(|parent| {
        cruft_ui::ui::card(parent, &theme)
            .size(Val::Px(300.0), Val::Auto)
            .with_children(|p, theme| {
                cruft_ui::ui::button(p, theme)
                    .text("Deploy")
                    .variant(UiButtonVariant::Primary);
            });
    });
}
```

### Progress (进度条)
展示任务完成度，支持动态更新。
```rust
cruft_ui::ui::progress(
    parent,
    &theme,
    0.5, // 初始进度 (0.0 - 1.0)
    Val::Px(400.0),
);
```

---

## 2. 核心特性

### 自动交互与更新系统
`CruftUiPlugin` 内置了以下系统（语义组件 + 皮肤系统）：
- **Button Skin**：根据 `Interaction` / `UiButtonVariant` / `UiButtonStyleOverride` 自动刷新样式。
- **`UiClick` (EntityEvent)**：按钮按下时触发，支持 `.click(handler)` 绑定 observer。
- **Progress**：当 `UiProgress.value` 改变时，自动更新填充宽度。
- **Responsive Flex**：`UiResponsiveFlex` 根据窗口宽度自动切换 `flex_direction`。

### 程序化网格背景 (`GeistGridMaterial`)
实现 Vercel 官网上那种极简的 40px 网格。它是一个 `UiMaterial`，可以直接挂载到 `MaterialNode` 上。
- **自定义**: 可在创建时调整 `spacing` (间隔) 和 `thickness` (粗细)。

### 自动交互系统
按钮与卡片皮肤默认包含：
- **Hover**: 变暗或显示背景色。
- **Focus Ring (Demo)**: 使用 `Outline` 实现悬浮外圈（后续可替换为真正的 focus 状态）。
- **Shadows**: 卡片自带 `0 1px 2px rgba(0,0,0,0.05)` 的软阴影。

---

## 3. 组件状态报告

### ✅ 已实现 (Implemented)
| 组件 / 特性 | 状态 | 备注 |
| :--- | :--- | :--- |
| **UiTheme** | 核心 | 支持 Light/Dark 切换（Geist Token），集中管理色彩。 |
| **Grid Background** | 完成 | 基于 WGSL 的程序化渲染网格。 |
| **Card (卡片)** | 完成 | 包含 Border、Radius 和 BoxShadow。 |
| **Button (按钮)** | 完成 | 支持 `Primary` (纯黑), `Secondary` (白底黑框), `Ghost` (透明)。 |
| **Progress (进度条)** | 完成 | 包含圆角轨道与动态进度条。 |
| **Focus Ring** | 完成 | 使用 Bevy 0.18 的 `Outline` 组件实现（Demo 级）。 |
| **交互逻辑** | 完成 | `UiClick` (EntityEvent) + Observers。 |

### ❌ 待实现 (Roadmap)
| 组件 | 优先级 | 技术挑战 |
| :--- | :--- | :--- |
| **Input (输入框)** | 高 | 需要集成 `bevy_input_focus` 和文本编辑逻辑。 |
| **Badge (徽章)** | 中 | 简单的文本胶囊封装。 |
| **Avatar (头像)** | 中 | 需要实现圆角遮罩 (Clipping)。 |
| **Select (下拉框)** | 高 | 涉及弹出层布局与点击区域判定。 |
| **Tabs (标签页)** | 低 | 基于 States 的 UI 切换逻辑。 |
| **Menu (菜单)** | 高 | 复杂的 z-index 管理与浮动层。 |

---

## 4. 样式自定义

你可以通过修改 `UiTheme` 资源来全局调整样式。例如，如果你想要更圆润的风格：

```rust
let mut theme = UiTheme::geist_light();
theme.radius = 12.0; // 修改为 12px 圆角
commands.insert_resource(theme);
```

### 黑暗模式 (Dark Mode)
只需在初始化或运行时更换资源：
```rust
commands.insert_resource(UiTheme::geist_dark());
```
系统会自动应用新的背景色 (`#000`) 和边框色 (`#333`)。
