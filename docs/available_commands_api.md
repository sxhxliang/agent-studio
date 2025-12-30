# Available Commands API

当会话创建时,Agent 会发送 `AvailableCommandsUpdate` 消息,告知客户端当前会话支持的命令列表(如斜杠命令)。

## 架构设计

### 数据流

```
Agent Process → GuiClient.session_notification()
  └─→ SessionUpdateBus.publish(AvailableCommandsUpdate)
       └─→ MessageService.init_persistence() subscription
            └─→ AgentService.update_session_commands()
                 └─→ 存储在 AgentSessionInfo.available_commands
```

### 关键组件

1. **AgentSessionInfo**: 存储会话的可用命令列表
2. **AgentService**: 提供更新和查询命令的方法
3. **MessageService**: 监听 `AvailableCommandsUpdate` 事件并自动更新会话信息

## API 使用

### 1. 查询会话的可用命令

#### 方法 A: 通过 MessageService (推荐)

```rust
use crate::app::AppState;

// 如果你知道 agent_name 和 session_id
let message_service = AppState::global(cx).message_service()?;
if let Some(commands) = message_service.get_session_commands(&agent_name, &session_id) {
    for cmd in commands {
        println!("Command: {} - {}", cmd.name, cmd.description.unwrap_or_default());
    }
}

// 或者只用 session_id (自动查找 agent_name)
if let Some(commands) = message_service.get_commands_by_session_id(&session_id) {
    for cmd in commands {
        println!("Command: {} - {}", cmd.name, cmd.description.unwrap_or_default());
    }
}
```

#### 方法 B: 通过 AgentService

```rust
use crate::app::AppState;

let agent_service = AppState::global(cx).agent_service()?;
if let Some(commands) = agent_service.get_session_commands(&agent_name, &session_id) {
    for cmd in commands {
        println!("Command: {} - {}", cmd.name, cmd.description.unwrap_or_default());
    }
}
```

### 2. 监听 AvailableCommandsUpdate 事件

如果你需要在 UI 中实时显示可用命令,可以订阅 SessionUpdateBus:

```rust
use agent_client_protocol::SessionUpdate;
use crate::app::AppState;

let message_service = AppState::global(cx).message_service()?;
let mut rx = message_service.subscribe_session_updates(Some(session_id.clone()));

cx.spawn(async move |cx| {
    while let Some(update) = rx.recv().await {
        if let SessionUpdate::AvailableCommandsUpdate(commands_update) = update {
            // 处理命令更新
            cx.update(|cx| {
                entity.update(cx, |this, cx| {
                    this.available_commands = commands_update.available_commands.clone();
                    cx.notify(); // 触发重新渲染
                });
            });
        }
    }
}).detach();
```

### 3. 从 AgentSessionInfo 获取命令

```rust
let agent_service = AppState::global(cx).agent_service()?;
if let Some(session_info) = agent_service.get_session_info(&agent_name, &session_id) {
    println!("Session {} has {} commands available",
        session_info.session_id,
        session_info.available_commands.len()
    );

    for cmd in &session_info.available_commands {
        println!("  /{}: {}", cmd.name, cmd.description.unwrap_or_default());
    }
}
```

## AvailableCommand 数据结构

根据 Agent Client Protocol (ACP),`AvailableCommand` 包含:

```rust
pub struct AvailableCommand {
    /// 命令名称 (例如: "compact", "init", "review")
    pub name: String,

    /// 命令描述 (可选)
    pub description: Option<String>,

    /// 输入参数定义 (可选)
    pub input: Option<CommandInput>,

    /// 扩展元数据 (可选)
    pub meta: Option<serde_json::Value>,
}
```

## 示例输出

根据你提供的日志,Claude Code Agent 返回的命令列表:

```
/compact - Clear conversation history but keep a summary in context
/init - Initialize a new CLAUDE.md file with codebase documentation
/pr-comments - Get comments from a GitHub pull request
/review - Review a pull request
/security-review - Complete a security review of the pending changes
```

## 日志输出

当命令更新时,你会看到以下日志:

```
[INFO  agentx::core::services::agent_service] Updating available commands for Claude Code:019b3b41-4f62-773f-b729-549eb55905b5 - 5 commands
[DEBUG agentx::core::services::message_service] Received AvailableCommandsUpdate for session 019b3b41-4f62-773f-b729-549eb55905b5: 5 commands
```

## UI 集成建议

### 在会话面板中显示可用命令

```rust
impl ConversationPanel {
    fn render_available_commands(&self, cx: &App) -> impl IntoElement {
        let message_service = AppState::global(cx).message_service()?;
        let commands = message_service.get_commands_by_session_id(&self.session_id)?;

        v_flex()
            .gap_2()
            .child(Label::new("Available Commands").weight(FontWeight::BOLD))
            .children(commands.iter().map(|cmd| {
                h_flex()
                    .gap_2()
                    .child(Label::new(format!("/{}", cmd.name)).text_color(Color::Accent))
                    .child(Label::new(cmd.description.clone().unwrap_or_default()))
            }))
    }
}
```

### 在输入框中添加命令自动完成

你可以使用 `available_commands` 来实现斜杠命令的自动完成功能:

```rust
impl ChatInputBox {
    fn get_command_suggestions(&self, input: &str) -> Vec<AvailableCommand> {
        let message_service = AppState::global(cx).message_service()?;
        let commands = message_service.get_commands_by_session_id(&self.session_id)?;

        if input.starts_with('/') {
            let prefix = &input[1..];
            commands.into_iter()
                .filter(|cmd| cmd.name.starts_with(prefix))
                .collect()
        } else {
            vec![]
        }
    }
}
```

## 注意事项

1. **初始化延迟**: `AvailableCommandsUpdate` 消息通常在会话创建后立即发送,但可能有短暂延迟
2. **命令可变性**: Agent 可能在会话过程中发送新的 `AvailableCommandsUpdate` 来更新命令列表
3. **线程安全**: 所有 API 都是线程安全的,使用 `RwLock` 保护共享状态
4. **自动持久化**: 命令更新会自动保存到 JSONL 会话文件中

## 实现细节

### 数据存储

- **位置**: `AgentSessionInfo.available_commands: Vec<AvailableCommand>`
- **初始值**: 空向量 `Vec::new()`
- **更新时机**: 接收到 `SessionUpdate::AvailableCommandsUpdate` 时
- **访问控制**: 通过 `RwLock<HashMap<String, HashMap<String, AgentSessionInfo>>>` 保护

### 事件订阅

MessageService 在 `init_persistence()` 中自动订阅 SessionUpdateBus:

```rust
// 在 MessageService::init_persistence() 中
session_bus.subscribe(move |event| {
    if let SessionUpdate::AvailableCommandsUpdate(ref commands_update) = update {
        if let Some(agent_name) = agent_svc.get_agent_for_session(&session_id) {
            agent_svc.update_session_commands(
                &agent_name,
                &session_id,
                commands_update.available_commands.clone(),
            );
        }
    }
    // ... 持久化逻辑
});
```

## 相关文件

- `src/core/services/agent_service.rs`: AgentSessionInfo 和命令管理方法
- `src/core/services/message_service.rs`: 事件订阅和查询 API
- `src/panels/conversation/helpers.rs`: SessionUpdate 类型识别
- `src/panels/conversation/panel.rs`: UI 中的 AvailableCommandsUpdate 处理
