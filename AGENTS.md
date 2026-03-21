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

## 开发与验证约定

- 默认以 GitHub Actions 为唯一编译与验证入口，不再要求本地 Docker/本地 Rust 环境完成日常构建检查。
- 本地开发以代码编辑、静态审查和必要的配置修改为主；编译、格式检查、Lint、测试和 Windows 产物生成统一交给 CI/CD。
- 如确有必要进行本地临时验证，需在变更说明中明确标注其为补充性操作，而不是默认流程。

## CI/CD 与编译

- **CI 工作流**：项目配置了 [ci.yml](.github/workflows/ci.yml)，在 `main`、`codex/dev` 分支推送以及 Pull Request 时自动执行。
- **CI 验证内容**：`cargo fmt --all -- --check`、`cargo clippy --all-targets --all-features -- -D warnings`、`cargo test --locked`、`cargo build --release --locked`。
- **开发分支产物策略**：`codex/dev` 分支只在 GitHub Actions 中生成并保存 artifact，不创建 GitHub Release。
- **正式发布工作流**：项目配置了 [release.yml](.github/workflows/release.yml)，仅在推送 `v*.*.*` tag 或手动触发时发布 GitHub Release。
- **手动触发**：若需手动验证开发分支，请在 Actions 里运行 `CI`；若需手动发布，请运行 `Release Win64`。

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
- CI/CD 中默认在 GitHub Actions 的 `windows-latest` 环境执行验证和构建，并缓存 Cargo registry 与 `target` 目录。

## 编码规范

- 使用 Rust 2021 习惯用法，并保持模块职责单一。
- 避免在运行时路径中使用 panic；返回 `anyhow::Result` 或显式处理错误。
- 优先使用 `tracing` 进行结构化日志记录，并使用现有的 i18n 键（`t!(...)`），而不是硬编码面向用户的文本。
- 除非有明确要求，否则保持 API 行为向后兼容。
- 除非必要，否则不要添加新依赖；在 PR/提交说明中注明原因。

## 测试说明

- 完成更改前的默认要求是等待 GitHub Actions `CI` 工作流通过。
- `CI` 工作流必须覆盖以下检查：
    - `cargo fmt --all -- --check`
    - `cargo clippy --all-targets --all-features -- -D warnings`
    - `cargo test --locked`
    - `cargo build --release --locked`
- 对于录音机（recorder）/监控器（watcher）的更改，若涉及真实设备或 VRChat 运行时行为，仍建议补充 Windows 手动冒烟测试；若未执行，应在说明中明确标注。
- 如果某项检查因 GitHub Actions 环境限制而被跳过，请明确说明跳过内容及原因。

## 安全考量

- 切勿提交来自 `.env` 的真实凭据/Cookie 或本地数据库内容。
- 将 `VRC_COOKIE`、账号凭据和会话数据视为机密信息。
- HTTP 服务器绑定到 localhost；除非用户明确要求远程公开并进行了相应的加固，否则请保持本地访问。

## 更改与 PR 指南

- 保持补丁（patch）小巧且聚焦；避免无关的重构。
- 更改环境变量、端点或输出布局时，请更新文档/示例。
- 如果 FSM 状态转换或 API 响应字段的行为发生变化，请在更改摘要中清楚地说明兼容性影响。

