# API 反向代理 — Claude↔Google 协议转换

## 需求概述

### 目标
修复 Chat 测试页面的 `401 invalid x-api-key` 错误，使 Antigravity 账号能够正确地通过代理发起 Claude API 请求。

### 背景
- Antigravity 账号使用的是 Google OAuth access_token，需要发送到 Google Cloud `v1internal` 端点
- 客户端（Chat 页面、Claude Code 等）发送的是 Anthropic Claude Messages API 格式
- 代理需要在中间做协议转换：Claude Messages ↔ Google generateContent

### 依据
通过分析 `cockpit-tools_sub` 下 **7 个参考项目**（Antigravity-Manager、gcli2api、sub2api 等），确认统一的架构模式。

---

## 功能要求

### 1. 协议转换（Claude Messages → Google generateContent）

| Claude 字段 | Google 字段 |
|-------------|-------------|
| `messages[].role: "assistant"` | `contents[].role: "model"` |
| `messages[].content: "text"` | `contents[].parts: [{"text": ...}]` |
| `system` | `systemInstruction.parts[].text` |
| `max_tokens` | `generationConfig.maxOutputTokens` |
| `temperature` | `generationConfig.temperature` |
| `tools[].input_schema` | `tools[].functionDeclarations[].parameters` |
| `stream: true` | 使用 `streamGenerateContent?alt=sse` |

### 2. 多端点降级

优先级：
1. `daily-cloudcode-pa.sandbox.googleapis.com/v1internal`（普通账号优先）
2. `daily-cloudcode-pa.googleapis.com/v1internal`
3. `cloudcode-pa.googleapis.com/v1internal`（GCP ToS 账号优先）

### 3. SSE 响应转换（Google → Claude）

将 Google 的 `candidates[0].content.parts[].text` 转换为 Claude 的 `content_block_delta` 事件。

### 4. Codex Provider 保持不变

OpenAI 格式请求直接转发到 `api.openai.com`，不做协议转换。

---

## 非功能要求

- 请求超时可配置（默认 120 秒）
- 账号轮询策略：round_robin / random / single
- 错误响应透传原始错误信息
