# AGENTS.md — PhAdmin Desktop

## Project

PhAdmin Desktop 是 Phalcon Admin 的主机侧部署控制台，目标替代仓库根目录的 `php admin` CLI。

- **状态**：Tauri 2 脚手架 + 静态 UI + SQLite 只读/导入层已完成；SSH/部署逻辑尚未接入。
- **技术栈**：Tauri 2、Rust、React 19、TypeScript、Vite、SQLite。
- **主数据源**：SQLite（见 [docs/data-model.md](docs/data-model.md)）。
- **legacy 引擎**：`php admin` 仅用于迁移、差分验收、过渡期回退，不是桌面端默认执行内核。
- **旧部署配置**：`D:\demo\code\flutter\library\projects\phalcon-admin\deploy\projects\vbox*` 只是历史测试配置，不作为生产配置直接导入。

## 必读文档

按顺序阅读：

1. [docs/architecture.md](docs/architecture.md) — 架构、模块、任务模型
2. [docs/data-model.md](docs/data-model.md) — SQLite schema、目录、密钥
3. [docs/development.md](docs/development.md) — 命令、目录、开发顺序
4. [docs/roadmap.md](docs/roadmap.md) — 当前阶段与验收
5. [docs/ui-preview.md](docs/ui-preview.md) — UI 设计稿

文档和代码冲突时，先确认设计意图；设计变更先改文档再改代码。

## Commands

```bash
npm install
npm run dev
npm run tauri dev
npm run typecheck
npm run build
npm run tauri build
```

Rust：

```bash
cd src-tauri
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo check
```

legacy 参考（只读）：

```powershell
cd D:\demo\code\flutter\library\projects\phalcon-admin
php admin config:export
```

## Architecture Rules

1. **SQLite 是唯一可写事实源**。不要把 PHP `server.php` 当第二主库。
2. **多服务器用 `(server_id, project_id)` 的 `deployments` 建模**。禁止按项目名共用生成物/备份/日志目录。
3. **前端不直接访问 SQLite**；只调用 Rust command。
4. **SQL 只在 `src-tauri/src/db/`**；Tauri command 只做校验 + 调 service。
5. **进程参数用 argv 数组**，禁止 `shell: true` 或字符串拼接。
6. **每个 deployment 同一时间只允许一个写任务**；长任务走 `runs` + 日志文件。
7. **密码/密钥不落 SQLite 明文**；使用 OS keychain，DB 只存引用。
8. **PHP fallback 单向导出**，不双向同步，不默认调用。
9. **生成物按 `deployment_id` 隔离**，不写旧仓库 `deploy/projects/`。
10. **危险操作**（reset、db:migrate、db:seed、nginx:remove、cron-rm）必须二次确认。

## Directory

```text
src/                 React/TS 前端
src-tauri/           Rust/Tauri 后端
docs/                设计/开发文档
AGENTS.md            本文件
```

后续 Rust 模块拆分见 `docs/architecture.md` 第 5 节。不要把所有逻辑塞进 `lib.rs`。

## Current Task

Phase 1（SQLite + 只读/导入）已完成：

- `rusqlite` + `servers` / `projects` / `deployments` 三表 + 启动 seed；
- `domains` / `env_vars` / `sync_items` / `deployment_settings` / `runs` 表；
- `load_workspace` 只读 command；
- `import_legacy_config`：解析 `php admin config:export` JSON，设置页可导入；
- `run_command`：本地 filesystem / 系统 `ssh` 执行，stdout/stderr 事件 + `runs`；
- 终端白名单命令：`docker/php/git/ssh/ls/pwd/whoami/composer`，前端 argv 解析，不经过 shell；
- `list_runs` / `cancel_run`：终端「任务」面板查看历史并取消；
- 运行输出持久化到 `logs/<run_id>.log`，`read_run_log` 供任务面板查看；
- `get_containers`：`docker compose ps --format json` 解析，Containers 面板自动刷新；
- 容器启动/停止/重启/日志通过 `run_command` 执行；
- Overview 容器统计/最近活动接入 `get_containers` / `list_runs`；备份仍为 mock；
- Deploy「开始升级」调用 `start_deploy`：升级 → compose up → Nginx reload → 可选重启 PHP；失败中止，同 deployment 防并发；
- 步骤 run 关联 `deploy_runs`，任务面板显示步骤；
- Deploy 面板读取真实 `sync_items` / `deployment_settings`；
- 本地 filesystem / SSH git 同步项支持 clone / pull（`sync_git_item`）；同步项级锁；
- 本地 / SSH bundle 同步支持 git bundle（`sync_bundle_item`）；ftp/SFTP 基础同步（`sync_ftp_item`）；
- 配置预览弹窗只读展示 sync items 与 settings JSON；
- rendered .env / nginx / compose 预览，并可写入本地 generated 目录（不写远程）。

下一步：

- ftp / SFTP 同步 / 配置渲染。

不要提前实现部署/SSH/Docker 写操作。先把数据层和只读链路打通。

## Testing

- Rust：`cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test`；
- 前端：`npm run typecheck`、`npm run build`；
- SQLite：migration 升级/恢复测试；
- 配置渲染：与 PHP 输出做 golden/diff；
- 没有人工逐行评审，测试是唯一验收标准；编译通过不等于完成。

## Code Style

- Rust：模块化、`Result` 错误、避免 `unwrap()` 进入生产路径；
- TypeScript：strict，API 调用集中，组件不散落 `invoke`；
- 命名：数据库 snake_case，Rust 类型 PascalCase，TS 类型 PascalCase；
- 注释只写“为什么”，不写显而易见的“是什么”。

## 安全

- WebView CSP 只允许本地资源；
- 不执行任意 shell；参数白名单/校验；
- 日志脱敏 password/secret/token/key；
- SQLite DB、日志、导出配置不提交 git（见 `.gitignore`）。

## Legacy 下线

在 desktop 覆盖全部主流程并通过真实项目回归前，不要删除/修改 legacy `deploy`。
PHP fallback 只用于迁移和应急，不参与日常部署。
