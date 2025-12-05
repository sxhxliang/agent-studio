# Bug Fix: 会话面板没有显示消息

## 🐛 问题描述

**现象**: 从 Welcome Panel 创建任务后，ConversationPanel 不显示任何消息（用户消息和 Agent 响应都看不到）。

**日志分析**:
```
17:05:05.772Z - 发布用户消息到 session bus
17:05:08.190Z - Agent 响应发布到 bus
17:05:08.198Z - ConversationPanel 创建并订阅 ← 太晚了！
17:05:08.222Z - 后台任务启动，但收不到任何消息
```

## 🔍 根本原因

**消息发送和面板订阅的时序问题**：

在 `workspace/actions.rs` 的 `on_action_create_task_from_welcome` 方法中：

```rust
// ❌ 错误的顺序
// Step 1: 发送消息（包括发布到 event bus）
let session_id = message_service.send_user_message(&agent_name, task_input).await?;

// Step 2: 创建面板（订阅 session）
let conversation_panel = DockPanelContainer::panel_for_session(session_id, window, cx);
```

**问题**: 面板在消息发布**之后**才创建和订阅，所以错过了所有消息（用户消息和 Agent 响应）。

## ✅ 解决方案

### 1. 添加新的 MessageService 方法

**文件**: `src/core/services/message_service.rs`

添加 `send_message_to_session` 方法，用于向已存在的 session 发送消息：

```rust
/// Send a user message to an existing session
///
/// This method performs the following steps:
/// 1. Publish the user message to the event bus (immediate UI feedback)
/// 2. Send the prompt to the agent
///
/// Use this when you already have a session ID and want to ensure
/// the UI panel has subscribed before the message is sent.
pub async fn send_message_to_session(
    &self,
    agent_name: &str,
    session_id: &str,
    message: String,
) -> Result<()> {
    // 1. Publish user message to event bus (immediate UI feedback)
    self.publish_user_message(session_id, &message);

    // 2. Send prompt to agent
    self.agent_service
        .send_prompt(agent_name, session_id, vec![message])
        .await
        .map_err(|e| anyhow!("Failed to send message: {}", e))?;

    Ok(())
}
```

### 2. 修改 workspace/actions.rs 执行顺序

**文件**: `src/workspace/actions.rs`

调整 `on_action_create_task_from_welcome` 的执行顺序：

```rust
// ✅ 正确的顺序
cx.spawn_in(window, async move |_this, window| {
    // Step 1: Get or create session (不发送消息)
    let session_id = agent_service.get_or_create_session(&agent_name).await?;

    // Step 2: Create panel (面板订阅 session)
    _ = window.update(move |window, cx| {
        let conversation_panel =
            DockPanelContainer::panel_for_session(session_id.clone(), window, cx);
        // ... dock setup
    });

    // Step 3: Send message to session (面板已订阅，能收到消息)
    message_service.send_message_to_session(&agent_name, &session_id, task_input).await?;
}).detach();
```

### 3. 更新文档

**文件**: `CLAUDE.md`

添加了新的使用示例，说明何时使用 `send_message_to_session`:

```rust
// Recommended pattern for creating panels:
// 1. Get or create session
let agent_service = AppState::global(cx).agent_service().unwrap();
let session_id = agent_service.get_or_create_session(&agent_name).await?;

// 2. Create panel (panel subscribes to session)
let conversation_panel = DockPanelContainer::panel_for_session(session_id.clone(), window, cx);

// 3. Send message to session (panel will receive it)
let message_service = AppState::global(cx).message_service().unwrap();
message_service.send_message_to_session(&agent_name, &session_id, message).await?;
```

## 📊 修改详情

### 新增代码

| 文件 | 新增内容 | 行数 |
|-----|---------|------|
| `src/core/services/message_service.rs` | `send_message_to_session` 方法 | +24 行 |

### 修改代码

| 文件 | 修改内容 | 变化 |
|-----|---------|------|
| `src/workspace/actions.rs` | 调整执行顺序，分离 session 创建和消息发送 | ~95 行（重构） |
| `CLAUDE.md` | 添加 `send_message_to_session` 使用示例 | +13 行 |

## 🎯 关键改进

### Before - 问题流程

```
Timeline:
  T1: send_user_message()
      ├─ get_or_create_session()
      ├─ publish_user_message() ← 发布消息
      └─ send_prompt()           ← Agent 开始处理

  T2: Agent 响应
      └─ publish to event bus    ← Agent 响应发布

  T3: Create ConversationPanel
      └─ subscribe_to_updates()  ← 订阅（太晚了！）

结果: 面板错过了 T1 和 T2 的所有消息
```

### After - 修复流程

```
Timeline:
  T1: get_or_create_session()    ← 只获取 session_id

  T2: Create ConversationPanel
      └─ subscribe_to_updates()  ← 订阅（已准备好）

  T3: send_message_to_session()
      ├─ publish_user_message()  ← 面板收到！
      └─ send_prompt()           ← Agent 开始处理

  T4: Agent 响应
      └─ publish to event bus    ← 面板收到！

结果: 面板接收到所有消息
```

## 🔒 为什么顺序很重要

### Event Bus 的工作原理

Event bus 使用发布-订阅模式：

1. **订阅 (subscribe)**: 注册一个回调，等待事件
2. **发布 (publish)**: 触发所有已注册的回调

**关键**: 只有**已经订阅**的回调才能接收到事件！

### 之前的问题

```rust
// ❌ 消息已发布
publish_user_message(session_id, message);  // T1: 发布事件

// ❌ 订阅在发布之后（错过了事件）
session_bus.subscribe(|event| { ... });     // T2: 订阅（太晚）
```

### 修复后

```rust
// ✅ 先订阅
session_bus.subscribe(|event| { ... });     // T1: 订阅（准备好）

// ✅ 后发布（订阅者能收到）
publish_user_message(session_id, message);  // T2: 发布事件（被接收）
```

## ⚠️ 重要提示

### 何时使用 send_user_message

使用 `send_user_message` 当：
- 你不需要立即显示面板
- 你只需要发送消息并获取 session_id
- 例如：ChatInputPanel（面板已经存在并已订阅）

### 何时使用 send_message_to_session

使用 `send_message_to_session` 当：
- 你需要**先创建面板，再发送消息**
- 你需要确保面板能接收到消息
- 例如：CreateTaskFromWelcome（创建新面板）

### 推荐模式

```rust
// 创建新面板时的标准流程
async fn create_panel_and_send_message() {
    // 1. Get session
    let session_id = agent_service.get_or_create_session(&agent_name).await?;

    // 2. Create panel (subscribes)
    let panel = DockPanelContainer::panel_for_session(session_id, window, cx);

    // 3. Send message (panel receives)
    message_service.send_message_to_session(&agent_name, &session_id, message).await?;
}
```

## ✅ 验证

### 编译检查

```bash
$ cargo build
✅ Finished `dev` profile in 7.48s
⚠️  19 warnings (仅未使用代码，无错误)
```

### 预期日志（修复后）

```
T1: Got session xxx for agent Iflow
T2: Creating ConversationPanel for session: xxx
T3: Subscribed to session updates via MessageService for: xxx
T4: Starting background task for session: xxx
T5: Published user message to session bus: xxx
T6: Background task received update (UserMessageChunk)  ← 面板收到用户消息
T7: Agent response published
T8: Background task received update (AgentMessageChunk) ← 面板收到 Agent 响应
```

## 📚 相关文档

- `REFACTORING_STAGE4_SUMMARY.md` - Stage 4 服务层重构总结
- `CLAUDE.md` - 项目文档（Service Layer Usage 章节）
- `src/core/services/message_service.rs` - MessageService 实现

## 🎓 经验教训

1. **Event Bus 时序至关重要**: 先订阅，后发布
2. **异步操作需要仔细编排**: 确保依赖关系正确
3. **UI 组件生命周期**: 面板创建时立即订阅，确保不错过消息
4. **API 设计**: 提供不同的方法来处理不同的使用场景
5. **日志调试**: 添加时间戳日志来诊断时序问题

---

**修复日期**: 2025-12-02
**Bug 发现者**: User
**修复者**: Claude Code
