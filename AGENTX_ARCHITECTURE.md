# AgentX 项目架构分析与代码审核功能设计

## 1. 项目概述

AgentX 是一个现代化的桌面 AI 代理工作室，基于 Rust 和 GPUI 框架构建，提供了一个统一的界面来与多个 AI 代理交互、编辑代码、管理任务等。

### 核心特性
- 🤖 **多代理支持** - 通过 Agent Client Protocol (ACP) 同时连接和聊天
- 💬 **实时对话** - 支持思考块和工具调用的流式响应
- 📝 **内置代码编辑器** - 支持 LSP 的编辑器，带有语法高亮和自动完成
- 🖥️ **集成终端** - 无需离开应用即可执行命令
- 🎨 **可自定义的停靠系统** - 拖放面板创建完美的工作区
- 🌍 **国际化** - 支持多种语言（英语、简体中文）
- 🎭 **主题支持** - 明暗主题，可自定义颜色
- 📊 **会话管理** - 跨多个会话组织对话
- 🔧 **工具调用查看器** - 详细检查代理工具执行
- 💾 **自动保存** - 自动会话持久化，永不丢失工作
- ⚡ **GPU 加速** - 由 GPUI 框架提供的快速 UI

## 2. 架构设计

AgentX 采用分层架构，清晰分离关注点：

```
┌─────────────────────────────────────────┐
│  UI Layer (panels/, components/)        │  ← GPUI 渲染，用户交互
├─────────────────────────────────────────┤
│  Event Bus (core/event_bus/)            │  ← 跨线程更新的发布/订阅
├─────────────────────────────────────────┤
│  Service Layer (core/services/)         │  ← 业务逻辑
├─────────────────────────────────────────┤
│  Agent Client (core/agent/)             │  ← ACP 协议，进程管理
└─────────────────────────────────────────┘
```

### 2.1 工作区结构

AgentX 使用工作区结构将关注点分离到可重用的 crate 中：

- **agentx-types** (`crates/agentx-types/`)：所有 crate 中使用的共享类型定义和数据结构
- **agentx-event-bus** (`crates/agentx-event-bus/`)：线程安全的发布/订阅通信的事件总线实现
- **agentx-agent** (`crates/agentx-agent/`)：Agent 客户端和 ACP 协议实现，进程管理
- **agentx-services** (`crates/agentx-services/`)：业务逻辑服务（AgentService, MessageService, PersistenceService 等）
- **agentx-acp-ui** (`crates/agentx-acp-ui/`)：用于渲染代理消息、工具调用和流的 ACP 特定 UI 组件
- **git-worktree-manager** (`crates/git-worktree-manager/`)：Git 工作树管理工具

### 2.2 目录结构与模块划分

| 目录 | 模块职责 | 主要组件 |
|------|---------|----------|
| **src/app/** | 应用级功能 | 菜单、动作、主题、窗口装饰、系统托盘 |
| **src/components/** | 可重用 UI 组件 | 代理选择器、聊天输入框、命令建议弹出框等 |
| **src/core/agent/** | 代理客户端实现 | ACP 协议、进程管理 |
| **src/core/event_bus/** | 事件总线 | 跨线程通信、事件发布/订阅 |
| **src/core/services/** | 业务逻辑服务 | AgentService、MessageService、PersistenceService 等 |
| **src/core/updater/** | 应用更新 | 检查更新、下载更新 |
| **src/panels/** | 可停靠面板 | 对话面板、代码编辑器、任务面板、设置面板、终端面板等 |
| **src/utils/** | 工具函数 | 文件操作、剪贴板、时间、工具调用 |
| **src/workspace/** | 工作区管理 | 工作区模型/动作和面板/服务之间的连接 |
| **src/schemas/** | 数据模式 | 序列化的数据模式 |
| **assets/** | 资源文件 | 图标和 UI 使用的徽标 |
| **themes/** | 主题文件 | 主题 JSON 文件和模式 |
| **locales/** | 国际化文件 | i18n 字符串（*.yml） |

### 2.3 核心架构模式

#### 2.3.1 事件总线系统（跨线程通信）

事件总线实现了代理线程和 UI 线程之间的线程安全发布/订阅：

**事件总线** (`src/core/event_bus/`)：
- `SessionUpdateBus`：代理消息、工具调用、思考更新
- `PermissionBus`：来自代理的权限请求
- `WorkspaceBus`：工作区状态更改
- `CodeSelectionBus`：编辑器集成的代码选择事件
- `AgentConfigBus`：代理配置更改

**模式**（代理线程 → UI 线程）：
```rust
// 1. 在 UI 组件中订阅（在 GPUI 主线程上运行）
let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
session_bus.subscribe(move |event| {
    let _ = tx.send((*event.update).clone());
});

cx.spawn(|mut cx| async move {
    while let Some(update) = rx.recv().await {
        cx.update(|cx| {
            entity.update(cx, |this, cx| {
                // 更新 UI 状态
                cx.notify();  // 触发重新渲染
            });
        });
    }
}).detach();

// 2. 从任何线程发布（代理线程、服务等）
session_bus.publish(SessionUpdateEvent {
    session_id: session_id.clone(),
    update: Arc::new(SessionUpdate::AgentMessage(...)),
});
```

**关键特性**：
- **批处理**：`BatchedEventCollector` 对快速事件进行分组
- **防抖**：`Debouncer` 防止过度更新
- **过滤**：订阅特定会话或所有会话
- **指标**：`EventBusStats` 跟踪订阅计数和事件吞吐量

#### 2.3.2 服务层模式

所有业务逻辑都位于服务 (`src/core/services/`) 中，通过全局 `AppState` 访问：

**服务**：
- `AgentService`：管理代理生命周期和会话（聚合根）
- `MessageService`：处理消息发送和事件总线集成
- `PersistenceService`：将会话历史保存/加载到 JSONL 文件
- `WorkspaceService`：管理工作区状态和面板可见性
- `AgentConfigService`：具有热重载的动态代理配置
- `AiService`：AI 驱动的功能（代码注释等）

**使用模式**：
```rust
let message_service = AppState::global(cx).message_service()?;

// 发送消息（异步操作）
cx.spawn(async move |_this, _cx| {
    match message_service.send_user_message(&agent_name, message).await {
        Ok(session_id) => log::info!("Message sent to {}", session_id),
        Err(e) => log::error!("Failed: {}", e),
    }
}).detach();

// 订阅会话更新并过滤
let mut rx = message_service.subscribe_session_updates(Some(session_id));
cx.spawn(async move |cx| {
    while let Some(update) = rx.recv().await {
        // 处理更新
    }
}).detach();
```

#### 2.3.3 DockPanel 系统

所有面板都实现 `DockPanel` trait 以获得一致的停靠行为：

```rust
pub trait DockPanel: 'static + Sized {
    fn title() -> &'static str;
    fn description() -> &'static str;
    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render>;

    // 可选自定义
    fn closable() -> bool { true }
    fn zoomable() -> bool { true }
    fn paddings() -> Pixels { px(16.) }
}
```

**面板** (`src/panels/`)：
- `ConversationPanel`：与 ACP 代理的聊天界面
- `CodeEditorPanel`：支持 LSP 的代码编辑器
- `TaskPanel`：任务/待办事项管理
- `SessionManagerPanel`：多会话切换
- `SettingsPanel`：应用设置
- `TerminalPanel`：嵌入式终端
- `ToolCallDetailPanel`：工具调用详细查看器
- `WelcomePanel`：欢迎屏幕

#### 2.3.4 Entity 生命周期

**GPUI Entity 规则**：在 `render()` 中创建的实体在方法返回后会被删除。

## 3. 核心子系统

### 3.1 代理管理

**流程**：`main.rs` → `AgentManager::initialize()` → 生成代理进程 → `GuiClient` 回调 → 事件总线

**代理配置** (`config.json`)：
- 位于用户数据目录（Windows: `%APPDATA%\agentx\config.json`）
- 通过 `ConfigWatcher` 支持热重载
- 命令行覆盖：`agentx --config /path/to/config.json`

**会话生命周期**：
```rust
let agent_service = AppState::global(cx).agent_service()?;

// 获取或创建会话（重用现有）
let session_id = agent_service.get_or_create_session(&agent_name).await?;

// 发送消息
let message_service = AppState::global(cx).message_service()?;
message_service.send_user_message(&agent_name, message).await?;

// 关闭会话
agent_service.close_session(&agent_name).await?;
```

### 3.2 布局持久化

**位置**：
- 调试：`target/docks-agentx.json`
- 发布：`docks-agentx.json`

**特性**：
- 自动保存布局（防抖 10 秒）
- 在应用退出时保存
- 包括面板位置、大小、活动标签
- 版本跟踪以进行迁移

### 3.3 会话持久化

**位置**：`target/sessions/{session_id}.jsonl`（调试）或 `sessions/`（发布）

**格式**（每行一个 JSON）：
```jsonl
{"timestamp":"2025-12-10T10:30:45Z","update":{"UserMessage":{"content":"..."}}}
{"timestamp":"2025-12-10T10:30:47Z","update":{"AgentMessage":{"content":"..."}}}
```

**自动**：`PersistenceService` 订阅会话总线并实时保存。

### 3.4 更新系统

**自动更新检查** (`src/core/updater/`)：
```rust
let manager = UpdateManager::new()?;

match manager.check_for_updates().await {
    UpdateCheckResult::UpdateAvailable(info) => {
        // 下载更新
        let path = manager.download_update(&info, Some(progress_callback)).await?;
    }
    UpdateCheckResult::UpToDate => {},
    UpdateCheckResult::Error(e) => {},
}
```

## 4. 代码审核功能设计

### 4.1 功能概述

代码审核功能将允许用户通过 AI 代理对代码进行分析、审查和改进建议。该功能将集成到现有的 AgentX 架构中，利用现有的代理系统和 UI 组件。

### 4.2 实现思路

#### 4.2.1 架构设计

1. **新增面板**：创建 `CodeReviewPanel`，实现 `DockPanel` trait
2. **服务扩展**：扩展 `AiService`，添加代码审核相关方法
3. **事件总线**：利用现有的 `CodeSelectionBus` 和 `SessionUpdateBus`
4. **代理集成**：利用现有的 ACP 代理系统进行代码分析

#### 4.2.2 目录结构

```
src/panels/code_review/
├── mod.rs           # 模块导出
├── panel.rs         # 主面板实现
├── types.rs         # 面板特定类型
├── components.rs    # UI 子组件
└── helpers.rs       # 工具函数
```

#### 4.2.3 核心功能

1. **代码选择**：从代码编辑器中选择代码进行审核
2. **审核配置**：允许用户设置审核参数（如语言、框架、关注点）
3. **AI 分析**：利用 ACP 代理分析代码并生成审核报告
4. **结果展示**：以结构化方式展示审核结果，包括问题、建议和改进
5. **代码修改**：允许用户应用建议的修改
6. **审核历史**：保存审核历史，以便后续参考

#### 4.2.4 实现步骤

1. **创建 CodeReviewPanel**
   - 实现 `DockPanel` trait
   - 设计 UI 布局，包括代码选择区域、审核配置、结果展示
   - 集成到应用的默认布局中

2. **扩展 AiService**
   - 添加 `analyze_code` 方法
   - 实现代码审核逻辑，包括与 ACP 代理的交互
   - 处理审核结果的解析和结构化

3. **集成事件总线**
   - 监听 `CodeSelectionBus` 事件，获取用户选择的代码
   - 发布审核结果到 `SessionUpdateBus`，更新 UI

4. **实现审核流程**
   - 代码选择 → 配置审核参数 → 发送到 AI 代理 → 接收并解析结果 → 展示审核报告

5. **添加设置选项**
   - 在 `SettingsPanel` 中添加代码审核相关设置
   - 允许用户配置默认审核参数

#### 4.2.5 技术实现细节

1. **代码选择集成**：
   - 利用现有的 `CodeSelectionBus` 事件
   - 添加代码选择监听逻辑

2. **审核请求构建**：
   - 构建结构化的审核请求，包括代码、语言、框架、关注点
   - 利用 ACP 协议发送到代理

3. **结果解析**：
   - 解析代理返回的审核结果
   - 结构化处理为审核报告

4. **UI 展示**：
   - 使用 GPUI 组件展示审核结果
   - 支持结果的展开/折叠
   - 提供代码修改建议的应用功能

5. **历史记录**：
   - 保存审核历史到会话文件
   - 提供历史审核记录的查看功能

### 4.3 集成点

1. **与代码编辑器的集成**：
   - 添加右键菜单选项 "审核代码"
   - 支持从代码编辑器选择代码并直接进行审核

2. **与会话系统的集成**：
   - 将审核结果保存到会话历史
   - 支持在会话中查看历史审核记录

3. **与设置系统的集成**：
   - 在设置面板中添加代码审核相关设置
   - 支持配置默认审核参数

4. **与代理系统的集成**：
   - 允许用户选择用于代码审核的代理
   - 支持针对不同语言和框架选择不同的代理

## 5. 技术栈与依赖

| 技术/依赖 | 用途 | 来源 |
|----------|------|------|
| Rust 1.83+ | 内存安全的系统编程语言 | [rust-lang.org](https://www.rust-lang.org/) |
| GPUI | GPU 加速的 UI 框架 | [gpui.rs](https://www.gpui.rs/) |
| gpui-component | 丰富的 UI 组件库 | [github.com/longbridge/gpui-component](https://github.com/longbridge/gpui-component) |
| Agent Client Protocol | 代理通信的标准协议 | [agentclientprotocol.com](https://agentclientprotocol.com/) |
| Tokio | 异步运行时 | [tokio.rs](https://tokio.rs/) |
| Tree-sitter | 语法高亮 | [tree-sitter.github.io](https://tree-sitter.github.io/) |

## 6. 结论

AgentX 是一个设计良好的 AI 代理工作室应用，采用分层架构和清晰的模块划分，为用户提供了一个统一的界面来与多个 AI 代理交互。通过扩展现有架构，可以相对容易地添加代码审核功能，利用现有的代理系统和 UI 组件，为用户提供强大的代码分析和改进能力。

代码审核功能的实现将进一步增强 AgentX 的实用性，使开发人员能够更有效地利用 AI 代理来提高代码质量和开发效率。

## 7. 参考资料

- [AgentX 项目仓库](https://github.com/sxhxliang/agent-studio)
- [Agent Client Protocol](https://agentclientprotocol.com/)
- [GPUI 框架](https://www.gpui.rs/)
- [Rust 编程语言](https://www.rust-lang.org/)
- [Tokio 异步运行时](https://tokio.rs/)
- [Tree-sitter](https://tree-sitter.github.io/)