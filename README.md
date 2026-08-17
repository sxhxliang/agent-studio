<div align="center">

# 🚀 AgentX

**A Modern Desktop AI Agent Studio**

[简体中文](./README.zh-CN.md)

[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey.svg)](#-installation)
[![Version](https://img.shields.io/badge/version-0.5.0-green.svg)](https://github.com/sxhxliang/agent-studio/releases)
[![Downloads](https://img.shields.io/github/downloads/sxhxliang/agent-studio/total.svg)](https://github.com/sxhxliang/agent-studio/releases)

[🎯 Features](#-features) • [📦 Installation](#-installation) • [🎬 Demo](#-demo) • [🛠️ Development](#%EF%B8%8F-development) • [📖 Documentation](#-documentation) • [❓ QA](#-qa)

</div>

---

## 🎬 Demo

<div align="center">
  <img src="assets/demo.gif" alt="AgentX Demo" width="100%" />
</div>

<div align="center">
  <img src="assets/demo1.jpeg" alt="AgentX Main Interface" width="32%" />
  <img src="assets/demo2.jpeg" alt="Multi-Agent Conversations" width="32%" />
  <img src="assets/demo3.jpeg" alt="Code Editor & Terminal" width="32%" />
</div>

---

## ✨ Why AgentX?

AgentX is a **GPU-accelerated**, **cross-platform** desktop application that brings AI agents to your workflow. Built with cutting-edge technologies, it provides a seamless experience for interacting with multiple AI agents, editing code, managing tasks, and more—all in one unified interface.

### 🎯 Features

- 🤖 **Multi-Agent Support** - Connect and chat with multiple AI agents simultaneously via Agent Client Protocol (ACP)
- 💬 **Real-time Conversations** - Streaming responses with support for thinking blocks and tool calls
- 📝 **Built-in Code Editor** - LSP-enabled editor with syntax highlighting and autocomplete
- 🖥️ **Integrated Terminal** - Execute commands without leaving the app
- 🎨 **Customizable Dock System** - Drag-and-drop panels to create your perfect workspace
- 🌍 **Internationalization** - Support for multiple languages (English, 简体中文)
- 🎭 **Theme Support** - Light and dark themes with customizable colors
- 📊 **Session Management** - Organize conversations across multiple sessions
- 🔧 **Tool Call Viewer** - Inspect agent tool executions in detail
- 💾 **Auto-save** - Never lose your work with automatic session persistence
- ⚡ **GPU-Accelerated** - Blazing fast UI powered by GPUI framework

---

## 🤖 Supported Agents

Based on `config.json` in this repository we test the following agents:

<div align="center">
  <table>
    <tr>
      <td align="center">
        <img src="assets/logo/openai.svg" alt="Codex" width="48" />
        <br />Codex
      </td>
      <td align="center">
        <img src="assets/logo/claude.svg" alt="Claude" width="48" />
        <br />Claude
      </td>
      <td align="center">
        <img src="assets/logo/kimi.svg" alt="Kimi Code" width="48" />
        <br />Kimi Code
      </td>
    </tr>
    <tr>
      <td align="center">
        <img src="assets/logo/qwen.svg" alt="Qwen" width="48" />
        <br />Qwen
      </td>
      <td align="center">
        <img src="assets/logo/qoder.svg" alt="Qoder" width="48" />
        <br />Qoder
      </td>
      <td align="center">
        <img src="assets/logo/opencode.svg" alt="OpenCode" width="48" />
        <br />OpenCode
      </td>
    </tr>
    <tr>
      <td align="center">
        <img src="assets/logo/gemini.svg" alt="Gemini" width="48" />
        <br />Gemini
      </td>
      <td align="center">
        <img src="assets/logo/augment_code.svg" alt="AugmentCode" width="48" />
        <br />AugmentCode
      </td>
      <td align="center">
        <img src="assets/logo/iflow.svg" alt="Iflow" width="48" />
        <br />Iflow
      </td>
    </tr>
  </table>
</div>

### More ACP-Compatible Agents

From the [ACP "Agents implementing the Agent Client Protocol"](https://agentclientprotocol.com/get-started/agents) list:

- AgentPool
- Blackbox AI
- Code Assistant
- Docker's cagent
- fast-agent
- GitHub Copilot (public preview)
- Goose
- JetBrains Junie (coming soon)
- Minion Code
- Mistral Vibe
- OpenHands
- Pi (via pi-acp adapter)
- Stakpak
- VT Code

---

## 📦 Installation

### 📥 [Download Latest Release](https://github.com/sxhxliang/agent-studio/releases)

<details>
<summary><b>View detailed installation instructions for each platform</b></summary>

### Download Pre-built Binaries

Get the latest release for your platform:

#### 🪟 Windows

Download: `agentx-v{version}-x86_64-windows.zip` or `agentx-{version}-setup.exe`

```bash
# Extract and run
# Or double-click setup.exe to install

# Using winget (coming soon)
# winget install AgentX
```

#### 🐧 Linux

Download: `agentx-v{version}-x86_64-linux.tar.gz` or `agentx_{version}_amd64.deb`

```bash
# For Debian/Ubuntu (.deb)
sudo dpkg -i agentx_0.5.0_amd64.deb

# For other distros (.tar.gz)
tar -xzf agentx-v0.5.0-x86_64-linux.tar.gz
cd agentx
./agentx

# Or using AppImage
chmod +x agentx-v0.5.0-x86_64.AppImage
./agentx-v0.5.0-x86_64.AppImage
```

#### 🍎 macOS

Download: `agentx-v{version}-aarch64-macos.dmg` (Apple Silicon) or `agentx-v{version}-x86_64-macos.dmg` (Intel)

```bash
# Double-click .dmg and drag AgentX to Applications folder

# Using Homebrew (coming soon)
# brew install --cask agentx
```

</details>

---

## 🚀 Quick Start

1. **Download** AgentX for your platform from the [releases page](https://github.com/sxhxliang/agent-studio/releases)
2. **Install** following your OS-specific instructions above
3. **Launch** AgentX
4. **Configure** your AI agent in Settings → MCP Config
5. **Start chatting** with your agent!

---

## 🛠️ Development

<details>
<summary><b>Click to expand development guide</b></summary>

### Prerequisites

- Rust 1.83+ (2024 edition)
- Platform-specific dependencies:
  - **Windows**: MSVC toolchain
  - **Linux**: `libxcb`, `libfontconfig`, `libssl-dev`
  - **macOS**: Xcode command line tools

### Build from Source

```bash
# Clone the repository
git clone https://github.com/sxhxliang/agent-studio.git
cd agent-studio

# Build and run
cargo run

# Release build
cargo build --release
```

### Development Commands

```bash
# Run with logging
RUST_LOG=info cargo run

# Run tests
cargo test

# Check code
cargo clippy

# Format code
cargo fmt
```

</details>

---

## 🏗️ Built With

- **[GPUI](https://www.gpui.rs/)** - GPU-accelerated UI framework from Zed Industries
- **[gpui-component](https://github.com/longbridge/gpui-component)** - Rich UI component library
- **[Agent Client Protocol](https://crates.io/crates/agent-client-protocol)** - Standard protocol for agent communication
- **[Tokio](https://tokio.rs/)** - Async runtime
- **[Tree-sitter](https://tree-sitter.github.io/)** - Syntax highlighting
- **Rust** - Memory-safe systems programming language

---

## 📖 Documentation

- [User Guide](docs/user-guide.md) - Learn how to use AgentX
- [Architecture](CLAUDE.md) - Technical architecture and design
- [Contributing](CONTRIBUTING.md) - How to contribute to the project
- [Agent Configuration](docs/agent-config.md) - Set up your AI agents

---

## ❓ QA

### Q: What should I do if no agents appear in the agent list?

A: This is usually caused by network access restrictions. Configure your proxy first, then set it on the startup page or in the Settings panel, and try again.

### Q: Does the app manage agent authorization and availability?

A: No. AgentX provides the desktop studio experience only, and does not manage provider authorization state or service availability. Ensure your target agent is authorized and reachable before using the app.

### Q: AgentX starts but no agent can respond. What should I check?

A: Open `Settings -> MCP Config` and verify your provider settings in `config.json` (API endpoint, key, command path, environment variables).

### Q: How do I reset a broken dock layout?

A: Close AgentX and delete `docks-agentx.json`, then relaunch the app.

### Q: Where can I find runtime/session data?

A: Layout/session runtime files are written under `agentx/` , and session data is stored in `sessions/`.

### Q: How can I debug startup or integration issues?

A: Run AgentX with logs enabled:

```bash
RUST_LOG=info cargo run
```

---

## 🤝 Contributing

We welcome contributions! Whether it's bug reports, feature requests, or pull requests—every contribution helps make AgentX better.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

---

## 🌟 Show Your Support

If you find AgentX helpful, please consider:

- ⭐ **Star this repository** to show your support
- 🐦 **Share** it with your friends and colleagues
- 🐛 **Report bugs** to help us improve
- 💡 **Suggest features** you'd like to see

---

## 📝 License

This project is licensed under the **Apache-2.0 License**. See [LICENSE](LICENSE) file for details.

---

## 🙏 Acknowledgments

Special thanks to:

- **[Zed Industries](https://zed.dev/)** for the amazing GPUI framework
- **[GPUI Component](https://github.com/longbridge/gpui-component)** contributors
- All our **contributors** and **supporters**

---
## Star History

<p align="center">
  <a href="https://star-history.dera.page/#sxhxliang/agent-studio&Date">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://star-history.dera.page/svg?repos=sxhxliang/agent-studio&type=Date&theme=dark"/>
      <img src="https://star-history.dera.page/svg?repos=sxhxliang/agent-studio&type=Date" alt="Star History Chart"/>
    </picture>
  </a>
</p>

<div align="center">

**Built with ❤️ using [GPUI](https://www.gpui.rs/)**

[⬆ Back to Top](#-agentx)

</div>
