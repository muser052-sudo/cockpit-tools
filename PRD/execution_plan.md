# 需求文档 (PRD)

## 1. 修复单文件 JSON 导入失败的问题
- **问题描述**：用户在“添加 Codex 账号”界面的 "Token / JSON" 导入页签中，粘贴单个 Codex 账号的 JSON 字符串（例如 `1wjq1ah0@duckmail.sbs.json` 的内容）时，系统提示“导入失败：无法解析 JSON 内容”。
- **原因分析**：Tauri 后端的 `import_codex_from_json` 解析代码可能仅支持包含数组格式的 JSON（即基于导出格式），或者是对单账号 JSON 对象（非数组）的兼容存在缺陷，导致在解析标准的单账号 JSON `{"type": "codex", "email": "..."}` 时失败。
- **解决方案**：修改 `src-tauri/src/modules/codex_account.rs` 中的 `import_codex_from_json` 命令逻辑。当解析为 `Vec<CodexAccountItem>` 失败时，尝试将其解析为单个 `CodexAccountItem`，如果成功则放入数组中。同时确保结构体能够正确反序列化该 JSON 格式。

## 2. 新增并发批量导入目录中的 JSON 文件功能
- **问题描述**：用户需要一种能够指定本地目录路径（如 `E:\蹬轮子1000\蹬轮子1000`），并并发批量导入该目录下所有 JSON 文件的功能。
- **解决方案**：
  1. **后端 (Rust)**：在 `src-tauri/src/modules/codex_account.rs` 中新增 Tauri command `import_codex_from_dir(dir_path: String)`。该函数将遍历指定目录，并发读取并解析所有 `.json` 文件，如果格式正确则保存为 Codex 账号，最后返回成功导入的账号列表或数量。
  2. **前端与通信 (TypeScript)**：在 `src/services/codexService.ts` 中新增对应接口 `importCodexFromDir(dirPath: string): Promise<CodexAccount[]>`。
  3. **前端 (UI)**：在 `src/pages/CodexAccountsPage.tsx` 的“导入”页签中，添加一个“批量导入目录”的入口。用户可以点击按钮选择一个本地文件夹（使用 `@tauri-apps/plugin-dialog` 的 `open({ directory: true })`），选择后调用 `importCodexFromDir` 进行导入，显示加载状态并在完成后刷新列表并给出成功/失败的提示。
  
## 3. 测试与验证
- 在 `CodexAccountsPage.tsx` 测试单个 JSON 文本的粘贴导入。
- 在 `CodexAccountsPage.tsx` 测试选择文件夹批量导入的功能。
