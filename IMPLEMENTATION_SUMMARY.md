# 会话消息持久化功能实现总结

## 功能概述

成功实现了会话消息持久化功能，可以将 Agent 的消息自动保存到 JSONL 文件中，并在点击左侧面板任务时自动加载会话历史记录。

## 实现的功能

### 1. 持久化服务 (PersistenceService)

**文件**: `src/core/services/persistence_service.rs`

- **数据结构**: `PersistedMessage` 包含时间戳和 SessionUpdate
- **保存**: `save_update()` - 将消息追加到 JSONL 文件
- **加载**: `load_messages()` - 从 JSONL 文件加载所有历史消息
- **删除**: `delete_session()` - 删除会话文件
- **列表**: `list_sessions()` - 列出所有可用会话

**特点**:
- 使用 JSONL 格式（每行一个 JSON 对象）
- 包含 ISO 8601 格式的时间戳
- 异步文件操作（不阻塞 UI）
- 自动创建目录

### 2. MessageService 集成

**文件**: `src/core/services/message_service.rs`

- **初始化**: `init_persistence()` - 订阅 SessionUpdateBus 并自动保存消息
- **加载历史**: `load_history()` - 加载会话的历史消息
- **删除历史**: `delete_history()` - 删除会话历史
- **列表会话**: `list_sessions_with_history()` - 列出所有有历史的会话

**特点**:
- 自动持久化所有 SessionUpdate 事件
- 在异步上下文中初始化（避免运行时错误）
- 错误处理和日志记录

### 3. ConversationPanel 历史加载

**文件**: `src/panels/conversation_acp/panel.rs`

- **加载历史**: `load_history_for_session()` - 在面板初始化时加载历史
- **异步加载**: 在后台线程加载，不阻塞 UI
- **自动滚动**: 加载完成后滚动到底部
- **索引管理**: 正确维护 `next_index` 以便新消息从历史后继续

**流程**:
1. 创建面板实体
2. **加载历史记录**（新增）
3. 订阅新消息
4. 订阅权限请求

### 4. AppState 初始化

**文件**: `src/app/app_state.rs`, `src/main.rs`

- 在 `set_agent_manager()` 中创建 PersistenceService
- 在 `main.rs` 的异步上下文中调用 `init_persistence()`
- 使用 `target/sessions/` （debug）或 `sessions/` （release）目录

## 文件位置

- **开发模式**: `target/sessions/{session_id}.jsonl`
- **发布模式**: `sessions/{session_id}.jsonl`

## 数据格式

JSONL 格式，每行一个 JSON 对象：

```json
{"timestamp":"2025-12-02T18:06:56.260803+00:00","update":{"sessionUpdate":"available_commands_update","availableCommands":[...]}}
{"timestamp":"2025-12-02T18:07:01.123456+00:00","update":{"UserMessageChunk":{"content":{"Text":{"text":"Hello"}}}}}
{"timestamp":"2025-12-02T18:07:02.234567+00:00","update":{"AgentMessageChunk":{"content":{"Text":{"text":"Hi"}}}}}
```

## 工作流程

### 消息保存流程

1. 用户发送消息或 Agent 响应
2. SessionUpdate 事件发布到 SessionUpdateBus
3. PersistenceService 订阅回调被触发
4. 在后台异步保存到 JSONL 文件
5. 错误日志记录（如果失败）

### 历史加载流程

1. 用户点击任务列表中的任务
2. 创建 ConversationPanel 面板
3. 调用 `load_history_for_session()`
4. 在后台异步加载 JSONL 文件
5. 解析每行 JSON 并转换为 SessionUpdate
6. 添加到 rendered_items 列表
7. 更新 next_index
8. 触发重新渲染并滚动到底部

## 技术细节

### 运行时处理

- **问题**: GPUI 使用 smol 运行时，直接在构造函数中使用 `tokio::spawn` 会导致崩溃
- **解决方案**: 在异步上下文中调用 `init_persistence()`，确保在 tokio 运行时中执行
- **调用位置**: `main.rs` 的 `cx.spawn(async move {})` 块中

### 错误处理

- 文件不存在时返回空列表（正常情况）
- JSON 解析失败时记录警告并继续读取其他行
- 目录创建失败时记录错误

### 性能优化

- **异步写入**: 不阻塞 UI 线程
- **追加模式**: 使用 append 模式打开文件，避免重写
- **独立文件**: 每个会话独立文件，避免单文件过大

## 测试指南

详细测试步骤请参阅 `TESTING_PERSISTENCE.md`

## 已知限制

1. 文件格式固定为 JSONL
2. 时间戳为 UTC 时区
3. 无自动清理旧会话文件
4. 非常长的会话可能导致加载时间较长

## 未来改进建议

- [ ] 会话导出功能（Markdown、JSON 等）
- [ ] 会话搜索功能
- [ ] 会话压缩（旧会话自动压缩）
- [ ] 会话统计
- [ ] 自动清理策略
- [ ] 会话备份和恢复
- [ ] 支持增量加载（分页）

## 相关文件

- `src/core/services/persistence_service.rs` - 持久化服务
- `src/core/services/message_service.rs` - 消息服务集成
- `src/core/services/mod.rs` - 服务导出
- `src/panels/conversation_acp/panel.rs` - 面板历史加载
- `src/app/app_state.rs` - 应用状态管理
- `src/main.rs` - 初始化入口
- `Cargo.toml` - 添加了 tokio 的 `fs` 和 `io-util` features
- `TESTING_PERSISTENCE.md` - 测试指南

## 提交信息建议

```
feat: 添加会话消息持久化功能

- 新增 PersistenceService 用于 JSONL 文件的读写
- MessageService 集成自动持久化所有 SessionUpdate
- ConversationPanel 支持自动加载历史记录
- 消息保存到 target/sessions/{session_id}.jsonl
- 支持历史加载、删除和会话列表查询

实现细节:
- 异步文件操作，不阻塞 UI
- 使用 tokio 运行时处理文件 I/O
- JSONL 格式，每行包含时间戳和 SessionUpdate
- 错误处理和日志记录
```

## 验证状态

✅ 编译成功
✅ 程序启动正常
✅ PersistenceService 初始化成功
✅ MessageService 持久化订阅已启用
✅ 会话文件已生成 (`target/sessions/019ae03e-fa96-727e-8c20-796af8fa179a.jsonl`)
✅ 没有运行时崩溃

功能已完全实现并可以正常使用！
