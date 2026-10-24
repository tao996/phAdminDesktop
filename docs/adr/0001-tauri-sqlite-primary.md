# ADR-0001：Tauri 原生 + SQLite 主数据，PHP deploy 作为回退

- 状态：已接受
- 日期：2026-10-24
- 决策者：项目所有者

## 背景

当前部署工具是仓库根目录的 `php admin`，基于 `deploy/src` 纯 PHP 引擎，已有 init/upgrade/docker/nginx/db/ssl 等能力。其限制：

- 一次命令只面向一台服务器（`--env` 选择 `server.<env>.php`）；
- 生成物、备份、日志按 `deploy/projects/<项目>/` 隔离，而不是按 `(server, project)` 隔离；
- 同一项目的 prod/staging 会互相覆盖本地状态；
- 没有服务器注册表、运行历史、并发任务和结构化输出。

项目决定用 Tauri 做桌面端，并在桌面端稳定后下线 PHP deploy。开发方式以 AI 编写为主，不存在技术栈学习成本。

## 决策

1. 桌面端使用 **Tauri 2 + Rust + React + TypeScript**；
2. **SQLite 作为唯一可写事实源**，管理 server/project/deployment/run；
3. 部署逻辑用 **Rust 原生实现**，不再默认调用 `php admin`；
4. 现有 PHP deploy 保留为：
   - 迁移来源（`config:export`）；
   - 差分验收 oracle；
   - 过渡期应急回退；
5. 多服务器通过 `deployments(server_id, project_id)` 建模，所有生成物/备份/日志按 `deployment_id` 隔离；
6. 密码/密钥进入 OS keychain，SQLite 只存引用；
7. PHP 回退采用**单向导出**，不双向同步。

## 备选方案

### A. Tauri UI + 继续调用 `php admin`

- 优点：复用现有引擎，最快。
- 缺点：无法解决 `(server, project)` 状态隔离；多服务器并发会互相覆盖；仍依赖 PHP 运行时；无法提供稳定的结构化状态。
- 结论：只适合过渡，不作为目标架构。

### B. Tauri 一次重写全部功能

- 优点：目标清晰。
- 缺点：一次性风险高，行为难以验证。
- 结论：拒绝 big-bang，采用分阶段实现。

### C. 继续增强 PHP CLI + Web UI

- 优点：语言统一。
- 缺点：桌面体验、进程管理、SQLite/多服务器状态仍需额外方案；与目标不符。
- 结论：不采用。

## 后果

正面：

- 真正的 `(server, project)` 多服务器模型；
- 无 PHP 运行时依赖；
- 结构化任务、可取消、可审计；
- SQLite 易备份/迁移；
- 可用 PHP deploy 做行为对照。

负面：

- 需要重写 SSH/SFTP/git/Docker/Nginx/DB 等逻辑；
- 需要跨平台验证；
- SQLite 与 legacy 配置存在迁移成本；
- 没有人工逐行评审，必须依赖自动化测试。

## 约束

- SQLite 不存明文密钥；
- 前端不直接执行 SQL/任意 shell；
- 每个 deployment 串行写；
- PHP fallback 不参与默认路径；
- 旧 `deploy/projects/vbox*` 仅作参考，不直接作为生产配置导入。
