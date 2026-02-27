# Bevy Geist UI 使用指南 (v0.18)

本套 UI 组件旨在 Bevy 0.18 中还原 Vercel (Geist) 与 shadcn/ui 的设计语言：高对比度、细边框、现代字体排版以及柔和的程序化背景。

## 1. 快速开始

### 注册插件
在 `main.rs` 中添加 `CruftUiAssetsPlugin` 和 `CruftUiPlugin`：

```rust
use cruft_ui::prelude::*;

fn main() {
    App::new()
        .add_plugins(CruftUiAssetsPlugin) // 嵌入字体与图标资源
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
        let mut ui = cruft_ui::ui::UiBuilder::new(parent, &theme);
        ui.card(|ui| {
            ui.button(UiButtonVariant::Primary, |ui| {
                ui.icon('\u{e9b2}'); // Play icon
                ui.label("Deploy");
            });
        }).size(Val::Px(300.0), Val::Auto);
    });
}
```

---

## 2. 核心特性

### 字体系统 (Geist Font Family)
已集成 Vercel 的 Geist 字体，支持三种变体：
- **`label("Text")`**: Geist Sans Regular (14px).
- **`label_semibold("Text")`**: Geist Sans SemiBold (14px).
- **`label_mono("Text")`**: Geist Mono Regular (13px).

### 图标系统 (Lucide Icons)
集成 Lucide 字体图标。
- **`icon(code)`**: 使用 Unicode 字符显示图标。
- 示例：`ui.icon('\u{e9b2}')` (Play), `ui.icon('\u{e9bb}')` (Menu)。

### 自动交互与更新系统
`CruftUiPlugin` 内置了以下系统：
- **Button Skin**：根据 `Interaction` / `UiButtonVariant` 自动刷新样式。
- **`UiClick` (EntityEvent)**：按钮按下时触发。
- **Progress**：动态更新进度。

---

## 3. 组件状态报告

### ✅ 已实现 (Implemented)
| 组件 / 特性 | 状态 | 备注 |
| :--- | :--- | :--- |
| **UiTheme** | 核心 | 支持 Light/Dark 切换。 |
| **Fonts** | 完成 | 集成 Geist Sans, Geist Mono。 |
| **Icons** | 完成 | 集成 Lucide 字体图标。 |
| **Card (卡片)** | 完成 | 包含 Border、Radius 和 BoxShadow。 |
| **Button (按钮)** | 完成 | 支持 `Primary`, `Secondary`, `Ghost`。 |
| **Progress (进度条)** | 完成 | 包含圆角轨道与动态进度条。 |

### ❌ 待实现 (Roadmap)
| 组件 | 优先级 | 技术挑战 |
| :--- | :--- | :--- |
| **Input (输入框)** | 高 | 需要文本编辑逻辑。 |
| **Select (下拉框)** | 高 | 涉及弹出层布局。 |
| **Tabs (标签页)** | 低 | 基于 States 的切换。 |

---

## 4. 样式自定义

你可以通过修改 `UiTheme` 资源来全局调整样式。
