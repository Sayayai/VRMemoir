# VRMemoir

`vrmemoir` 是一个基于 Rust 开发的强大服务，用于监控 VRChat 日志、追踪玩家加入与离开事件，并记录 VRChat 进程的音频（支持可选的麦克风输入），以此来保存您的虚拟现实记忆。该程序能够将事件数据无缝存储至 SQLite 中，并在本地提供 HTTP API，以便轻松访问与集成。

![VRMemoir Logo](vrmemoir_logo.png)

## ✨ 特性

- **日志监控**：自动追踪 VRChat 日志，捕获玩家加入、离开及世界切换事件。
- **音频录制**：通过 WASAPI Loopback 捕捉高质量的 VRChat 进程音频，支持麦克风混音。
- **高效编码**：将录制的音频直接编码为 OGG/Opus 格式。
- **数据持久化**：使用可靠的 SQLite 数据库在本地存储事件和元数据。
- **本地 HTTP API**：在 `127.0.0.1:3001` 提供 REST 风格 API，用于交互、认证及个性化数据检索。
- **CI/CD 驱动**：以 GitHub Actions 作为默认编译、验证和发布入口，减少本地环境依赖。

## 🚀 快速开始

### 环境依赖

- 日常开发默认不要求本地构建环境，编译与验证统一由 GitHub Actions 执行。
- 目标平台：**Windows**（依赖于 `wasapi`、`winreg` 以及典型的 VRChat 日志路径）

### 开发与交付流程

1. 克隆本仓库并进入项目根目录。
2. 在开发分支 `codex/dev` 上进行日常开发。
3. 推送后由 GitHub Actions `CI` 工作流自动执行格式检查、Lint、测试和 Windows Release 构建。
4. `codex/dev` 与普通分支只保留 Actions artifact，不发布 GitHub Release。
5. 需要正式发布时，推送版本 tag（如 `v1.2.3`）或手动触发 `Release Win64` 工作流。
6. 将环境变量配置示例文件复制为 `.env` 并填写；实际运行时再使用生成的 Windows 可执行文件。

### CI/CD 说明

- `CI` 工作流会在 `main`、`codex/dev` 和 Pull Request 上运行。
- `Release Win64` 仅用于正式版本发布。
- Cargo registry 与 `target` 目录已在 GitHub Actions 中启用缓存，以缩短重复构建时间。

## 🧾 VRCX 月度报告导出

运行中的程序会监听终端输入。直接输入一个 VRChat 用户 ID，例如：

```text
usr_893713c7-1e96-44fc-b5ef-43e458a25473
```

程序会执行以下操作：

1. 读取 VRCX 数据库，默认路径为 `C:\Users\admin\AppData\Roaming\VRCX\VRCX.sqlite3`。
2. 读取最近 30 天的 VRCX 时间线记录，包括：位置、上线、下线、简介变更、状态、模型。
3. 复用 VRChat API 获取玩家基础资料和群组信息。
4. 以玩家显示名称创建目录，并输出完整 Markdown 报告：

```text
<显示名称>/<显示名称>_L1-Lxxx_PROFILE_L1-Lxxx_TIMELINE_Lx-Lx_GROUPS_Lx-Lx.md
```

例如显示名称是 `sakuya`，则输出：

```text
sakuya/sakuya_L1-L120_PROFILE_L1-L28_TIMELINE_L33-L108_GROUPS_L112-L120.md
```

补充说明：

- 每次输入 `userid` 都会重新读取并重新生成完整报告，不走旧缓存文件复用。
- 如果需要改 VRCX 数据库位置，可以设置环境变量 `VRCX_DB_PATH`。
- 目前这个功能是普通 CLI 导出流程，不走 FSM，因为它属于一次性任务，不是运行时状态切换。

## ⚙️ 配置说明

您可以通过修改可执行文件所在目录下的 `.env` 文件来配置应用程序。

关键的环境变量包括：
- `VRC_COOKIE`：直接调用 API 进行身份验证所需。
- `VRC_USERNAME` / `VRC_PASSWORD`：可选，用于自动重新登录。
- `VRC_PROXY`：可选，SOCKS5 代理地址。
- `RECORD_MIC`：设置是否录音麦克风，填 `true` 或 `false`（默认：`false`）。
- `MIC_DEVICE`：可选，用于指定录音麦克风设备名称的子字符串。

> **注意**：程序运行产生的产物，如 `data.db`（SQLite 数据库）、录音文件及时间线输出等，都将保存在可执行文件的同级目录下。

## 🛠️ 开发指南

- **默认验证入口**：GitHub Actions `CI`
- **触发分支**：`main`、`codex/dev`、Pull Request
- **CI 检查项**：`cargo fmt --all -- --check`、`cargo clippy --all-targets --all-features -- -D warnings`、`cargo test --locked`、`cargo build --release --locked`
- **缓存策略**：缓存 Cargo registry 和 `target` 目录，加速重复编译
- **Release 入口**：GitHub Actions `Release Win64`，仅用于版本发布

在提交 Pull Request 之前，请确保 `CI` 工作流已通过。如果您修改了监控器 (watcher) 或录音模块 (recorder)，建议在 Windows 上使用真实的 VRChat 日志和进程完成手动的冒烟测试。

## 🔒 安全考量

- **切勿**提交您的 `.env` 文件、真实的账号凭证或是本地的 `data.db` 数据库文件。
- 请将您的 `VRC_COOKIE` 及相关身份会话数据作为机密信息妥善保管。
- HTTP 服务器默认仅绑定在 `localhost` 上。除非您完全了解相关的安全风险并采取了必要的安全加固措施，否则请勿将其暴露至公网环境。

## 📄 许可证

详情请查阅项目中的 `LICENSE` 文件。
