# AGENTS.md

## 项目概览

- `vrmemoir` 是一个 Rust 服务，用于监控 VRChat 日志，追踪玩家加入/离开事件，记录 VRChat 进程音频（可选麦克风），将数据存储在 SQLite 中，并在 `127.0.0.1:3001` 暴露本地 HTTP API。
- 主要模块：
    - `src/main.rs`: 应用引导、身份验证流程、任务编排、服务器启动。
    - `src/watcher.rs`: VRChat 日志追踪和事件解析。
    - `src/fsm.rs`: 运行时状态机（`Idle` / `InWorld` / `Recording`）。
    - `src/recorder.rs`: WASAPI 回环 + 可选麦克风混音 + OGG/Opus 编码。
    - `src/server.rs`: Axum 路由以及身份验证/个性化端点。
    - `src/db.rs`: SQLite 持久化。

## 设置命令

- 安装工具链：`rustup toolchain install stable`
- 构建调试版：`cargo build`
- 本地运行：`cargo run`
- 运行测试：`cargo test`
- Lint 检查：`cargo clippy --all-targets --all-features -- -D warnings`
- 格式检查：`cargo fmt --all -- --check`
- 格式化写入：`cargo fmt --all`

## CI/CD 与编译

- **GitHub Actions**：项目配置了 [win64-release.yml](.github/workflows/win64-release.yml)，在 `main` 分支有代码推送时自动执行。
- **手动触发编译**：若需手动触发编译并发布 Release，请访问 GitHub 项目页面的 **Actions** 选项卡，选择 **Build And Release Win64** 工作流，并点击 **Run workflow** 按钮。
  [👉 立即前往 GitHub Actions 触发编译](../../actions/workflows/win64-release.yml)


## 环境与运行时

- 将可执行文件目录（或本地开发的仓库根目录）下的 `.env.example` 复制为 `.env`。
- 重要环境变量：
    - `VRC_COOKIE`（直接 API 认证所需）。
    - `VRC_USERNAME` / `VRC_PASSWORD`（可选，用于自动重新登录）。
    - `VRC_PROXY`（可选，SOCKS5 代理）。
    - `RECORD_MIC`（`true/false`，默认为 `false`）。
    - `MIC_DEVICE`（可选，设备名称子字符串）。
- 运行时产物将写入可执行文件旁：
    - `data.db` SQLite 数据库。
    - 录音文件夹以及时间线/音频输出。

## 平台说明

- 主要目标是 Windows（使用 `wasapi`, `winreg`, 以及 VRChat 进程/日志路径）。
- 本地日志监控预期路径为 `%APPDATA%\\..\\LocalLow\\VRChat\\VRChat`。
- 支持使用 Docker 交叉编译到 Windows GNU：
    - `docker compose run --rm builder`

## 编码规范

- 使用 Rust 2021 习惯用法，并保持模块职责单一。
- 避免在运行时路径中使用 panic；返回 `anyhow::Result` 或显式处理错误。
- 优先使用 `tracing` 进行结构化日志记录，并使用现有的 i18n 键（`t!(...)`），而不是硬编码面向用户的文本。
- 除非有明确要求，否则保持 API 行为向后兼容。
- 除非必要，否则不要添加新依赖；在 PR/提交说明中注明原因。

## 测试说明

- 完成更改前的最低要求：
    - `cargo fmt --all -- --check`
    - `cargo clippy --all-targets --all-features -- -D warnings`
    - `cargo test`
- 对于录音机（recorder）/监控器（watcher）的更改，还需在 Windows 上使用真实的 VRChat 日志/进程进行手动冒烟测试（smoke check）。
- 如果跳过了某项检查（受平台/工具限制），请明确说明跳过的内容及其原因。

## 安全考量

- 切勿提交来自 `.env` 的真实凭据/Cookie 或本地数据库内容。
- 将 `VRC_COOKIE`、账号凭据和会话数据视为机密信息。
- HTTP 服务器绑定到 localhost；除非用户明确要求远程公开并进行了相应的加固，否则请保持本地访问。

## 更改与 PR 指南

- 保持补丁（patch）小巧且聚焦；避免无关的重构。
- 更改环境变量、端点或输出布局时，请更新文档/示例。
- 如果 FSM 状态转换或 API 响应字段的行为发生变化，请在更改摘要中清楚地说明兼容性影响。
