# xtshell

仿 PrivateShell 的 SSH / Telnet / Serial 终端管理工具，基于 Tauri 2 + Vue 3 + TypeScript，终端渲染使用 xterm.js。

## 功能

- **多协议**：SSH（密码 / 私钥 + 口令，兼容老式设备的旧 KEX/加密算法）、Telnet（IAC 协商过滤 + 自动登录）、Serial（串口参数可配置）
- **会话管理**：多会话并行、标签页切换，连接配置以目录树保存（可拖拽整理）
- **凭据安全**：密码与私钥口令用 AES-256-GCM 加密落盘；SSH 主机密钥经 known_hosts 校验，首次连接弹框确认指纹
- **SFTP**：在既有 SSH 会话上开 subsystem 通道，支持浏览、上传、下载、新建目录、重命名、递归删除
- **ZMODEM**：支持远端 `sz`/`rz` 双向传输，冲突时可覆盖 / 跳过 / 自动改名
- **终端体验**：GBK/UTF-8 等编码切换、全局与单会话字体/配色、会话日志（自动剥离 ANSI）、复制粘贴
- **MCP**：本地 `http://127.0.0.1:9567/mcp`，供外部 agent 创建/操作会话（菜单「工具 → MCP」查看配置）

## 开发

需要 Node 22+、pnpm、Rust stable（Windows 上还需 MSVC 工具链与 WebView2）。

```bash
pnpm install
pnpm tauri dev     # Vite (1420) + Rust 后端
pnpm build         # vue-tsc 类型检查 + vite 构建
cd src-tauri && cargo check && cargo test
```

打包本机平台安装包：

```bash
pnpm tauri build
```

跨平台发布由 `.github/workflows/release.yml` 在推送 `v*` 标签时触发（Windows / macOS / Linux 矩阵）。

## 目录结构

```
src/                 Vue 前端（Pinia store、xterm 封装、UI 组件）
src-tauri/src/
  protocol/          SSH / Telnet / Serial 三种 TerminalStream 实现
  session/           SessionManager + 输出环形缓冲
  profile/           连接配置持久化与加密
  sftp/              SFTP 会话与文件操作
  zmodem.rs          ZMODEM 收发状态机
  mcp/               MCP 服务（rmcp + axum）
  commands.rs        前端 invoke 命令
src-tauri/vendor/    为兼容老式交换机打过补丁的 russh / ssh-encoding
```

更多架构与开发约定见 `AGENTS.md`，设计背景见 `设计方案.md`。
