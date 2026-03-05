# API 反向代理功能实现计划

参考 `sub2api` 项目的 API 网关模式，在 cockpit-tools 中实现内置的 API 反向代理服务，以 Provider 为维度，支持多账号轮询和可配置参数。

## 核心目标

- 在 Tauri Rust 后端启动一个 HTTP 反向代理服务器
- 以 Provider（Antigravity / Codex / GitHub Copilot / Windsurf / Kiro）为维度进行路由
- 支持多账号**轮询/负载均衡**（round-robin），可配置
- 在设置页「网络服务」标签中添加代理配置 UI

## User Review Required

> [!IMPORTANT]
> 以下问题需确认：
> 1. **初始版本先支持哪些 Provider？** 建议先做 Antigravity（它是核心平台），后续逐步添加其他平台。
> 2. **代理监听方式**：是统一一个端口，通过路径前缀区分 Provider（如 `/antigravity/v1/messages`、`/codex/v1/chat/completions`），还是每个 Provider 独立端口？建议统一端口 + 路径前缀。
> 3. **认证方式**：API Key 前缀建议使用 `sk-cockpit-`，客户端请求时携带此 Key，代理验证后替换为真实账号 Token 转发上游。是否需要此机制，还是不验证直接转发？

## Proposed Changes

### 1. Rust 后端 - 配置模型

#### [MODIFY] [config.rs](file:///e:/learn/cockpit-tools/src-tauri/src/modules/config.rs)

在 `UserConfig` 中添加 API 代理配置字段 `ApiProxyConfig`，包含 `enabled`、`port`、以及各 Provider 的 `ProviderProxyConfig`（策略和账号 ID 列表）。

### 2. Rust 后端 - 代理服务模块

#### [NEW] [api_proxy.rs](file:///e:/learn/cockpit-tools/src-tauri/src/modules/api_proxy.rs)

核心反向代理模块（`axum` + `reqwest`）：路由 `/{provider}/v1/*path` → 上游，支持 SSE 流式透传、多账号轮询。

### 3. Rust 后端 - Tauri Commands

#### [NEW] [api_proxy.rs](file:///e:/learn/cockpit-tools/src-tauri/src/commands/api_proxy.rs)

`get_api_proxy_config`、`save_api_proxy_config`、`get_api_proxy_status`、`restart_api_proxy`

### 4. 前端 - 设置页网络标签

#### [MODIFY] [SettingsPage.tsx](file:///e:/learn/cockpit-tools/src/pages/SettingsPage.tsx)

在 Network Tab 的 WebSocket 配置下方新增「API 反向代理」配置区，包含全局开关、端口、各 Provider 开关和负载均衡策略选择。

### 5. Cargo 依赖

#### [MODIFY] [Cargo.toml](file:///e:/learn/cockpit-tools/src-tauri/Cargo.toml)

添加 `axum`、`hyper`、`tower` 依赖。
