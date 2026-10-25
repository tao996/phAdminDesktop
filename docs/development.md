# 开发指南

> 状态：脚手架阶段
> 关联：[architecture.md](architecture.md) · [data-model.md](data-model.md) · [roadmap.md](roadmap.md)

---

## 1. 环境要求

| 依赖 | 版本/说明 |
|---|---|
| Node.js | 22+（当前验证 24.11） |
| npm | 10+（当前 11.6） |
| Rust | stable（当前 1.96） |
| Tauri CLI | 2.x（随 `@tauri-apps/cli` 安装） |
| Windows | WebView2 Runtime、MSVC Build Tools |
| macOS | Xcode Command Line Tools |
| Linux | WebKitGTK、libappindicator 等 Tauri 依赖 |

PHP **不是桌面端运行的必需依赖**。只有使用 legacy `php admin` 回退/导入时才需要。

---

## 2. 常用命令

在项目根目录执行：

```bash
npm install                 # 安装前端依赖
npm run dev                 # 只启动 Vite（浏览器调试）
npm run tauri dev           # 启动 Tauri 桌面窗口
npm run typecheck           # TypeScript 类型检查
npm run build               # 前端构建
npm run tauri build         # 打包桌面安装包
```

Rust 侧：

```bash
cd src-tauri
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo check
```

> `npm run tauri dev` 首次会编译 Rust，耗时较长；后续增量编译较快。

---

## 3. 目录结构

```text
phAdminDesktop/
├── src/                     # React + TypeScript 前端
│   ├── App.tsx              # 当前仅脚手架占位
│   ├── App.css
│   └── main.tsx
├── src-tauri/               # Rust / Tauri 后端
│   ├── src/
│   │   ├── lib.rs
│   │   └── main.rs
│   ├── capabilities/
│   ├── icons/
│   ├── Cargo.toml
│   └── tauri.conf.json
├── docs/                    # 设计/开发文档
│   ├── architecture.md
│   ├── data-model.md
│   ├── development.md
│   ├── roadmap.md
│   ├── ui-preview.md
│   └── adr/
├── AGENTS.md                # AI 代理开发约定
├── package.json
└── vite.config.ts
```

后续 Rust 模块拆分见 [architecture.md](architecture.md#5-rust-模块建议)。

---

## 4. 开发顺序

不要先写 UI 再补后端。按以下顺序：

1. **SQLite schema + migrations**
   - 先落 `servers/projects/deployments`；
   - 再落 `sync_items/domains/env_vars/runs`；
   - 每个 migration 可重复执行、可恢复。
2. **Rust repository + Tauri commands**
   - `list_servers` / `list_projects` / `list_deployments`；
   - 前端只通过 command 取数据。
3. **只读 UI**
   - 服务器列表、项目 × 服务器矩阵；
   - 不接部署动作。
4. **运行任务**
   - `run_command`、流式事件、取消、日志；
   - 先用只读命令（`docker compose ps`、日志）验证。
5. **部署核心**
   - `init` / `upgrade`；
   - 与 PHP deploy 做差分。
6. **数据库/Nginx/SSL**
   - 高风险动作最后做，必须确认 + 测试。

---

## 5. 代码约定

### Rust

- 业务逻辑放 `src-tauri/src/` 的模块，不塞进 `lib.rs`；
- 所有 Tauri command 只做参数校验 + 调 service，不写 SQL/不拼 shell；
- SQL 只在 `db/`；
- 进程执行使用 argv 数组，禁止 `shell: true` 拼接；
- 长任务通过 `runs::manager` 统一管理，不在 command 里阻塞；
- 错误用 `thiserror`/`anyhow`（待定）统一转换为前端可读错误。

### TypeScript / React

- 不直接访问 SQLite；
- API 调用集中在 `src/api/`，组件不散落 `invoke`；
- 类型与 Rust serde 结构保持一致；
- 终端为 React 自绘 xterm 风格，不使用 xterm.js；
- 状态管理先用 React hooks；复杂后再引入 store。

### SQLite

- WAL + foreign_keys + busy_timeout；
- 所有多表写入用事务；
- 不存密码/密钥明文；
- 不存大日志/备份 BLOB。

---

## 6. 与 legacy deploy 协作

legacy 仓库（本机）：

```text
D:\demo\code\flutter\library\projects\phalcon-admin
```

当前可用的迁移入口：

```bash
cd D:\demo\code\flutter\library\projects\phalcon-admin
php admin config:export
```

输出 `deploy/.cache/config.json`，桌面端导入后映射为 SQLite。

规则：

- **只读导入**：不要修改 legacy 仓库的 `server.php`/`projects/*`；
- **单向导出**：需要回退时，导出到桌面端 `exports/`，不写回 legacy；
- **不双向同步**：检测到 legacy 文件变化时提示重新导入；
- **不要删除 legacy**：直到 desktop 稳定并完成验收。

---

## 7. 调试

### 前端

- `npm run dev` 用浏览器打开 `http://localhost:1420`，可先用 mock 数据；
- 但 `invoke` 只在 Tauri 窗口可用，浏览器模式需要 mock API。

### Rust

```bash
RUST_LOG=debug npm run tauri dev
```

Windows PowerShell：

```powershell
$env:RUST_LOG="debug"; npm run tauri dev
```

### SQLite

- DB 在 `<app_data_dir>/phadmin.db`；
- 可用 DB Browser for SQLite / `sqlite3` 查看；
- 不要提交 DB 文件。

---

## 8. 测试要求

- 每个 Rust service 至少单测；
- SQLite migration 要有 upgrade + restore 测试；
- 配置渲染要有 golden test；
- 关键部署流程用 Docker/sshd 集成测试；
- 前端至少 `typecheck` + `build`；
- CI 未通过不得合并。

没有人工逐行评审，**测试就是评审**。

---

## 9. 当前任务

Phase 0 已完成：

- Tauri 2 + React + TS 脚手架；
- 窗口/产品名/标识；
- 静态 UI 原型（mock 数据、页面切换、弹窗、模拟终端）；
- 文档基线。

Phase 1 已完成：

- SQLite `servers/projects/deployments` 三表 + 启动 seed；
- `load_workspace` 只读 command；
- UI 的服务器/项目/部署从 SQLite 加载；
- legacy `config:export` JSON 导入（设置 → 导入 legacy 配置）；
- `domains/env_vars/sync_items/deployment_settings/runs` 表；
- `run_command` 基础执行：本地 filesystem / 系统 `ssh`，stdout/stderr 事件 + `runs` 落库；
- `list_runs` 任务历史 + `cancel_run`；终端「任务」面板；
- 运行输出写入 `logs/<run_id>.log`，任务面板可查看；
- `get_containers` 解析 `docker compose ps --format json`，Containers 面板自动刷新；
- 容器启动/停止/重启/日志已接入真实命令；
- Overview 容器统计使用 `get_containers`，最近活动使用 `list_runs`；
- 终端支持白名单命令（argv 解析，不经过 shell）；
- Deploy 面板「开始升级」调用 `start_deploy`，按步骤执行升级 → compose up → Nginx reload → 可选重启 PHP；
- 任一步失败中止；同一 deployment 禁止并发部署；步骤 run 关联 `deploy_runs`；
- 同步项级锁防止重复执行；ftp 锁在查询失败时不会残留；
- 配置预览：.env / nginx / compose / config.php；本地 generated 写入与状态检测；
- 配置上传：逐文件回读校验，覆盖前 `.bak-<millis>`；支持备份列表、恢复、可配置保留数量清理；
- 模板 `env.example` / `docker-compose.yaml` / `config.php.template` 与 legacy 逐行对齐；
- “同步全部”按 sync_items 顺序执行，git 等待 run，失败中止；
- “原生部署（实验）”：前端 preflight 同步全部 + 生成/上传配置，后端只执行 compose up / nginx reload / 可选重启 PHP，跳过 legacy upgrade；
- 原生部署 vbox 全链路已跑通（compose up + nginx reload + 容器健康）；
- 原生部署 preflight 会从示例补齐 docker/nginx/php/mysql 运行配置与日志目录；
- 真实 Phalcon 项目在本地 Docker 原生部署成功，HTTP 200；
- `.env` 密码取自 `env_vars` 的 MYSQL_PASSWORD / REDIS_PASSWORD；
- SSH 远程命令统一作为单个 shell 字符串传递，修复 `cd` 未生效导致 compose 找不到配置文件的问题。
- 部署历史展示最近 deploy_runs；Deploy 面板展示真实计划；
- vbox 已验证远程 git / bundle / ftp / 配置上传 / 备份恢复 / 同步全部。

下一步：

- 将“同步全部 + 配置上传”接入部署启动流程；
- 生产环境远程写入策略与回滚入口；
- PHP 回退验收。

详见 [roadmap.md](roadmap.md)。
