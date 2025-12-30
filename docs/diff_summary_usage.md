# DiffSummary 组件使用说明

`DiffSummary` 是一个用于在会话底部汇总展示所有文件变化的 UI 组件。

## 功能特性

- 📊 **文件变化统计**: 显示修改的文件数量和增删行数
- 📁 **文件列表**: 按修改量降序排列所有文件
- 🎨 **直观标识**: 新文件标记、增删行数彩色显示
- 🔽 **可折叠界面**: 支持展开/折叠文件列表
- 🖱️ **点击跳转**: 点击文件直接跳转到详情面板

## 数据结构

### FileChangeStats
```rust
pub struct FileChangeStats {
    pub path: PathBuf,        // 文件路径
    pub additions: usize,     // 新增行数
    pub deletions: usize,     // 删除行数
    pub is_new_file: bool,    // 是否为新文件
}
```

### DiffSummaryData
```rust
pub struct DiffSummaryData {
    pub files: HashMap<PathBuf, FileChangeStats>,
    pub tool_calls: Vec<ToolCall>,  // 用于点击跳转
}
```

## 使用方法

### 1. 从 ToolCall 列表创建汇总数据

```rust
use agentx::{DiffSummary, DiffSummaryData};
use agent_client_protocol::ToolCall;

// 假设你有一个会话的所有 tool calls
let tool_calls: Vec<ToolCall> = get_session_tool_calls();

// 提取所有 Diff 信息并创建汇总
let summary_data = DiffSummaryData::from_tool_calls(&tool_calls);

// 创建 UI 组件
let diff_summary = cx.new(|_| DiffSummary::new(summary_data));
```

### 2. 在 ConversationPanel 中集成

在 `src/panels/conversation/panel.rs` 中的 `ConversationPanel` 结构体中添加字段：

```rust
pub struct ConversationPanel {
    // ... 现有字段
    diff_summary: Option<Entity<DiffSummary>>,
}
```

在构造函数中初始化：

```rust
impl ConversationPanel {
    pub fn new(session_id: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            // ... 现有字段
            diff_summary: None,
        }
    }
}
```

在处理会话更新时更新汇总：

```rust
fn update_diff_summary(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
    // 收集当前会话的所有 tool calls
    let tool_calls = self.collect_tool_calls(cx);


    // 创建汇总数据
    let summary_data = DiffSummaryData::from_tool_calls(&tool_calls);

    // 仅在有变化时显示
    if !summary_data.has_changes() {
        self.diff_summary = None;
        return;
    }

    // 更新或创建 diff_summary
    if let Some(summary) = &self.diff_summary {
        summary.update(cx, |summary, cx| {
            summary.update_data(summary_data, cx);
        });
    } else {
        self.diff_summary = Some(cx.new(|_| DiffSummary::new(summary_data)));
    }
}
```

在 `render()` 方法中渲染组件：

```rust
impl Render for ConversationPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 更新 diff summary
        self.update_diff_summary(window, cx);

        v_flex()
            .size_full()
            // ... 现有内容 (聊天记录、输入框等)
            // 在底部添加 diff summary
            .when_some(self.diff_summary.clone(), |this, summary| {
                this.child(
                    div()
                        .w_full()
                        .px_2()
                        .pb_2()
                        .child(summary)
                )
            })
    }
}
```

### 3. 手动创建汇总数据

```rust
use std::collections::HashMap;
use std::path::PathBuf;

let mut files = HashMap::new();

// 添加文件变化
files.insert(
    PathBuf::from("src/main.rs"),
    FileChangeStats {
        path: PathBuf::from("src/main.rs"),
        additions: 50,
        deletions: 10,
        is_new_file: false,
    }
);

files.insert(
    PathBuf::from("src/new_module.rs"),
    FileChangeStats {
        path: PathBuf::from("src/new_module.rs"),
        additions: 189,
        deletions: 0,
        is_new_file: true,
    }
);

// 创建数据
let data = DiffSummaryData {
    files,
    tool_calls: Vec::new(),
};

// 创建组件
let diff_summary = cx.new(|_| DiffSummary::new(data));
```

## 工具方法

### DiffSummaryData 方法

```rust
// 检查是否有变化
if data.has_changes() {
    println!("有文件被修改");
}

// 获取总文件数
let count = data.total_files();

// 获取总增加行数
let additions = data.total_additions();

// 获取总删除行数
let deletions = data.total_deletions();

// 获取按修改量排序的文件列表
let sorted_files = data.sorted_files();
```

### FileChangeStats 方法

```rust
let stats = FileChangeStats::from_diff(
    PathBuf::from("file.rs"),
    Some("old content\nline2"),  // old_text
    "new content\nline2\nline3"  // new_text
);

// 获取总变化行数
let total = stats.total_changes();
```

## 多次编辑处理

### 问题说明
当同一文件在一个会话中被多次编辑时，需要正确统计**从初始状态到最终状态**的总变化量，而不是只显示最后一次编辑的变化。

### 解决方案
组件自动追踪每个文件的：
- **初始状态**: 第一次编辑的 `old_text`
- **最终状态**: 最后一次编辑的 `new_text`

然后计算初始到最终的总 diff。

### 示例场景

**场景 1: 文件被编辑两次**
```
第一次编辑:
  old: "line1\nline2"
  new: "line1\nline2_modified"
  变化: +1 -1

第二次编辑:
  old: "line1\nline2_modified"
  new: "line1\nline2_modified\nline3"
  变化: +1

总统计 (初始→最终):
  initial: "line1\nline2"
  final:   "line1\nline2_modified\nline3"
  总变化: +2 -1 ✅ 正确

错误统计 (只看最后一次):
  总变化: +1 -0 ❌ 不准确
```

**场景 2: 添加后删除**
```
第一次: 添加 line2
  old: "line1\nline3"
  new: "line1\nline2\nline3"

第二次: 删除 line2
  old: "line1\nline2\nline3"
  new: "line1\nline3"

总统计:
  initial: "line1\nline3"
  final:   "line1\nline3"
  总变化: +0 -0 ✅ 净变化为 0
```

### 点击跳转行为
当文件被多次编辑时，点击文件行会打开一个**合并后的 diff 视图**，显示从初始状态到最终状态的总变化，而不是最后一次编辑。

**合并 diff 特性**:
- **标题**: 显示 "Edit filename.rs (N times)" 表明文件被编辑了 N 次
- **内容**: 显示初始状态 → 最终状态的完整 diff
- **状态**: 标记为 Completed
- **工具调用 ID**: `merged-{filepath}` 格式

**示例**:
```
文件: test.rs 被编辑 3 次

第 1 次: "A\nB" → "A\nB_v1"
第 2 次: "A\nB_v1" → "A\nB_v2"
第 3 次: "A\nB_v2" → "A\nB_v2\nC"

点击后显示合并 diff:
  标题: "Edit test.rs (3 times)"
  old_text: "A\nB"        (初始状态)
  new_text: "A\nB_v2\nC"  (最终状态)
  总变化: +2 -1
```

单次编辑的文件仍然显示原始的 ToolCall。

## UI 效果

组件渲染效果：

```
┌─────────────────────────────────────────┐
│ ⁕  2 files changed        +189  -0     │
│                                    ▲    │
├─────────────────────────────────────────┤
│ 📄 chess.js               +602    →    │
│ 📄 chess.html             +189  NEW →  │
└─────────────────────────────────────────┘
```

- 顶部显示文件总数和总体统计
- 点击右侧按钮可折叠/展开文件列表
- 每个文件显示：图标、文件名、增删统计、箭头
- 新文件会显示 "NEW" 标记（绿色）
- 增加行数显示为绿色，删除行数显示为红色

## 实现位置

- **组件实现**: `src/components/diff_summary.rs`
- **导出声明**: `src/components/mod.rs`
- **库导出**: `src/lib.rs`

## 依赖

组件使用以下依赖：
- `gpui`: UI 框架
- `gpui_component`: UI 组件库
- `similar`: diff 计算（用于统计增删行数）
- `agent_client_protocol`: ToolCall 和 Diff 数据结构

## 注意事项

1. **实体生命周期**: `DiffSummaryView` 应该在面板构造函数中创建并存储，而不是在 `render()` 方法中创建
2. **性能**: 对于大量文件（>100），建议添加虚拟滚动或分页
3. **更新频率**: 建议在会话更新时批量更新汇总，而不是每次 tool call 都更新
4. **图标选择**: 目前所有文件类型都使用 `IconName::File`，可以根据需要自定义图标映射

## 扩展建议

1. **点击跳转**: 在文件行添加点击事件，跳转到对应的 ToolCallDetailPanel
2. **过滤功能**: 添加按文件类型或修改量过滤的功能
3. **搜索**: 添加文件名搜索功能
4. **导出**: 添加导出变化摘要的功能（如复制到剪贴板）
5. **动画**: 添加展开/折叠动画提升用户体验
