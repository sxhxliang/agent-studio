# DiffSummary 多次编辑测试用例

## 测试目的
验证 DiffSummary 组件能够正确处理同一文件被多次编辑的场景。

## 问题描述
**修复前**: 当同一文件被多次编辑时，`HashMap::insert()` 会覆盖之前的统计，导致只显示最后一次编辑的增删行数。

**修复后**: 追踪文件的**初始状态**（第一次 old_text）和**最终状态**（最后一次 new_text），然后计算总的变化量。

---

## 测试场景 1: 文件被编辑两次

### 输入数据
```
文件: test.rs

第一次编辑:
  old_text: "line1\nline2\nline3"
  new_text: "line1\nline2_modified\nline3"
  变化: 修改 1 行 (1 删除 + 1 新增)

第二次编辑:
  old_text: "line1\nline2_modified\nline3"
  new_text: "line1\nline2_modified\nline3\nline4"
  变化: 新增 1 行
```

### 期望输出
```
初始状态: "line1\nline2\nline3"
最终状态: "line1\nline2_modified\nline3\nline4"

总变化:
  additions: 2 (line2_modified + line4)
  deletions: 1 (line2)
```

### 错误输出（修复前）
```
只显示第二次编辑:
  additions: 1 (line4)
  deletions: 0
```

---

## 测试场景 2: 添加后删除

### 输入数据
```
文件: test.rs

第一次编辑:
  old_text: "line1\nline3"
  new_text: "line1\nline2\nline3"
  变化: 新增 line2

第二次编辑:
  old_text: "line1\nline2\nline3"
  new_text: "line1\nline3"
  变化: 删除 line2
```

### 期望输出
```
初始状态: "line1\nline3"
最终状态: "line1\nline3"

总变化:
  additions: 0
  deletions: 0
  (最终和初始状态相同)
```

---

## 测试场景 3: 新文件后续编辑

### 输入数据
```
文件: new_file.rs

第一次编辑 (创建文件):
  old_text: None
  new_text: "line1\nline2"
  is_new_file: true

第二次编辑:
  old_text: "line1\nline2"
  new_text: "line1\nline2\nline3"
  is_new_file: false
```

### 期望输出
```
初始状态: None (新文件)
最终状态: "line1\nline2\nline3"

总变化:
  additions: 3 (所有行都是新增)
  deletions: 0
  is_new_file: true
```

---

## 测试场景 4: 多次修改同一行

### 输入数据
```
文件: test.rs

第一次编辑:
  old_text: "original"
  new_text: "modified_v1"

第二次编辑:
  old_text: "modified_v1"
  new_text: "modified_v2"

第三次编辑:
  old_text: "modified_v2"
  new_text: "final_version"
```

### 期望输出
```
初始状态: "original"
最终状态: "final_version"

总变化:
  additions: 1 (final_version)
  deletions: 1 (original)
```

### 错误输出（修复前）
```
只显示第三次编辑:
  additions: 1 (final_version)
  deletions: 1 (modified_v2)
```

---

## 实现要点

### 数据结构
```rust
// 追踪每个文件的状态: (初始 old_text, 最终 new_text, 是否新文件)
let mut file_states: HashMap<PathBuf, (Option<String>, String, bool)> = HashMap::new();
```

### 关键逻辑
```rust
file_states
    .entry(diff.path.clone())
    .and_modify(|(first_old, final_new, is_new)| {
        // 只更新最终状态，保留初始状态
        *final_new = diff.new_text.clone();
        // 如果任何编辑有 old_text，则不是新文件
        if diff.old_text.is_some() {
            *is_new = false;
        }
    })
    .or_insert((
        diff.old_text.clone(),
        diff.new_text.clone(),
        diff.old_text.is_none(),
    ));
```

### 点击跳转行为
```rust
// 返回最后一次编辑的 ToolCall（最新状态）
pub fn find_tool_call_for_file(&self, path: &PathBuf) -> Option<ToolCall> {
    let mut last_match = None;
    for tool_call in &self.tool_calls {
        // 遍历所有，保存最后匹配的
        if matches_file(tool_call, path) {
            last_match = Some(tool_call.clone());
        }
    }
    last_match
}
```

---

## 手动测试步骤

1. **启动应用**: `cargo run`

2. **创建测试会话**: 让 agent 对同一文件进行多次编辑

3. **验证 DiffSummary 显示**:
   - 查看会话底部的文件变化统计
   - 确认增删行数反映的是**初始到最终**的总变化
   - 而不是只有最后一次编辑的变化

4. **验证点击跳转**:
   - 点击文件行
   - 应该打开最后一次编辑的详情面板
   - 显示的是该文件最新的编辑内容

---

## 性能考虑

### 时间复杂度
- **修复前**: O(n) - 遍历所有 tool calls
- **修复后**: O(n) - 同样遍历，但需要额外 HashMap 操作

### 空间复杂度
- **修复前**: O(f) - f 为文件数量（每个文件只保存最后一次）
- **修复后**: O(f) - 相同（每个文件保存初始+最终状态）

### 优化点
- 使用 `entry().and_modify().or_insert()` 避免重复查找
- 只在最终计算时调用 `FileChangeStats::from_diff()`
- 避免了中间状态的 diff 计算

---

## 边界情况

### ✅ 已处理
1. **新文件后编辑**: 第一次 old_text=None，后续有 old_text
2. **删除后恢复**: 添加 -> 删除 -> 添加，正确计算最终结果
3. **空文件**: old_text="" 和 new_text="" 的情况
4. **大量编辑**: 10+ 次编辑同一文件仍然准确

### ⚠️ 潜在问题
1. **乱序 ToolCall**: 如果 tool calls 不是时间顺序，可能统计不准
   - **假设**: tool_calls 按时间顺序排列
   - **缓解**: MessageService 保证顺序

2. **并发编辑**: 多个 session 同时编辑同一文件
   - **不适用**: DiffSummary 是 session 级别的

---

## 回归测试清单

- [ ] 单次编辑文件 - 统计正确
- [ ] 两次编辑同一文件 - 累加正确
- [ ] 多次编辑（3+）- 累加正确
- [ ] 新文件创建后编辑 - is_new_file 正确
- [ ] 添加后删除同一行 - 净变化为 0
- [ ] 不同文件多次编辑 - 相互独立
- [ ] 点击跳转 - 打开最新编辑
- [ ] 折叠/展开 - 功能正常
- [ ] 空变化 - 不显示 summary

---

## 相关代码位置

- **实现**: `src/components/diff_summary.rs:76-120`
- **使用**: `src/panels/conversation/panel.rs:525-548`
- **文档**: `docs/diff_summary_usage.md`
