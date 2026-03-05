# Codex 模型列表获取 + 账号额度检测

## 背景

当前 TDD 脚本和 Rust 后端中的 Codex 模型列表存在两个问题：
1. **模型硬编码且过时** — `api_proxy.rs` 的 `get_codex_model_list()` 返回 `gpt-4o` 等旧模型，而 Codex 后端实际支持的是 `gpt-5.x` 系列
2. **未检测账号额度** — TDD 脚本直接用找到的第一个 Codex 账号发请求，不判断该账号是否有额度，导致免费账号返回 `429 usage_limit_reached`

## 参考项目分析

| 项目 | 模型列表来源 | 有远程 API 吗？ |
|------|-------------|----------------|
| **CodexMonitor** (Tauri/Rust) | ✅ 通过 Codex CLI 的 JSON-RPC `model/list` 远程获取 | ✅ 有（通过 CLI 进程代理） |
| **Codex2API** (Go) | `openai.DefaultModels` 硬编码 gpt-5.x 系列 | ❌ 无 |
| **sub2api** (Go) | `openai.DefaultModels` 硬编码 + 数据库 `model_mapping` | ❌ 无 |
| **opencode-antigravity-auth** (TS) | Google `fetchAvailableModels` API 远程获取 | ✅ 仅 Google/Antigravity |
| **agentapi** (Go) | 不涉及模型管理（是 CLI agent 封装层） | ❌ 不适用 |

### CodexMonitor 的远程模型获取机制（✅ 关键发现）

```
CodexMonitor (Tauri App)
  → 启动 Codex CLI 进程
  → 通过 stdin/stdout JSON-RPC 通信
  → 发送 "model/list" 请求
  → Codex CLI 向 chatgpt.com 查询
  → 返回模型列表（包含 gpt-5.3-codex-spark, gpt-5.2-codex 等）
```

**返回的模型数据结构：**
```json
{
  "result": {
    "data": [
      {
        "id": "m1",
        "model": "gpt-5.3-codex-spark",
        "displayName": "GPT-5.3-Codex-Spark",
        "description": "...",
        "supportedReasoningEfforts": [...],
        "defaultReasoningEffort": "medium",
        "isDefault": false
      }
    ]
  }
}
```

> [!IMPORTANT]
> **CodexMonitor 证明了 Codex 模型可以远程获取！** 但它需要启动 Codex CLI 进程作为中间层。对于我们的 TDD 脚本来说，有两种选择：
> 1. **方案 A（推荐）**：直接调用 ChatGPT 隐藏的 API 获取模型列表（需要逆向 Codex CLI `model/list` 的实际 HTTP 请求）
> 2. **方案 B（务实）**：参考 `Codex2API` 的 `DefaultModels` 更新硬编码列表为 gpt-5.x，同时加配额检查

### Codex2API 的 DefaultModels（硬编码参考）

```
gpt-5.3, gpt-5.3-codex, gpt-5.2, gpt-5.2-codex,
gpt-5.1-codex-max, gpt-5.1-codex, gpt-5.1, gpt-5.1-codex-mini, gpt-5
```

### 当前 Rust 后端的硬编码（已过时）

```
gpt-4o, gpt-4o-mini, gpt-4-turbo, gpt-4.1, gpt-4.1-mini,
gpt-4.1-nano, o1, o1-preview, o1-mini, o3, o3-mini, o3-pro, o4-mini, gpt-4.5-preview
```

## Proposed Changes

### 1. TDD 脚本：远程获取 Codex 模型列表（方案 A）

#### [MODIFY] [test_proxy_tdd.py](file:///e:/learn/cockpit-tools/src-tauri/test_proxy_tdd.py)

逆向 Codex CLI 的 `model/list` 请求，直接向 ChatGPT API 发送 HTTP 请求获取模型：
- URL: `https://chatgpt.com/backend-api/codex/models`（需确认）
- Headers: 与 codex/responses 相同（`User-Agent: codex_cli_rs/0.104.0`）
- 若该 URL 不可用，回退到 `Codex2API` 的 DefaultModels 硬编码

### 2. TDD 脚本：账号额度检测

#### [MODIFY] [test_proxy_tdd.py](file:///e:/learn/cockpit-tools/src-tauri/test_proxy_tdd.py)

- 调用 `chatgpt.com/backend-api/wham/usage` 检查配额
- 扫描所有 codex_accounts，跳过无额度的账号
- 选择有额度的账号进行测试

### 3. 更新 Rust 后端模型列表

#### [MODIFY] [api_proxy.rs](file:///e:/learn/cockpit-tools/src-tauri/src/modules/api_proxy.rs)

更新 `get_codex_model_list()` 为 gpt-5.x 系列（与 Codex2API DefaultModels 同步）

## Verification Plan

1. 运行 `cmd /c python -u test_proxy_tdd.py`
2. 验证模型列表来自远程或更新后的硬编码
3. 验证额度检测正常（跳过 429 账号）
4. 有额度账号成功对话
