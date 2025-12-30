# Slash Command Autocomplete - Floating List Implementation

## Overview

命令列表现在使用**浮动列表**的方式显示，而不是内嵌在 ChatInputBox 中。这提供了更好的视觉层次和用户体验，类似于代码编辑器中的自动完成列表。

## 视觉设计

命令列表显示为一个浮动在输入框下方的独立容器：

```
┌────────────────────────────────────────────────┐
│  [Agent ▼] [Session ▼] [+] [Mode ▼]           │  ← 顶部控件
├────────────────────────────────────────────────┤
│  输入内容: /spec                               │  ← 输入框
└────────────────────────────────────────────────┘
┌────────────────────────────────────────────────┐
│ Available Commands:                            │  ← 浮动命令列表
│ /speckit.analyze   ...n.md, and tasks.md...   │
│ /speckit.plan      ...he plan template to...  │
│ /speckit.checklist ...ent feature based on... │
└────────────────────────────────────────────────┘
```

### 样式特性

- **容器**:
  - 最大宽度: 500px
  - 最大高度: 300px
  - 圆角边框
  - 背景色: `theme.background`
  - 边框: `theme.border`
  - 阴影: `shadow-lg`

- **标题**:
  - "Available Commands:"
  - 小号字体，半粗体
  - 颜色: `theme.muted_foreground`

- **命令项**:
  - 命令名: 强调色 (`theme.accent`)，中等字重
  - 描述: 前景色 (`theme.foreground`)
  - Hover 效果: 背景变为 `theme.secondary`
  - 最小宽度: 100px（命令名列）

## 实现细节

### 架构变化

**之前**: 内嵌在 ChatInputBox 内部
```rust
.child(Input)
.child(command_list)  // 内嵌
.child(buttons)
```

**现在**: 作为独立元素显示
```rust
.child(pasted_images_row)
  .child(context_element)
  .when_some(command_suggestions_element, |this, elem| {
      this.child(elem)  // 独立浮动元素
  })
.child(Input)
.child(buttons)
```

### 代码位置

**ChatInputBox** (`src/components/chat_input_box.rs`):

```rust
// 第 293-352 行: 构建命令建议列表
let command_suggestions_element = if self.show_command_suggestions && !self.command_suggestions.is_empty() {
    Some(
        v_flex()
            .w_full()
            .max_w(px(500.))
            .max_h(px(300.))
            .gap_1()
            .p_2()
            .rounded(theme.radius)
            .bg(theme.background)
            .border_1()
            .border_color(theme.border)
            .shadow_lg()
            // ... 命令项
    )
} else {
    None
};

// 第 569-573 行: 渲染到界面
.child(context_element)
.when_some(command_suggestions_element, |this, popover| {
    this.child(popover)
}),
```

**WelcomePanel** (`src/panels/welcome_panel.rs`):

新增字段用于键盘导航（未来功能）:
```rust
/// Selected command index for keyboard navigation
selected_command_index: Option<usize>,
```

初始化时设为 `None`:
```rust
selected_command_index: None,
```

## 与文件选择器的对比

| 特性 | 文件选择器 (@) | 命令列表 (/) |
|------|---------------|-------------|
| 触发方式 | 点击 "Add context" 按钮 | 输入 `/` |
| 显示方式 | Popover (弹出菜单) | 浮动列表 |
| 触发器 | Button | 无（条件渲染） |
| 位置 | 相对于按钮 | 输入框下方 |
| 宽度 | 280px | 最大 500px |
| 高度 | 300px | 最大 300px |
| 可搜索 | 是（ListState） | 是（前缀过滤） |

## 为什么不使用 Popover？

最初尝试使用 Popover 组件，但遇到了技术限制：

1. **Trigger 要求**: Popover 需要一个实现 `Selectable` trait 的 trigger（如 Button）
2. **无触发器场景**: 命令列表不需要点击按钮触发，而是根据输入自动显示
3. **简化实现**: 直接条件渲染更简单，不需要管理 Popover 的打开/关闭状态

当前实现使用 `when_some()` 条件渲染，更适合这个用例。

## 工作流程

```
用户输入 "/"
  ↓
WelcomePanel.on_input_change() 检测到斜杠
  ↓
get_available_commands() 查询命令
  ↓
过滤命令 (前缀匹配)
  ↓
更新 command_suggestions 和 show_command_suggestions
  ↓
重新渲染
  ↓
ChatInputBox 收到 command_suggestions
  ↓
构建 command_suggestions_element (Option<AnyElement>)
  ↓
使用 .when_some() 条件渲染
  ↓
显示浮动命令列表 ✓
```

## 未来增强功能

利用新增的 `selected_command_index` 字段，可以实现：

### 1. 键盘导航
```rust
// 监听键盘事件
.on_key_down(move |event, cx| {
    match event.key {
        Key::ArrowDown => {
            // 选择下一个命令
            self.selected_command_index = Some(
                self.selected_command_index
                    .map(|i| (i + 1) % self.command_suggestions.len())
                    .unwrap_or(0)
            );
        }
        Key::ArrowUp => {
            // 选择上一个命令
        }
        Key::Enter => {
            // 插入选中的命令
            if let Some(idx) = self.selected_command_index {
                let cmd = &self.command_suggestions[idx];
                // 插入命令到输入框
            }
        }
        _ => {}
    }
})
```

### 2. 高亮选中项
```rust
.children(
    commands.iter().enumerate().map(|(idx, cmd)| {
        let is_selected = self.selected_command_index == Some(idx);

        h_flex()
            .when(is_selected, |this| {
                this.bg(theme.accent.opacity(0.1))
                    .border_l_2()
                    .border_color(theme.accent)
            })
            // ... 渲染命令
    })
)
```

### 3. 鼠标悬停选中
```rust
.on_mouse_enter(move |_, cx| {
    self.selected_command_index = Some(idx);
    cx.notify();
})
```

### 4. 点击插入命令
```rust
.on_click(move |_, cx| {
    // 插入命令到输入框
    let cmd_text = format!("/{} ", cmd.name);
    input_state.update(cx, |state, cx| {
        state.set_value(&cmd_text, window, cx);
    });

    // 隐藏命令列表
    self.show_command_suggestions = false;
    cx.notify();
})
```

## 测试指南

### 基本功能测试

1. **显示所有命令**:
   ```
   输入: /
   预期: 显示浮动列表，包含所有可用命令
   ```

2. **过滤命令**:
   ```
   输入: /spec
   预期: 只显示以 "spec" 开头的命令
   ```

3. **隐藏列表**:
   ```
   操作: 删除 "/" 或输入其他字符
   预期: 命令列表消失
   ```

4. **样式验证**:
   - 列表是否浮动在输入框下方
   - 是否有阴影效果
   - 命令名是否以强调色显示
   - Hover 时背景是否变化

### 运行测试

```bash
# 启动应用
cargo run

# 或带调试日志
RUST_LOG=info,agentx::panels::welcome_panel=debug cargo run
```

测试步骤:
1. 选择一个 Agent
2. 创建或选择一个会话
3. 在输入框中输入 `/`
4. 观察命令列表显示
5. 输入更多字符测试过滤
6. 删除字符测试隐藏

## 样式定制

可以通过修改以下代码自定义样式：

```rust
// 容器尺寸
.max_w(px(500.))  // 最大宽度
.max_h(px(300.))  // 最大高度

// 背景和边框
.bg(theme.background)      // 背景色
.border_color(theme.border) // 边框颜色

// 命令名样式
.text_color(theme.accent)   // 命令名颜色
.min_w(px(100.))            // 最小宽度

// Hover 效果
.hover(|this| this.bg(theme.secondary))
```

## 已知限制

1. **无滚动条**: 当命令很多时，需要手动添加滚动支持
2. **无键盘导航**: 当前不支持上下箭头选择
3. **无点击选择**: 不能点击命令自动插入
4. **固定位置**: 列表位置固定，不会根据可用空间调整

这些功能可以通过未来的增强来实现。

## 总结

新的浮动列表实现：
- ✅ 更清晰的视觉层次
- ✅ 与文件选择器一致的设计语言
- ✅ 简化的代码实现（无 Popover 复杂性）
- ✅ 为未来键盘导航做好准备
- ✅ 编译成功，无错误

命令列表现在是一个独立的浮动元素，而不是内嵌在输入框中，提供了更好的用户体验！
