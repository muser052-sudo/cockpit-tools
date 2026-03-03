# 批量导入目录进度条显示及并发处理功能需求文档 (PRD)

## 目标 (Goal)
在 Codex 账号的 "批量导入目录" 功能中，增加并发处理和明显的进度反馈机制。解决用户在导入包含大量 JSON 文件的目录时效率奇慢且体验不佳的问题。
具体要求：
- **高并发处理**：总共数十条到成百上千条的 JSON 文件通过 20-50 个并发线程同时处理解析和校验。
- **进度详情显示**：
  - 显示共有多少个 JSON 文件、已经导入的数量、剩余数量
  - 实时更新百分比进度（%）
  - 已经处理的时间（已流逝时间）
  - 根据当前速率预估的剩余时间（ETA）

## 提议的设计修改 (Proposed Changes)

### 1. 后端 (Rust - Tauri)
引入 `tokio` 异步流 (`StreamExt`) 实现 `buffer_unordered` 控制 20~50 的并发度，并将静默导入的过程转变为带 `AppHandle` 事件派发的进度状态推送。
- #### [MODIFY] [codex.rs](file:///e:/learn/cockpit-tools/src-tauri/src/commands/codex.rs)
  修改 `import_codex_from_dir` 命令：增加 `app: tauri::AppHandle` 注入。并将其修改为 `async` 函数。
- #### [MODIFY] [codex_account.rs](file:///e:/learn/cockpit-tools/src-tauri/src/modules/codex_account.rs)
  修改 `import_codex_from_dir` 函数定义接收 `app: &tauri::AppHandle` 并异步执行。
  增加一个新的数据结构 `ImportProgressPayload` 暴露进度字段：总数(`total`)、当前数(`current`)、成功数(`success`)、失败数(`failed`)和当前处理文件(`current_file`)。
  - **高并发具体实现**：由于更新 `CodexAccountIndex` 目前是单线程写锁定的过程，我们可以先收集和解析目录下所有的 JSON 后缀文件，得出 `total`。
  - 使用 `tokio::task::spawn_blocking` 或使用 `futures::stream::iter(files).map(|file| async { ... }).buffer_unordered(20)` 控制并发。在解析文件结构获取了扁平的账号数据后，加锁执行写入 `accounts.json`；每完成一条则原子增加计数，同时通过 Tauri 触发一次 `codex-import-progress` 事件给前端。
  
### 2. 前端 (React API/UI)
接收 Tauri 的事件推送，并运用 React 状态驱动美观的进度条组件展示。
- #### [MODIFY] [CodexAccountsPage.tsx](file:///e:/learn/cockpit-tools/src/pages/CodexAccountsPage.tsx)
  引入 `useState` 管理 `importProgress`，对象包含时间戳起始点 `startTime` 和事件 payload 数据。
  在发出 `codexService.importCodexFromDir` 之前注册对 `codex-import-progress` 的 Tauri `listen`，在结束时 `unlisten`。
  界面部分：当 `importProgress` 存在且 `total > 0` 时，动态渲染一个高亮进度框：
  - `<progress>` 元素或带有自适应宽度的 CSS 动画条
  - 展示文案区：`处理进度: 已倒入 / 总共 (%百分比)`
  - 展示时间信息：已用时间（精确到秒），及利用当前已完成进度比率预测的剩余耗时（ETA）。

## 验证计划 (Verification Plan)
- **静态验证**：通过 `npm run typecheck` 以及 `cargo check` 确保事件的派发和接收两端类型安全合法。
- **并发及 UI 验证**：复制一组测试用的 JSON 导入目录，设定 20-50 的大小控制流并行限制进行真实导入，确保 Tauri 的事件未造成死锁，前端实时呈现的数字随着进度条有条不紊地增长，中英文文案能够友好显示。
