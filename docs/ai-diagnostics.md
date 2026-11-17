# AI 诊断命令（phadmin-cli）

`phadmin-cli` 是 PhAdmin Desktop 的只读诊断命令行，供本机 Claude Code、Codex、Cursor 等能执行 shell 的 AI 调用。

## 前提

- 桌面端至少启动过一次，SQLite 数据库已存在；
- CLI 与桌面端读取同一数据库：
  - Windows: `%APPDATA%\cn.gu19.phadmin.desktop\phadmin.db`
  - 可用 `PHADMIN_DB` 覆盖路径；
- CLI 执行 SSH 时复用桌面端保存的密码/私钥配置；
- 仅读取，不修改服务器、数据库或项目文件。

## 构建

```bash
cargo build --manifest-path src-tauri/Cargo.toml --bin phadmin-cli
```

产物：

```text
src-tauri/target/debug/phadmin-cli.exe
```

仓库根目录的 `phadmin-cli.cmd` 会自动选择 debug/release 产物。

## 命令

```powershell
.\phadmin-cli.cmd projects

# 给 AI 用的结构化输出
.\phadmin-cli.cmd error yihe --format json

# 人看的 markdown
.\phadmin-cli.cmd diagnose yihe

# 导出报告
.\phadmin-cli.cmd export yihe --output yihe-diagnose.md

# 单独查看某类日志
.\phadmin-cli.cmd logs yihe --source php
.\phadmin-cli.cmd logs yihe --source app --output app.log
```

`error` 是 `diagnose` 的别名。

## JSON 内容

- 项目、服务器、部署 ID；
- HTTP 状态；
- 容器状态；
- 本地规则发现（1045 / Parse error / Redis DNS / 502 / 容器退出等）；
- 最近任务；
- 应用/php/nginx/nginx-error 日志（已截断、已脱敏）；
- 采集失败项。

规则只分析最近 120 分钟日志；完整日志在 `logs` 字段中。

## 推荐给 AI 的指令

在项目目录中对 Claude Code / Codex 说：

```text
线上项目 yihe 出现异常。请先执行：
.\phadmin-cli.cmd error yihe --format json

根据 JSON 中的 findings、httpStatus、containers、logs 定位原因。
需要更多信息时可以执行：
.\phadmin-cli.cmd logs yihe --source php
.\phadmin-cli.cmd containers yihe

不要自动执行修复命令，先给出证据、根因和分步修复方案。
```

如果 AI 在 PowerShell 中运行，注意使用 `.\`；如果已经将 CLI 加入 PATH，可直接用 `phadmin-cli`。

## 安全边界

- 输出前脱敏 `password/passwd/token/secret/authorization/cookie/Bearer`；
- 不输出 `.env` 内容、SSH 密码、私钥；
- CLI 没有部署、重启、写文件、执行 SQL 的命令；
- 修复仍由用户在桌面端或终端确认后执行。

## 当前限制

- AI 必须能执行本地命令；网页版聊天窗口不适用，可先用 `export` 导出报告再粘贴；
- CLI 尚未由安装包自动加入 PATH；
- Windows 发布包需要把 `phadmin-cli.exe` 作为 sidecar 一并分发。
## 桌面端集成

- 日志页「故障诊断」卡片提供：
  - 复制 AI 指令；
  - 导出 JSON 报告；
  - 安装 CLI（复制到应用数据目录，提示词使用完整路径）。
- 安装 CLI 后，AI 指令会使用类似：
  ```text
  "C:\Users\<user>\AppData\Roaming\cn.gu19.phadmin.desktop\bin\phadmin-cli.exe" error yihe --format json
  ```
## 发布包 sidecar

- `tauri.conf.json` 的 `bundle.externalBin` 包含 `binaries/phadmin-cli`；
- 构建前脚本 `scripts/build-cli-sidecar.mjs` 会编译 release CLI 并复制为 `binaries/phadmin-cli-<target-triple>.exe`；
- 开发启动时使用 `--debug` 版本，保证 Tauri dev 能通过 sidecar 校验；
- `src-tauri/binaries/` 为生成目录，不提交 git。
## PATH 设置

- 设置页会检查用户 PATH 是否包含：
  `%APPDATA%\cn.gu19.phadmin.desktop\bin`
- 添加时直接修改 `HKCU\Environment\Path`，保留原有的 `REG_SZ/REG_EXPAND_SZ` 类型，不使用 setx，避免 1024 字符截断；
- 修改后广播 `WM_SETTINGCHANGE`，新开的终端可识别 `phadmin-cli`。
