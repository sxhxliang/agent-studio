<div align="center">

# 🚀 AgentX

**A Modern Desktop AI Agent Studio**

[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey.svg)](#-installation)
[![Version](https://img.shields.io/badge/version-0.5.0-green.svg)](https://github.com/sxhxliang/agent-studio/releases)
[![Downloads](https://img.shields.io/github/downloads/sxhxliang/agent-studio/total.svg)](https://github.com/sxhxliang/agent-studio/releases)

[🎯 Features](#-features) • [📦 Installation](#-installation) • [🎬 Demo](#-demo) • [🛠️ Development](#%EF%B8%8F-development) • [📖 Documentation](#-documentation)

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

<div align="center">

**Built with ❤️ using [GPUI](https://www.gpui.rs/)**

[⬆ Back to Top](#-agentx)

</div>
