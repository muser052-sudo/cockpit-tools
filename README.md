# Antigravity Cockpit Tools

[English](README.en.md) · 简体中文

[![GitHub stars](https://img.shields.io/github/stars/jlcodes99/antigravity-cockpit-tools?style=flat&color=gold)](https://github.com/jlcodes99/antigravity-cockpit-tools)
[![GitHub issues](https://img.shields.io/github/issues/jlcodes99/antigravity-cockpit-tools)](https://github.com/jlcodes99/antigravity-cockpit-tools/issues)
[![License](https://img.shields.io/github/license/jlcodes99/antigravity-cockpit-tools)](https://github.com/jlcodes99/antigravity-cockpit-tools)

一款**专为 Antigravity 客户端用户**设计的桌面端多账号管理工具。

> 本工具需要配合本地 Antigravity 客户端使用，核心功能是**一键切换账号**（切号），帮助用户在多个账号之间快速切换，并降低切号风险，充分利用不同账号的配额。

**功能**：一键切号 · 多账号管理 · 配额监控 · 唤醒任务 · 设备指纹 · 插件联动

**语言**：支持 16 种语言

🇺🇸 English · 🇨🇳 简体中文 · 繁體中文 · 🇯🇵 日本語 · 🇩🇪 Deutsch · 🇪🇸 Español · 🇫🇷 Français · 🇮🇹 Italiano · 🇰🇷 한국어 · 🇧🇷 Português · 🇷🇺 Русский · 🇹🇷 Türkçe · 🇵🇱 Polski · 🇨🇿 Čeština · 🇸🇦 العربية · 🇻🇳 Tiếng Việt

---

## 功能概览

### 一键切号（核心功能）

![Accounts](docs/images/accounts.png)

本工具的核心功能是**一键切换账号**，帮助用户在多个 Antigravity 账号之间快速切换：

- **快捷切换**：一键切换当前使用的账号，无需手动登录登出
- **多种导入方式**：OAuth 授权、Refresh Token、从插件同步、从 JSON 导入
- **批量操作**：批量刷新配额、批量删除、批量导出
- **卡片/列表视图**：两种视图模式，按需切换
- **配额展示**：实时查看各模型剩余配额与重置时间

---

### 唤醒任务

![Wakeup Tasks](docs/images/wakeup-tasks.png)

定时唤醒 AI 模型，提前触发配额重置周期：

- **多种触发模式**：
  - 定时调度：每日 / 每周 / 间隔循环
  - Crontab 高级模式
  - 配额重置触发
- **多模型支持**：同时唤醒多个模型
- **多账号支持**：指定多个账号执行任务
- **时间窗口**：限制任务只在指定时间段内执行
- **历史记录**：查看详细的触发日志和 AI 响应
- **测试运行**：手动测试唤醒效果

---

### 设备指纹

![Fingerprints](docs/images/fingerprints.png)

管理绑定到账号的设备指纹：

- **生成指纹**：随机生成新的设备指纹
- **捕获当前**：捕获当前客户端使用的指纹
- **绑定账号**：将指纹绑定到多个账号
- **导入导出**：支持从旧版工具导入或 JSON 导入
- **批量管理**：批量删除指纹

---

### 配额监控

实时监控各账号的模型配额：

- **多模型展示**：显示常用模型（Claude Sonnet、Gemini Pro、Gemini Flash 等）
- **配额进度条**：可视化剩余配额百分比
- **重置倒计时**：显示配额重置的剩余时间
- **颜色提示**：绿色/黄色/红色三档提醒

---

### 与插件协作

支持与 [Antigravity Cockpit](https://github.com/jlcodes99/vscode-antigravity-cockpit) 插件联动：

- **插件内切号**：在 VS Code 插件内直接调用 Tools 进行快速切号，无需离开编辑器
- **账号同步**：从插件同步已授权的账号到 Tools
- **当前账号同步**：自动同步本地客户端当前使用的账号
- **双向通信**：与插件实时通信

---

## 截图

| 账号总览 | 唤醒任务 |
| :---: | :---: |
| ![Accounts](docs/images/accounts.png) | ![Wakeup Tasks](docs/images/wakeup-tasks.png) |

| 设备指纹 | - |
| :---: | :---: |
| ![Fingerprints](docs/images/fingerprints.png) | - |

---

## 安装

### 下载发行包

从 [Releases](https://github.com/jlcodes99/antigravity-cockpit-tools/releases) 页面下载对应平台的安装包：

- **macOS**：`.dmg` 或 `.app`
- **Windows**：`.msi` 或 `.exe`
- **Linux**：`.deb`、`.rpm` 或 `.AppImage`

---

## 快速开始

1. 下载发行包并安装运行
2. 添加账号（OAuth 授权 / Refresh Token / 从插件同步 / JSON 导入）
3. 在"账号总览"查看配额与状态
4. 创建唤醒任务定时唤醒模型
5. 管理设备指纹（可选）

---

## 开发与构建

### 前置要求

- Node.js v18+
- npm v9+
- Rust（Tauri 运行时）

### 安装依赖

```bash
npm install
```

### 开发模式

```bash
npm run tauri dev
```

### 构建产物

```bash
npm run tauri build
```

---

## 技术栈

- **前端**：React 19 + TypeScript + Vite
- **后端**：Tauri 2 (Rust)
- **国际化**：i18next + react-i18next
- **状态管理**：Zustand
- **样式**：TailwindCSS + DaisyUI

---

## 致谢

- 切号逻辑参考：[Antigravity-Manager](https://github.com/lbjlaq/Antigravity-Manager)

感谢项目作者的开源贡献！如果这些项目对你有帮助，也请给他们点个 ⭐ Star 支持一下！

---

## 支持

- ⭐ [GitHub Star](https://github.com/jlcodes99/antigravity-cockpit-tools)
- 💬 [反馈问题](https://github.com/jlcodes99/antigravity-cockpit-tools/issues)

---

---

## 许可证

[MIT](LICENSE)

---

## 免责声明

本项目仅供个人学习和研究使用。使用本项目即表示您同意：

- 不将本项目用于任何商业用途
- 承担使用本项目的所有风险和责任
- 遵守相关服务条款和法律法规

项目作者对因使用本项目而产生的任何直接或间接损失不承担责任。
