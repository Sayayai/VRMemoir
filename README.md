# VRMemoir

`vrmemoir` 是一个基于 Rust 开发的强大服务，用于监控 VRChat 日志、追踪玩家加入与离开事件，并记录 VRChat 进程的音频（支持可选的麦克风输入），以此来保存您的虚拟现实记忆。该程序能够将事件数据无缝存储至 SQLite 中，并在本地提供 HTTP API，以便轻松访问与集成。

![VRMemoir Logo](vrmemoir_logo.png)

## ✨ 特性

- **日志监控**：自动追踪 VRChat 日志，捕获玩家加入、离开及世界切换事件。
- **音频录制**：通过 WASAPI Loopback 捕捉高质量的 VRChat 进程音频，支持麦克风混音。
- **高效编码**：将录制的音频直接编码为 OGG/Opus 格式。
- **数据持久化**：使用可靠的 SQLite 数据库在本地存储事件和元数据。
- **本地 HTTP API**：在 `127.0.0.1:3001` 提供 REST风格 API，用于交互、认证及个性化数据检索。
- **CI/CD 就绪**：已集成 GitHub Actions，支持自动化构建 Windows 64 位版本。

## 🚀 快速开始

### 环境依赖

- [Rust 工具链](https://rustup.rs/) (Stable 稳定版)
- 目标平台：**Windows** (依赖于 `wasapi`、`winreg` 以及典型的 VRChat 日志路径)

### 安装与运行

1. 克隆本仓库并进入项目根目录。
2. 构建项目：
   ```bash
   cargo build --release
   ```
3. 复制环境变量配置示例文件并进行配置：
   ```bash
   cp .env.example .env
   ```
4. 运行应用：
   ```bash
   cargo run
   ```

## ⚙️ 配置说明

您可以通过修改可执行文件所在目录下的 `.env` 文件来配置应用程序。

关键的环境变量包括：
- `VRC_COOKIE`：直接调用 API 进行身份验证所需。
- `VRC_USERNAME` / `VRC_PASSWORD`：可选，用于自动重新登录。
- `VRC_PROXY`：可选，SOCKS5 代理地址。
- `RECORD_MIC`：设置是否录音麦克风，填 `true` 或 `false`（默认: `false`）。
- `MIC_DEVICE`：可选，用于指定录音麦克风设备名称的子字符串。

> **注意**：程序运行产生的产物，如 `data.db`（SQLite 数据库）、录音文件及时间线输出等，都将保存在可执行文件的同级目录下。

## 🛠️ 开发指南

- **Lint 检查**：`cargo clippy --all-targets --all-features -- -D warnings`
- **代码格式化**：`cargo fmt --all`
- **运行测试**：`cargo test`

在提交 Pull Request 之前，请确保通过了所有的代码检查。如果您修改了监控器 (watcher) 或录音模块 (recorder)，建议在 Windows 上使用真实的 VRChat 日志和进程完成手动的冒烟测试。

## 🔒 安全考量

- **切勿**提交您的 `.env` 文件、真实的账号凭证或是本地的 `data.db` 数据库文件。
- 请将您的 `VRC_COOKIE` 及相关身份会话数据作为机密信息妥善保管。
- HTTP 服务器默认仅绑定在 `localhost` 上。除非您完全了解相关的安全风险并采取了必要的安全加固措施，否则请勿将其暴露至公网环境。

## 📄 许可证

详情请查阅项目中的 `LICENSE` 文件。
