# 会话消息持久化功能测试指南

## 功能概述

新增的会话消息持久化功能可以：
1. **自动保存**：将所有 Agent 消息自动保存到 JSONL 文件中
2. **历史加载**：点击左侧任务列表时，ConversationPanel 面板自动加载历史记录
3. **会话隔离**：每个会话的消息保存在独立的文件中

## 文件位置

- **开发模式**：`target/sessions/{session_id}.jsonl`
- **发布模式**：`sessions/{session_id}.jsonl`

## 测试步骤

### 1. 创建新会话并发送消息

1. 启动应用程序：
   ```bash
   RUST_LOG=info,agentx::core::services=debug,agentx::panels::conversation_acp=debug cargo run
   ```

2. 在左侧任务列表中选择一个 Agent 任务，这会创建一个新的会话

3. 在 Conversation 面板的输入框中输入消息并发送

4. 观察日志输出，应该看到类似：
   ```
   [DEBUG] Published user message to session bus: <session_id>
   [DEBUG] Saved message to session file: target/sessions/<session_id>.jsonl
   ```

### 2. 检查持久化文件

查看会话文件是否已创建：
```bash
ls -la target/sessions/
cat target/sessions/<session_id>.jsonl
```

文件格式示例：
```json
{"timestamp":"2025-12-02T18:00:00Z","update":{"UserMessageChunk":{"content_block":{...}}}}
{"timestamp":"2025-12-02T18:00:01Z","update":{"AgentMessageChunk":{"content_block":{...}}}}
```

### 3. 测试历史加载

1. 关闭当前的 Conversation 面板（点击面板关闭按钮）

2. 再次点击左侧任务列表中的同一个任务

3. 观察日志输出，应该看到：
   ```
   [INFO] Loading history for session: <session_id>
   [INFO] Loaded N historical messages for session: <session_id>
   [DEBUG] Loading historical message 0: timestamp=...
   [DEBUG] Loading historical message 1: timestamp=...
   ...
   [INFO] Loaded history for session <session_id>: N items, next_index=N
   ```

4. 面板应该显示之前的所有消息

### 4. 测试消息持续追加

1. 在已经加载历史的面板中继续发送新消息

2. 新消息应该同时：
   - 实时显示在面板中
   - 追加到 JSONL 文件末尾

3. 检查文件，确认新消息已追加：
   ```bash
   tail -5 target/sessions/<session_id>.jsonl
   ```

## 预期行为

### 自动持久化

- ✅ 用户消息立即保存
- ✅ Agent 响应自动保存
- ✅ Tool Call 自动保存
- ✅ Plan 自动保存
- ✅ 所有 SessionUpdate 类型都会保存

### 历史加载

- ✅ 打开会话时自动加载历史
- ✅ 历史消息按时间顺序显示
- ✅ 加载完成后滚动到底部
- ✅ 新消息的 index 从历史记录后继续

### 错误处理

- ⚠️ 如果文件不存在，返回空列表（正常情况，首次会话）
- ⚠️ 如果 JSON 解析失败，记录警告并继续读取其他行
- ❌ 如果目录无法创建，记录错误

## 调试技巧

### 查看详细日志

```bash
# 查看持久化相关的所有日志
RUST_LOG=agentx::core::services::persistence_service=trace cargo run

# 查看消息服务日志
RUST_LOG=agentx::core::services::message_service=debug cargo run

# 查看面板加载日志
RUST_LOG=agentx::panels::conversation_acp=debug cargo run

# 综合调试
RUST_LOG=info,agentx::core::services=debug,agentx::panels::conversation_acp=debug cargo run
```

### 手动检查会话文件

```bash
# 列出所有会话
ls -1 target/sessions/*.jsonl | xargs -n1 basename

# 查看会话消息数量
wc -l target/sessions/*.jsonl

# 查看最新消息
tail -1 target/sessions/<session_id>.jsonl | jq '.'

# 查看所有用户消息
cat target/sessions/<session_id>.jsonl | jq 'select(.update.UserMessageChunk != null)'
```

### 清理测试数据

```bash
# 删除所有会话文件
rm -rf target/sessions/

# 删除特定会话
rm target/sessions/<session_id>.jsonl
```

## 已知限制

1. **文件格式固定为 JSONL**：每行一个 JSON 对象，不支持其他格式
2. **时间戳为 UTC**：保存的时间戳采用 ISO 8601 格式（UTC 时区）
3. **无自动清理**：旧的会话文件不会自动删除，需要手动管理
4. **顺序写入**：消息按接收顺序写入，不会重新排序

## 性能考虑

- ✅ **异步写入**：文件操作在后台异步执行，不阻塞 UI
- ✅ **追加模式**：使用 append 模式打开文件，避免重写整个文件
- ✅ **独立会话**：每个会话独立文件，避免单个大文件
- ⚠️ **大会话**：非常长的会话（数千条消息）可能导致加载时间较长

## 故障排查

### 消息没有保存

1. 检查 MessageService 是否初始化：
   ```
   [INFO] Initialized service layer (AgentService, MessageService, PersistenceService)
   ```

2. 检查是否有写入错误：
   ```bash
   grep "Failed to persist message" <log_file>
   ```

3. 检查目录权限：
   ```bash
   ls -ld target/sessions/
   ```

### 历史加载失败

1. 检查文件是否存在：
   ```bash
   ls target/sessions/<session_id>.jsonl
   ```

2. 检查文件内容格式：
   ```bash
   cat target/sessions/<session_id>.jsonl | jq '.'
   ```

3. 查看加载错误日志：
   ```bash
   grep "Failed to load history" <log_file>
   ```

### JSON 解析错误

如果看到 "Failed to parse line" 警告：

1. 检查文件中是否有损坏的行：
   ```bash
   cat target/sessions/<session_id>.jsonl | while read line; do
     echo "$line" | jq '.' > /dev/null || echo "Invalid: $line"
   done
   ```

2. 手动修复或删除损坏的行

## 扩展功能建议

未来可以考虑添加：

- [ ] 会话导出功能（导出为 JSON、Markdown 等格式）
- [ ] 会话搜索（在历史消息中搜索关键词）
- [ ] 会话压缩（压缩旧的会话文件以节省空间）
- [ ] 会话统计（消息数量、时间跨度等）
- [ ] 自动清理策略（删除超过 N 天的会话）
- [ ] 会话备份和恢复
