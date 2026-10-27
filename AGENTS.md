# AGENTS.md — PhAdmin Desktop

## Project

PhAdmin Desktop 是 Phalcon Admin 的主机侧部署控制台，已完全接管原仓库根目录 `php admin` CLI 的功能。

- **状态**：原生部署、健康检查、失败配置回滚、数据库备份/恢复、密钥托管、Nginx/SSL、迁移/seed/cron、SSH 隧道、临时 phpMyAdmin 均已原生化；剩余工作见 [docs/current-state.md](docs/current-state.md)。
- **技术栈**：Tauri 2、Rust、React 19、TypeScript、Vite、SQLite。
- **SSH 登录**：`host/port/user` + 密码 或 私钥（可选 passphrase）；密码/口令按用户要求直接存 SQLite，通过 `SSH_ASKPASS` 非交互登录（Windows OpenSSH 9.5 实测）。
- **远程验收**：vbox 测试机已实测 git/bundle/ftp/配置上传/备份恢复/同步全部/真实 Phalcon 5.23 原生部署；vbox3 已实测全新部署（HTTP 200）、宿主机 Nginx 发布、数据库备份。
- **主数据源**：SQLite（见 [docs/data-model.md](docs/data-model.md)）。
- **不依赖 legacy**：桌面端不调用 `php admin`，不读取 `deploy/projects/*/server.php`；旧仓库仅作历史对照。

## 必读文档

按顺序阅读：

1. [docs/current-state.md](docs/current-state.md) — 最新进度、已验证场景、缺口、下一步
2. [docs/architecture.md](docs/architecture.md) — 架构、模块、任务模型
3. [docs/data-model.md](docs/data-model.md) — SQLite schema、目录、密钥
4. [docs/development.md](docs/development.md) — 命令、目录、开发顺序
5. [docs/roadmap.md](docs/roadmap.md) — 当前阶段与验收
6. [docs/ui-preview.md](docs/ui-preview.md) — UI 设计稿

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

## Architecture Rules

1. **SQLite 是唯一可写事实源**。
2. **多服务器用 `(server_id, project_id)` 的 `deployments` 建模**。禁止按项目名共用生成物/备份/日志目录。
3. **前端不直接访问 SQLite**；只调用 Rust command。
4. **SQL 只在 `src-tauri/src/db/`**；Tauri command 只做校验 + 调 service。
5. **进程参数用 argv 数组**，禁止 `shell: true` 或字符串拼接。
6. **每个 deployment 同一时间只允许一个写任务**；长任务走 `runs` + 日志文件。
7. **服务器 SSH 密码/口令存 SQLite（用户明确要求）；其他密钥（env_vars 密文）仍走 OS keychain**。数据库与备份按敏感文件管理。
8. **不再有 PHP fallback**：桌面端是唯一执行内核；旧 `php admin` 仓库不参与运行。
9. **生成物按 `deployment_id` 隔离**，不写旧仓库 `deploy/projects/`。
10. **危险操作**（reset、db:migrate、db:seed、nginx:remove、cron-rm、ssl）必须二次确认。

## Directory

```text
src/                 React/TS 前端
src-tauri/           Rust/Tauri 后端
docs/                设计/开发文档
AGENTS.md            本文件
```

后续 Rust 模块拆分见 `docs/architecture.md` 第 5 节。不要把所有逻辑塞进 `lib.rs`。

## Current Task

`php admin` 的全部功能已原生接管，桌面端是唯一执行内核：

- **部署**：`handleStartDeploy` → 同步全部 → `prepare_runtime_assets` → 渲染并上传 `.env` / nginx / compose / `src/config/config.php` / 容器配置 → `start_deploy`（确保共享网络存在 → compose up → nginx reload → 可选重启 php → 健康检查）→ 失败恢复部署前配置；
- **容器**：`docker compose` 命令经 `run_command` 执行（ps / logs / restart / up -d）；
- **配置**：`render.rs` 生成、`config_apply.rs` 上传回读、备份/恢复/清理；
- **Nginx（宿主机）**：`publish_nginx_config` / `remove_nginx_config` / `reload_host_nginx` / `read_nginx_log` / `check_server`；
- **SSL**：`enable_ssl`（certbot webroot + 证书链接 + 切换 HTTPS 配置）；
- **数据库**：`backup_database` / `list_database_backups` / `restore_database_backup`（恢复前快照）、`migrate_database`、`seed_database`、`install_backup_cron` / `remove_backup_cron`；
- **项目**：`create_project` / `update_project`（Remote/Local + 部署目录 + 可编辑 APP_NAME/MYSQL_DATABASE + OPEN_PORT/域名/SSL + 代码同步项 + PHP 配置）、`reset_project`（主仓库 + git/bundle 同步目录 `git reset --hard`）、`generate_git_deploy_key`；
- **模板**：`templates.rs` + `templates` 表存覆盖/自定义；内置默认在 `src-tauri/templates/`；`.env`/compose/config.php/三个容器配置统一走 `{{KEY}}` 渲染，自定义模板按 manifest 上传；
- **临时通道**：`start_ssh_tunnel` / `stop_ssh_tunnel` / `list_ssh_tunnels`、`start_phpmyadmin` / `stop_phpmyadmin`；
- **密钥**：`secrets.rs` 走 OS 凭据管理器，`env_vars.rs` 管理环境变量（`/etc` 密码不落库）；
- **空库启动**：`db::open` 只建表、不 seed；服务器由顶栏「服务器管理」新增/修改；项目由「添加项目」创建，列表右键「修改项目」复用同一表单（`create_project` / `update_project`），部署编排待补；
- **前端**：所有动作通过 `phadmin:<action>` 由 `App.tsx` 统一分发，不再拼 `php admin` 命令。

## Testing

- Rust：`cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test`；
- 前端：`npm run typecheck`、`npm run build`；
- SQLite：migration 升级/恢复测试；
- 密钥：`cargo test keychain -- --ignored`、`cargo test secret_round_trips -- --ignored`（真机凭据管理器）；
- 配置渲染：compose/nginx/env 模板单测，不再与 PHP 输出比对；
- 没有人工逐行评审，测试是唯一验收标准；编译通过不等于完成。

## Code Style

- Rust：模块化、`Result` 错误、避免 `unwrap()` 进入生产路径；
- TypeScript：strict，API 调用集中，组件不散落 `invoke`；
- 命名：数据库 snake_case，Rust 类型 PascalCase，TS 类型 PascalCase；
- 注释只写“为什么”，不写显而易见的“是什么”。

## 安全

- WebView CSP 只允许本地资源；
- 不执行任意 shell；参数白名单/校验（终端白名单不含 `php`）；
- 日志脱敏 password/secret/token/key；
- SQLite DB、日志、导出配置不提交 git（见 `.gitignore`）。

## Legacy 下线

桌面端已覆盖全部主流程并通过真实项目回归，`php admin` 依赖已移除：
不再有配置导入、legacy deploy 步骤、PHP fallback 或 `php` 终端白名单。
旧仓库 `D:\demo\code\flutter\library\projects\phalcon-admin` 仅作历史参考，不读取、不调用、不回写。

