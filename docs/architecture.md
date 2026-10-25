# 架构设计

> 状态：设计基线（静态 UI + SQLite 读写/导入 + 任务执行基础已实现，部署编排未接入）
> 最后更新：2026-10-24
> 关联文档：[data-model.md](data-model.md) · [ui-preview.md](ui-preview.md) · [roadmap.md](roadmap.md)

---

## 1. 目标与边界

### 1.1 目标

PhAdmin Desktop 是 Phalcon Admin 的**主机侧部署控制台**，用 Tauri 2 + Rust + React/TypeScript 实现，最终替代仓库根目录的 `php admin` 命令行。

核心能力：

- 多服务器管理：同一项目可同时部署到 prod / staging / 本地 filesystem；
- 多项目、多部署 `(server, project)` 隔离；
- 部署、升级、重置、Docker、Nginx、数据库备份/迁移、SSL；
- 命令终端、运行历史、日志查看；
- SQLite 管理配置、部署关系与运行记录；
- 现有 PHP `deploy` 作为迁移来源和过渡期回退路径。

### 1.2 非目标

- 不做服务器端常驻 Web 服务；
- 不做 Phalcon Admin 业务后台的替代品；
- 不把数据库备份、日志正文存进 SQLite；
- 不在 WebView 中直接执行 SQL 或任意 shell；
- 不长期维护 PHP 与 Rust 两套并行主逻辑。

---

## 2. 关键决策

| 决策 | 结论 | 原因 |
|---|---|---|
| 桌面技术栈 | Tauri 2 + Rust + React + TypeScript + Vite | 跨平台、体积小、原生进程/文件能力 |
| 主数据源 | SQLite | 多服务器/多项目关系、运行历史、可迁移 |
| 配置来源 | SQLite 为唯一可写事实源 | 避免 PHP 配置与 SQLite 双写 |
| 部署执行 | Rust 原生实现 | 去掉 PHP 运行时依赖，便于并发/取消/结构化状态 |
| PHP deploy | 仅迁移、差分验收、应急回退 | 保留已验证的部署语义 |
| 终端 | PTY + xterm.js | ANSI、交互式命令、可取消 |
| 密钥 | OS keychain（后续） | 不把密码明文放 SQLite 或前端 |

> 重要：当前 `php admin` 的 `--env` 只能一次连接一台服务器，且 `deploy/projects/<项目>/` 的生成物/备份只按项目隔离。桌面端必须用 `(server, project)` 的 `deployment` 模型解决该问题，而不是简单包一层 CLI。

---

## 3. 总体架构

```text
┌─────────────────────────────────────────────────────────────┐
│ Tauri WebView                                                │
│ React + TypeScript + Vite                                    │
│  - 服务器 × 项目矩阵 / 项目 Tab / 终端 / 状态                  │
└──────────────────────────┬──────────────────────────────────┘
                           │ invoke / event
┌──────────────────────────▼──────────────────────────────────┐
│ Rust 后端                                                    │
│  commands/       Tauri invoke 接口                            │
│  db/             SQLite repository、migrations                │
│  config/         配置模型、导入/导出、模板渲染                  │
│  ssh/            SSH exec / SFTP                              │
│  sync/           git / bundle / ftp                           │
│  docker/         compose ps/up/restart/logs                   │
│  nginx/          server block / reload / SSL                  │
│  database/       migrate / seed / backup / cron / proxy       │
│  deploy/         init / upgrade / reset / hooks 编排           │
│  runs/           任务队列、锁、日志、取消                      │
└──────────────────────────┬──────────────────────────────────┘
                           │ 可选、仅过渡期
┌──────────────────────────▼──────────────────────────────────┐
│ PHP deploy fallback（仓库根目录 admin）                       │
│  - 从 SQLite 导出单 deployment 的 PHP 配置后执行               │
│  - 用于迁移验证/应急，不是日常主路径                           │
└─────────────────────────────────────────────────────────────┘
```

### 3.1 分层规则

- **前端不直接访问 SQLite**，只调用 Rust command；
- **Rust repository 是唯一 SQL 入口**；
- **部署动作不依赖 PHP**；
- **PHP fallback 不参与默认执行路径**；
- 所有长任务通过 `runId` 管理，支持流式输出和取消；
- 每个 `deployment` 同一时间只允许一个写任务。

---

## 4. 领域模型

核心实体：

```text
Server 1 ─── n Deployment ─── 1 Project
                  │
                  ├── SyncItem
                  ├── Domain
                  ├── EnvVar
                  └── Run
```

- `Server`：SSH 目标或本地 filesystem；
- `Project`：代码仓库/项目逻辑；
- `Deployment`：项目在某台服务器上的具体部署（远程路径、端口、SSL、同步项、env）；
- `Run`：一次命令执行记录。

详细表结构见 [data-model.md](data-model.md)。

### 4.1 多服务器隔离

所有本地状态按 `deployment_id` 隔离：

```text
<app_data_dir>/
  phadmin.db
  deployments/
    <deployment_id>/
      generated/        # .env / compose / nginx / php.ini / config.php
      backups/          # 数据库备份
      logs/             # 命令日志
```

禁止再把同一项目的 prod/staging 生成物写到同一个 `deploy/projects/<项目>/` 目录。

---

## 5. Rust 模块建议

```text
src-tauri/src/
  lib.rs
  main.rs
  commands/
    servers.rs
    projects.rs
    deployments.rs
    runs.rs
    settings.rs
  db/
    mod.rs
    migrations/
    servers.rs
    projects.rs
    deployments.rs
    runs.rs
  config/
    model.rs
    render.rs
    legacy_import.rs
    legacy_export.rs
  ssh/
    mod.rs
    exec.rs
    sftp.rs
  sync/
    git.rs
    bundle.rs
    ftp.rs
  docker/
    compose.rs
  nginx/
    template.rs
    ssl.rs
  database/
    backup.rs
    migrate.rs
    proxy.rs
  deploy/
    init.rs
    upgrade.rs
    reset.rs
    hooks.rs
  runs/
    manager.rs
    log.rs
    lock.rs
  error.rs
```

实现优先级：

1. `db` + `commands`：服务器/项目/部署 CRUD、运行历史；
2. `ssh` + `runs`：连接、exec、流式输出、取消；
3. `deploy/init` + `upgrade`：核心部署；
4. `docker` + `nginx`；
5. `database` + SSL；
6. `config/legacy_*`：导入/导出/回退。

**不要一次性写满所有模块。** 每步先落 SQLite 和测试，再接 UI。

---

## 6. 进程与任务模型

```text
frontend invoke("run_command", { deploymentId, command, args })
  → Rust 创建 Run 记录（queued）
  → 获取 deployment 锁
  → spawn 系统命令 / SSH 会话（running）
  → 通过 event 推送 stdout/stderr
  → 完成：写 exit_code/status/ended_at（succeeded/failed/canceled）
  → 释放锁
```

规则：

- 命令参数使用 argv 数组，不拼 shell 字符串；
- 每个任务记录 `runId`、`deploymentId`、命令、参数、开始/结束时间、退出码、日志路径；
- 取消时终止进程树（Windows `taskkill /T`，Unix 先 SIGINT 再 SIGTERM）；
- `db:proxy` 等阻塞命令进入后台任务，UI 显示“运行中”并提供停止；
- 同一 `deployment` 写任务串行；只读任务可并行。

---

## 7. SQLite 与文件的分工

| 数据 | 存放 |
|---|---|
| 服务器/项目/部署/同步项/域名/env | SQLite |
| 密码/密钥引用 | SQLite 存引用，实际值进 OS keychain |
| 运行记录 | SQLite |
| 命令输出 | 文件（`logs/<runId>.log`），DB 存路径 |
| 生成的 `.env`/compose/nginx/config | `<app_data>/deployments/<id>/generated/` |
| 备份 | `<app_data>/deployments/<id>/backups/` |
| 设置 | SQLite `settings` 或 Tauri Store |
| PHP fallback 配置 | 导出到临时目录，不写回 SQLite |

SQLite 不存大文本日志、备份二进制、生成物全文。

---

## 8. 与 PHP deploy 的关系

### 8.1 迁移

首次启动或手动触发：

```text
php admin config:export
  → JSON
  → Rust legacy_import
  → SQLite servers/projects/deployments
```

`config:export` 已输出合并后的配置，是现成的迁移入口，不需要在 Rust 解析 PHP 源码。

### 8.2 回退

过渡期需要应急时：

```text
SQLite
  → 导出单 deployment 的 server.php / 项目 server.php
  → php admin app:<项目> ...
```

限制：

- 现有 PHP CLI 是单服务器、单项目配置模型；
- 一次只能导出/操作一个 deployment；
- 回退路径不保证多服务器并发；
- 导出文件含敏感信息，必须放在 gitignored 临时目录，用完清理。

### 8.3 下线条件

满足以下条件后，PHP deploy 可下线：

- 桌面端覆盖 init/upgrade/reset/dc/nginx/db/ssl 全部主流程；
- 至少一个真实项目在 prod + staging 完成回归；
- 导入/导出与差分测试通过；
- 关键路径有自动化测试；
- 有明确的回滚方案和版本记录。

---

## 9. 安全

- SQLite 不存 SSH 密码明文；使用 OS keychain（Windows Credential Manager / macOS Keychain / Linux Secret Service）；
- 日志脱敏 `password`、`secret`、`token`、`key`；
- WebView CSP 限制本地资源，不加载远程脚本；
- Rust 侧对参数做白名单/校验，不能只靠前端；
- 项目名 `[a-zA-Z0-9_-]+`；
- 删除/重置/迁移/SSL 等操作二次确认；
- 备份 SQLite 时使用 `VACUUM INTO`，迁移前自动备份。

---

## 10. 验证策略

因为没有人工逐行评审，**自动化是唯一验收标准**：

- Rust：`cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test`；
- 前端：`npm run typecheck`、`npm run build`；
- SQLite：每个 migration 有升级/恢复测试，迁移前备份；
- 配置渲染：与 PHP 生成的 `.env`/compose/nginx/config.php 做 golden/diff；
- 集成：Docker 起 sshd + compose 栈，跑真实 init/upgrade；
- 回退：CI 定期用导出的 PHP 配置执行 `php admin` 冒烟；
- 不允许“只编译通过”就认为完成。

---

## 11. 当前状态

- 已完成 Tauri 2 + React + TypeScript 脚手架；
- 已完成静态 UI 原型（概览、部署、容器、Nginx、数据库、日志、弹窗）；
- 已接入 SQLite 只读层：启动自动建 `servers/projects/deployments` 三表并 seed，`load_workspace` command 返回服务器/项目/部署；
- 已实现 legacy 导入：`import_legacy_config` 解析 `php admin config:export` JSON，写入 servers/projects/deployments/domains/env_vars/sync_items/deployment_settings；
- 已实现基础任务执行：`run_command` 支持本地 filesystem 与系统 `ssh`（BatchMode + key），stdout/stderr 通过 Tauri event 流式推送，写入 `runs`；
- 终端支持白名单命令（`docker/php/git/ssh/ls/pwd/whoami/composer`），前端 argv 解析，不经过 shell；
- 已实现 `list_runs` 任务历史与 `cancel_run`；终端「任务」面板可查看/取消；
- 任务 stdout/stderr 同时写入 `%APPDATA%/cn.gu19.phadmin.desktop/logs/<run_id>.log`，`read_run_log` 可在任务面板查看；
- 已实现 `get_containers`：执行 `docker compose ps --format json` 并解析为容器列表；Containers 面板打开时自动刷新；
- 容器全部启动/重启/停止、单服务日志/重启均通过 `run_command` 真实执行，完成后自动刷新；
- Overview 的容器统计来自 `get_containers`，最近活动来自 `list_runs`（Tauri 模式）；备份仍为 mock；
- Deploy 面板「开始升级」已接入 `start_deploy` 编排：升级 → compose up → Nginx reload → 可选重启 PHP；任一步失败中止；同一 deployment 禁止并发部署；
- 步骤 run 通过 `deploy_run_id` / `step_key` / `step_title` 关联 `deploy_runs`，任务面板可查看步骤；
- 任务面板增加最近部署父记录（deploy_runs）展示；
- Deploy 面板增加“同步全部”：按 sync_items 顺序执行 git / bundle / ftp，git 等待 run 结束；
- Deploy 面板通过 `preview_deploy_plan` 展示真实部署步骤与命令；
- 「预览配置」打开只读 ConfigModal，查看 sync_items 与 config/hooks/docker/router/nginx JSON；
- 新增 `sync_git_item`：本地 filesystem / SSH 的 git 同步项支持 clone / pull，执行进入 runs 与日志；同步项级锁防止重复执行；
- 新增 `sync_bundle_item`：本地 filesystem / SSH 的 bundle 同步项使用 `git bundle` + clone/fetch/merge，SSH 通过 scp 上传；执行结果写入 runs 与日志；
- 新增 config.php 预览与生成：将 deployment_settings.config_json 转 PHP 数组字面量并套用模板，纳入 generated 与远程上传；
- 新增 `sync_ftp_item`：本地目录递归复制 / SSH `scp -r`，本地支持 excludes；
- ftp/SFTP 基础同步已接入；`.env` / Nginx / docker-compose / config.php 可预览并写入本地 generated 目录，支持变更状态检测；生成文件可上传远程、写 `.bak-<millis>` 备份、检查差异、恢复并按可配置数量清理旧备份（vbox 已实测）；
- 下一步：远程配置写入产品策略 / 部署计划接入原生同步。

参见 [roadmap.md](roadmap.md)。
