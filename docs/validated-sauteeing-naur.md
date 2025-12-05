# 实现计划：完善 SessionUpdateBus 事件总线功能

## 问题总结

**核心问题：** AgentManager 使用 `CliClient` 而非 `GuiClient`，导致会话更新无法发布到事件总线。

**根本原因：**
1. `src/acp_client.rs:217` 创建 CliClient（只打印到控制台）
2. `src/main.rs:44` 未传递 session_bus 参数给 AgentManager
3. `src/conversation_acp.rs` 订阅回调无法触发 GPUI 渲染

**解决方案：**
1. 替换 CliClient → GuiClient（发布到总线）
2. 传递 session_bus 参数
3. 使用 channel + 后台执行器实现实时渲染
4. ChatInput 立即发布用户消息（即时反馈）

---

## 实现方案（4 个阶段）

### 阶段 1：修改 AgentManager 使用 GuiClient

**目标：** 将 session_bus 参数贯穿整个 agent 初始化链，用 GuiClient 替换 CliClient。

**文件：** `src/acp_client.rs`

#### 1.1 更新函数签名（7 处修改）

```rust
// 1. AgentManager::initialize (line 33-36)
pub async fn initialize(
    configs: HashMap<String, AgentProcessConfig>,
    permission_store: Arc<PermissionStore>,
    session_bus: SessionUpdateBusContainer,  // 新增参数
) -> Result<Arc<Self>>

// 2. AgentHandle::spawn (line 74-77)
async fn spawn(
    name: String,
    config: AgentProcessConfig,
    permission_store: Arc<PermissionStore>,
    session_bus: SessionUpdateBusContainer,  // 新增参数
) -> Result<Self>

// 3. run_agent_worker (line 151-156)
fn run_agent_worker(
    agent_name: String,
    config: AgentProcessConfig,
    permission_store: Arc<PermissionStore>,
    session_bus: SessionUpdateBusContainer,  // 新增参数
    command_rx: mpsc::Receiver<AgentCommand>,
    ready_tx: oneshot::Sender<Result<()>>,
) -> Result<()>

// 4. agent_event_loop (line 177-183)
async fn agent_event_loop(
    agent_name: String,
    config: AgentProcessConfig,
    permission_store: Arc<PermissionStore>,
    session_bus: SessionUpdateBusContainer,  // 新增参数
    mut command_rx: mpsc::Receiver<AgentCommand>,
    ready_tx: oneshot::Sender<Result<()>>,
) -> Result<()>
```

#### 1.2 传递参数到调用链

```rust
// Line 42: 在 AgentManager::initialize 中
match AgentHandle::spawn(name.clone(), cfg, permission_store.clone(), session_bus.clone()).await {

// Line 88: 在 AgentHandle::spawn 中
run_agent_worker(worker_name, config, permission_store, session_bus, receiver, ready_tx)

// Line 166: 在 run_agent_worker 中
agent_event_loop(agent_name, config, permission_store, session_bus, command_rx, ready_tx)
```

#### 1.3 替换 CliClient 为 GuiClient

```rust
// Line 217: 核心修改
// 删除:
let client = CliClient::new(agent_name.clone(), permission_store);

// 替换为:
use crate::gui_client::GuiClient;
let client = GuiClient::new(agent_name.clone(), permission_store, session_bus);
```

#### 1.4 添加导入

```rust
// 文件顶部添加
use crate::session_bus::SessionUpdateBusContainer;
```

---

### 阶段 2：从 main.rs 传递 session_bus

**目标：** 确保 AgentManager 初始化时接收到 session_bus 参数。

**文件：** `src/main.rs`

#### 2.1 在 spawn 前获取 session_bus

```rust
// 在 line 13 (agentx::init(cx)) 之后添加
let session_bus = agentx::AppState::global(cx).session_bus.clone();

// 修改 cx.spawn 调用
cx.spawn(move |cx| {
    let session_bus = session_bus.clone();  // clone 到 async 闭包中
    async move {
        // ... 现有代码 ...
    }
})
```

#### 2.2 传递给 AgentManager::initialize

```rust
// Line 44: 修改调用
match AgentManager::initialize(
    config.agent_servers.clone(),
    permission_store.clone(),
    session_bus.clone(),  // 新增参数
).await
```

---

### 阶段 3：使用 Channel 实现实时渲染触发

**目标：** 使用 mpsc channel + GPUI 后台执行器实现实时更新，无延迟。

**文件：** `src/conversation_acp.rs`

**方案优势：**
- ✅ 实时更新，零延迟
- ✅ 清晰的跨线程通信模式
- ✅ 符合现代 UI 响应性要求

#### 3.1 移除旧的 pending_updates 机制

```rust
// Line 461: 移除 pending_updates 字段
pub struct ConversationPanel {
    focus_handle: FocusHandle,
    rendered_items: Vec<RenderedItem>,
    next_index: usize,
    // 删除: pending_updates: Arc<std::sync::Mutex<Vec<SessionUpdate>>>,
}

// Line 486: 移除初始化
// 删除: pending_updates: Arc::new(std::sync::Mutex::new(Vec::new())),

// Line 519-531: 删除整个 process_pending_updates 方法

// Line 704: 移除调用
// 删除: self.process_pending_updates(cx);
```

#### 3.2 重写订阅逻辑

```rust
// Line 493: 完全重写 subscribe_to_updates
pub fn subscribe_to_updates(entity: &Entity<Self>, cx: &mut App) {
    let weak_entity = entity.downgrade();
    let session_bus = AppState::global(cx).session_bus.clone();

    // 创建 unbounded channel 用于跨线程通信
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<SessionUpdate>();

    // 订阅总线，回调中将更新发送到 channel
    session_bus.subscribe(move |event| {
        // 这个回调在 agent I/O 线程中执行
        let _ = tx.send((*event.update).clone());
        log::info!("Session update sent to channel: session_id={}", event.session_id);
    });

    // 启动后台任务，从 channel 读取并更新 entity
    cx.spawn(|mut cx| async move {
        while let Some(update) = rx.recv().await {
            let weak = weak_entity.clone();
            let _ = cx.update(|cx| {
                if let Some(entity) = weak.upgrade() {
                    entity.update(cx, |this, cx| {
                        let index = this.next_index;
                        this.next_index += 1;
                        Self::add_update_to_list(&mut this.rendered_items, update, index, cx);
                        cx.notify();  // 立即触发重新渲染
                        log::info!("Rendered session update, total items: {}", this.rendered_items.len());
                    });
                }
            });
        }
    }).detach();

    log::info!("Subscribed to session bus with channel-based updates");
}
```

**关键点：**
- 使用 `tokio::sync::mpsc::unbounded_channel` 在线程间传递更新
- 订阅回调（agent I/O 线程）→ channel → 后台任务（GPUI 线程）
- `cx.spawn()` 创建的异步任务在 GPUI 上下文中执行
- `cx.update()` 安全地跨线程更新 entity 并触发渲染

---

---

## 实现顺序与依赖

### 步骤 1：修改 acp_client.rs（核心修复）
- **依赖：** 无
- **风险：** 低（编译期类型检查）
- **验证：** 代码编译通过，无类型错误

### 步骤 2：修改 main.rs（参数传递）
- **依赖：** 步骤 1 完成
- **风险：** 低（简单参数传递）
- **验证：** 应用启动正常，agents 初始化成功

### 步骤 3：修改 conversation_acp.rs（实时渲染）
- **依赖：** 步骤 1-2 完成
- **风险：** 中等（新的 channel 机制）
- **验证：**
  - 从 ChatInput 发送消息
  - 控制台显示 "Session update sent to channel" 日志
  - 控制台显示 "Rendered session update" 日志
  - 消息在 ConversationPanel 中实时显示

### 步骤 4：修改 chat_input.rs（用户消息即时反馈）
- **依赖：** 步骤 1-3 完成
- **风险：** 低（增量功能）
- **验证：**
  - 用户输入消息并点击发送
  - 消息立即出现在对话面板（无需等待 agent 响应）
  - Agent 响应也正常显示

---

## 关键文件与修改点

### 必须修改的文件（4 个）

| 文件 | 修改行数范围 | 关键修改 |
|------|-------------|----------|
| `src/acp_client.rs` | 33-217 | 7 个函数签名 + GuiClient 替换 + imports |
| `src/main.rs` | 13-44 | 获取 session_bus + 传递参数 |
| `src/conversation_acp.rs` | 461-704 | 移除 pending_updates + 重写订阅 + 删除轮询 |
| `src/chat_input.rs` | ~308 | 发布用户消息到总线 |

### 参考文件（无需修改）

| 文件 | 用途 |
|------|------|
| `src/gui_client.rs` | 验证 session_notification 发布逻辑 |
| `src/session_bus.rs` | 了解 subscribe/publish API |

---

## 测试与验证

### 单元验证

**步骤 1-2 验证（编译和初始化）：**
```bash
cargo check --example agentx  # 编译检查
cargo run --example agentx     # 启动应用
```
- ✅ 代码编译无错误
- ✅ 应用启动，agents 初始化成功
- ✅ 控制台无错误信息

**步骤 3 验证（实时渲染）：**
1. 从 ChatInput 发送消息 "Hello"
2. 观察控制台日志：
   ```
   [gui_client] Publishing to session bus: session_id=xxx
   [conversation_acp] Session update sent to channel: session_id=xxx
   [conversation_acp] Rendered session update, total items: 1
   ```
3. 对话面板实时显示 agent 响应（无延迟）

**步骤 4 验证（用户消息即时反馈）：**
1. 在 ChatInput 输入 "Test message"
2. 点击发送
3. 消息立即出现在对话面板（即使 agent 还未响应）
4. Agent 响应到达后，也正常显示

### 集成验证

**功能测试：**
- [ ] 发送 10 条快速连续消息，全部按序显示
- [ ] 不同消息类型正确渲染（UserMessage, AgentMessage, ToolCall, Plan）
- [ ] 切换不同 agent，消息正确路由到对应 session
- [ ] 长文本消息正常换行和滚动

**性能测试：**
- [ ] 发送 100 条消息，UI 保持流畅
- [ ] 使用任务管理器检查内存无异常增长
- [ ] Channel 缓冲区无积压（检查日志时间戳）

**边界情况：**
- [ ] Agent 进程崩溃时，UI 不崩溃，显示错误信息
- [ ] 快速点击发送按钮，所有消息都能发送和显示
- [ ] 关闭应用再打开，新会话正常工作

---

### 阶段 4：ChatInput 发布用户消息到总线

**目标：** 用户发送消息后立即在对话面板显示，提供即时视觉反馈。

**文件：** `src/chat_input.rs`

#### 4.1 在 send_message 中发布用户消息

```rust
// Line 308: 在调用 agent_handle.prompt() 之前添加

// 立即发布用户消息到总线，实现即时 UI 反馈
use agent_client_protocol_schema as schema;
use std::sync::Arc;

let user_event = crate::session_bus::SessionUpdateEvent {
    session_id: session_id.clone(),
    update: Arc::new(schema::SessionUpdate::UserMessageChunk(
        schema::ContentChunk {
            chunk_id: format!("local-{}", uuid::Uuid::new_v4()),  // 唯一 ID
            content: schema::ContentBlock::Text(schema::TextContent {
                text: input_text.clone(),
            }),
        }
    )),
};

// 从全局状态获取 session_bus 并发布
AppState::global(cx).session_bus.publish(user_event);
log::info!("Published user message to session bus: {}", session_id);

// 然后继续现有的 agent_handle.prompt() 调用
```

#### 4.2 处理可能的重复消息

**场景：** Agent 可能通过 `UserMessageChunk` 回显用户消息，导致重复显示。

**解决方案：**
1. 使用唯一的 `chunk_id`（如 `local-<uuid>`）标识本地发布的消息
2. Agent 回显的消息会有不同的 `chunk_id`
3. 如果 Agent 确实回显，会显示两条消息（一条本地，一条来自 agent）

**备选方案（如需去重）：**
- 在 ConversationPanel 中维护 `seen_chunks: HashSet<String>`
- 根据 `chunk_id` 去重

**当前建议：** 先实现基本功能，观察 Agent 是否真的会回显。如果不回显，则无需去重逻辑。
