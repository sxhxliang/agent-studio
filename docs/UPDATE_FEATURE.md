# 软件版本检测和自动更新功能

## 功能概述

本功能为 AgentX 应用添加了完整的版本检测和自动更新系统，允许用户在设置面板中查看当前版本、检查更新和配置自动更新选项。

## 架构设计

### 模块结构

```
src/core/updater/
├── mod.rs           # 导出和 UpdateManager
├── version.rs       # 版本解析和比较
├── checker.rs       # 更新检查器
└── downloader.rs    # 更新下载器
```

### 核心组件

#### 1. Version (版本管理)

- **功能**: 版本号解析、比较和验证
- **支持格式**: "1.2.3" 或 "v1.2.3"
- **比较逻辑**: 遵循语义化版本规范 (major.minor.patch)

```rust
let current = Version::current(); // 从 Cargo.toml 读取
let latest = Version::parse("1.0.0")?;
if latest.is_newer_than(&current) {
    println!("有新版本可用！");
}
```

#### 2. UpdateChecker (更新检查器)

- **功能**: 从远程 API 检查可用更新
- **API 端点**: GitHub Releases API (可配置)
- **超时设置**: 默认 10 秒 (可配置)
- **返回结果**:
  - `NoUpdate`: 已是最新版本
  - `UpdateAvailable(UpdateInfo)`: 有可用更新
  - `Error(String)`: 检查失败

```rust
let checker = UpdateChecker::new();
match checker.check_for_updates().await {
    UpdateCheckResult::UpdateAvailable(info) => {
        println!("新版本: {}", info.version);
        println!("发布说明: {}", info.release_notes);
    }
    _ => {}
}
```

#### 3. UpdateDownloader (更新下载器)

- **功能**: 下载更新文件到本地
- **下载目录**: 系统临时目录 `/tmp/agentx_updates`
- **进度回调**: 支持下载进度监控
- **特性**:
  - 自动创建下载目录
  - 支持清理旧下载
  - 从 URL 自动提取文件名

#### 4. UpdateManager (更新管理器)

- **功能**: 协调版本检查、下载和安装
- **方法**:
  - `check_for_updates()`: 检查可用更新
  - `download_update()`: 下载更新文件
  - `current_version()`: 获取当前版本

## UI 集成

### 设置面板增强

在 `src/panels/settings_panel.rs` 中的 "Software Update" 页面添加了以下 UI 元素：

#### 1. 版本信息区域

显示：
- 当前版本号 (从 Cargo.toml 读取)
- 更新状态（空闲、检查中、有更新、无更新、错误）
- 状态图标和颜色提示

#### 2. 检查更新按钮

- 按钮标签: "Check Now"
- 图标: LoaderCircle
- 点击后异步检查更新
- 实时更新状态显示

#### 3. 更新配置选项

新增配置项：

| 配置项 | 类型 | 默认值 | 说明 |
|--------|------|--------|------|
| Auto Check on Startup | Switch | true | 启动时自动检查更新 |
| Enable Notifications | Switch | true | 启用更新通知 |
| Auto Update | Switch | true | 自动下载并安装更新 |
| Check Frequency (days) | Number | 7 | 自动检查频率（天） |

### 更新状态显示

根据不同状态显示不同的 UI：

```
空闲状态: 提示用户点击检查更新
检查中:   显示加载图标和"正在检查更新..."
无更新:   显示勾选图标和"您已是最新版本！"
有更新:   显示下载图标、版本号和发布说明
错误:     显示错误图标和错误信息（红色）
```

## 使用方法

### 1. 在设置面板中检查更新

1. 启动应用程序
2. 打开设置面板 (Settings)
3. 切换到 "Software Update" 标签
4. 点击 "Check Now" 按钮
5. 等待检查完成，查看更新状态

### 2. 配置自动更新

在 "Update Settings" 组中配置以下选项：
- 启用/禁用启动时自动检查
- 启用/禁用更新通知
- 启用/禁用自动更新
- 设置检查频率（1-30 天）

### 3. 编程方式使用

```rust
use crate::core::updater::{UpdateManager, UpdateCheckResult};

let update_manager = UpdateManager::new()?;

// 检查更新
match update_manager.check_for_updates().await {
    UpdateCheckResult::UpdateAvailable(info) => {
        println!("发现新版本: {}", info.version);

        // 下载更新
        let file_path = update_manager
            .download_update(&info, None)
            .await?;

        println!("下载完成: {:?}", file_path);
    }
    UpdateCheckResult::NoUpdate => {
        println!("已是最新版本");
    }
    UpdateCheckResult::Error(e) => {
        eprintln!("检查失败: {}", e);
    }
}
```

## 实现细节

### 异步任务处理

使用 GPUI 的 `cx.spawn()` 在后台异步检查更新，避免阻塞 UI：

```rust
cx.spawn(async move |_this, mut cx| {
    let result = update_manager.check_for_updates().await;

    let _ = cx.update(|cx| {
        let _ = entity.update(cx, |this, cx| {
            this.update_status = match result {
                UpdateCheckResult::NoUpdate => UpdateStatus::NoUpdate,
                UpdateCheckResult::UpdateAvailable(info) => UpdateStatus::Available {
                    version: info.version,
                    notes: info.release_notes,
                },
                UpdateCheckResult::Error(err) => UpdateStatus::Error(err),
            };
            cx.notify();
        });
    });
}).detach();
```

### 状态管理

使用枚举 `UpdateStatus` 管理更新状态：

```rust
#[derive(Debug, Clone, PartialEq)]
enum UpdateStatus {
    Idle,                                    // 空闲
    Checking,                                // 检查中
    Available { version: String, notes: String },  // 有可用更新
    NoUpdate,                                // 无更新
    Error(String),                           // 错误
}
```

### 版本比较

采用标准的语义化版本比较：

```rust
impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.major.cmp(&other.major) {
            Ordering::Equal => match self.minor.cmp(&other.minor) {
                Ordering::Equal => self.patch.cmp(&other.patch),
                other => other,
            },
            other => other,
        }
    }
}
```

## 待实现功能

### 当前限制

1. **HTTP 客户端未实现**:
   - `UpdateChecker::fetch_latest_release()` 方法需要集成 HTTP 客户端（如 reqwest）
   - 当前返回错误消息提示需要实现

2. **下载功能未实现**:
   - `UpdateDownloader::download()` 方法需要实现实际的 HTTP 下载
   - 需要添加 `reqwest` 依赖

3. **安装功能**:
   - 不同平台的安装逻辑（macOS .dmg, Windows .exe, Linux .AppImage）
   - 权限提升和安全验证

### 扩展计划

#### 短期 (Phase 1)
- [ ] 集成 reqwest HTTP 客户端
- [ ] 实现 GitHub Releases API 调用
- [ ] 实现文件下载功能
- [ ] 添加下载进度条

#### 中期 (Phase 2)
- [ ] 实现自动安装功能（macOS）
- [ ] 添加更新签名验证
- [ ] 实现增量更新（差分下载）
- [ ] 添加更新回滚功能

#### 长期 (Phase 3)
- [ ] 支持多平台安装（Windows, Linux）
- [ ] 实现后台静默更新
- [ ] 添加更新日志查看
- [ ] Beta 版本通道支持

## 依赖要求

### 当前依赖

```toml
[dependencies]
anyhow = "1.0"
tokio = { version = "1.48.0", features = ["rt", "rt-multi-thread", "process", "fs", "io-util"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

### 需要添加的依赖

```toml
[dependencies]
# HTTP 客户端
reqwest = { version = "0.11", features = ["json", "stream"] }

# 可选：进度条显示
indicatif = "0.17"

# 可选：文件校验
sha2 = "0.10"
```

## 安全考虑

1. **HTTPS 强制**: 所有更新检查和下载必须通过 HTTPS
2. **签名验证**: 验证下载文件的数字签名
3. **权限最小化**: 只请求必要的系统权限
4. **用户确认**: 重要操作需要用户明确确认
5. **回滚机制**: 更新失败时能够回滚到之前版本

## 测试

### 单元测试

```bash
cargo test --package agentx --lib core::updater
```

### 集成测试

1. 启动应用程序
2. 打开设置面板
3. 点击 "Check Now" 按钮
4. 验证状态更新是否正确

### 模拟测试

可以修改 `UpdateChecker::fetch_latest_release()` 返回模拟数据进行测试：

```rust
Ok(UpdateInfo {
    version: "0.5.0".to_string(),
    download_url: "https://example.com/agentx-0.5.0.dmg".to_string(),
    release_notes: "新功能：版本检测和自动更新".to_string(),
    published_at: "2025-12-05T00:00:00Z".to_string(),
    file_size: Some(50 * 1024 * 1024), // 50MB
})
```

## 相关文件

- `src/core/updater/` - 更新系统核心模块
- `src/panels/settings_panel.rs` - 设置面板 UI
- `docs/UPDATE_FEATURE.md` - 本文档
- `Cargo.toml` - 版本配置

## 常见问题

### Q: 如何修改更新检查 API 端点？

A: 在创建 `UpdateChecker` 时使用 `with_url()` 方法：

```rust
let checker = UpdateChecker::with_url("https://api.example.com/releases/latest");
```

### Q: 如何自定义下载目录？

A: 使用 `UpdateDownloader::with_dir()`:

```rust
let downloader = UpdateDownloader::with_dir(PathBuf::from("/custom/path"))?;
```

### Q: 更新检查是否会阻塞 UI？

A: 不会。所有更新检查都在后台异步执行，UI 保持响应。

### Q: 如何禁用自动更新？

A: 在设置面板中关闭 "Auto Update" 开关。

## 贡献

欢迎提交 Issue 和 Pull Request 来改进此功能。

## 许可证

与 AgentX 项目使用相同的许可证。
