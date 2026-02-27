# Input / Window（Bevy v0.18）：键鼠/手柄/触摸、窗口事件、多窗口

Bevy v0.18 的输入系统以“资源状态 + 消息流”结合为主：

- “当前按键是否按下”：`Res<ButtonInput<KeyCode>>`
- “这一帧有哪些输入事件”：`MessageReader<KeyboardInput>` 等

## 0) 入口与源码

- InputPlugin：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_input/src/lib.rs`
- WindowPlugin：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_window/src/lib.rs`
- examples：
  - `examples/input/keyboard_input.rs`
  - `examples/input/keyboard_input_events.rs`
  - `examples/input/mouse_input.rs`
  - `examples/window/*`

## 1) 键盘：状态读取（推荐） vs 事件流（需要细节时）

### 1.1 状态读取：`ButtonInput<KeyCode>`

```rust
fn movement(input: Res<ButtonInput<KeyCode>>) {
    if input.pressed(KeyCode::ArrowLeft) {
        // ...
    }
    if input.just_pressed(KeyCode::Space) {
        // ...
    }
}
```

这种写法在 states 示例里也大量使用：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/state/states.rs`

### 1.2 事件流：`MessageReader<KeyboardInput>`

当你需要“按键事件的更细信息”（例如扫描码、重复等），读消息：

- 示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/input/keyboard_input_events.rs`

```rust
fn print_keyboard_event_system(mut keyboard_inputs: MessageReader<KeyboardInput>) {
    for keyboard_input in keyboard_inputs.read() {
        info!("{:?}", keyboard_input);
    }
}
```

这些输入事件是 **Messages**（pull-based），不是 Observers 的 Events。

## 2) 常用 run conditions：把输入写成 `run_if(...)`

input crate 提供了常见条件函数（common_conditions）：

- 示例：`https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/examples/remote/server.rs`（`input_just_pressed(KeyCode::Space)`）

这种写法能让系统更“声明式”，并减少无意义的每帧判断。

## 3) 鼠标/触摸/手柄：同样是状态 + 消息

InputPlugin 会按 feature 安装：

- MouseButtonInput / MouseMotion / MouseWheel（消息）
- ButtonInput<MouseButton>（状态）
- TouchInput / Touches（消息 + 状态）
- GamepadEvent、rumble request 等（消息）

源码入口：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_input/src/lib.rs`（`InputPlugin::build`）

## 4) Window：窗口也是 ECS 实体 + 一组消息

WindowPlugin 会注册大量窗口相关消息：

- `WindowResized` / `WindowFocused` / `CursorMoved` / `FileDragAndDrop` 等

源码入口：

- `https://github.com/bevyengine/bevy/raw/refs/tags/v0.18.0/crates/bevy_window/src/lib.rs`（`WindowPlugin::build`）

常见读取方式：

- `Query<&Window>`（当前窗口状态，如 cursor_position）
- `MessageReader<WindowResized>`（窗口事件流）

## 5) 多窗口与相机 viewport_to_world

当你需要做：

- 多窗口 UI
- 多相机/分屏/子视图
- 用鼠标位置射线投射（2D/3D picking 等）

通常会组合：

- `Window::cursor_position()`
- `Camera::viewport_to_world` 或 `viewport_to_world_2d`

参考 examples：

- `examples/2d/2d_viewport_to_world.rs`
- `examples/3d/3d_viewport_to_world.rs`
- `examples/window/*`

## 6) 与 Picking 的关系

Picking 体系会消费输入与窗口信息，产生 pointer events，并通过 Observers 分发：

- picking pipeline 文档：`picking.md`
- 示例：`examples/picking/simple_picking.rs`
