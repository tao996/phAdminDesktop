# 路线图

> 状态：规划中
> 关联：[architecture.md](architecture.md) · [data-model.md](data-model.md)

---

## 1. 目标

用 Tauri 2 原生实现替代 `php admin`，以 SQLite 管理多服务器/多项目部署；桌面端稳定后，PHP deploy 下线。

### 下线条件

- `init` / `upgrade` / `reset` / `dc:*` / `nginx:*` / `db:*` / SSL 主流程全部由桌面端覆盖；
- 至少 1 个真实项目在 prod + staging 完成回归；
- SQLite 导入/导出、PHP 回退路径验证通过；
- 关键路径自动化测试 + CI 门禁；
- 有版本化安装包和回滚方案。

---

## 2. 阶段

### Phase 0：脚手架（已完成）

- Tauri 2 + React + TypeScript + Vite；
- 产品名、标识、窗口尺寸；
- 文档基线；
- `npm run build` / `cargo check` 可运行。

### Phase 1：SQLite + 只读/导入（已完成）

- 接入 SQLite（sqlx 或 rusqlite，先定一个）；
- migrations：`servers`、`projects`、`deployments`；
- Rust repository + `list_*` commands；
- UI：服务器列表、项目 × 服务器矩阵；
- 从 `php admin config:export` 导入一次真实配置；
- 只读：不执行任何部署动作。

验收：

- 重启应用后数据还在；
- 同一项目可绑定 prod/staging 两个 deployment；
- 导入幂等，重复导入不产生重复行。

### Phase 2：状态与终端

- `ssh` exec、流式 stdout/stderr、取消；
- `runs` 表 + 日志文件；
- 只读命令：`docker compose ps`、`docker compose logs`、nginx 日志、`server:init` 检查；
- xterm.js 终端；
- 服务器连接测试。

验收：

- 命令输出实时显示；
- 取消后进程树结束；
- 运行记录可查；
- 不阻塞主界面。

### Phase 3：部署核心

- `init`：目录准备、配置生成、代码同步、compose up、nginx、hooks；
- `upgrade`：git/bundle/ftp、method/name/full、配置更新；
- `reset`；
- `sync_items` 执行器；
- 模板渲染（`.env`/compose/nginx/php.ini/config.php）；
- 与 PHP deploy 做 golden/diff。

验收：

- 在测试服务器完整部署一个项目；
- 生成文件与 PHP 版本一致或差异有明确说明；
- 失败可回滚/重试；
- 同一 deployment 不会并发写。

### Phase 4：Docker / Nginx / 数据库

- `dc:status/restart/logs`；
- `nginx:reload/add/remove/ssl`；
- `db:migrate/seed/backup/list/download/cron`；
- SSH 隧道、phpMyAdmin；
- 高风险操作确认。

验收：

- 备份可下载/恢复；
- SSL 申请成功；
- `db:proxy` 后台运行可停止；
- 危险操作有确认。

### Phase 5：迁移与下线

- SQLite → legacy PHP 配置导出；
- 差分测试全量通过；
- 文档/安装包/自动更新；
- 观察期；
- PHP deploy 标记 legacy，最终移出主流程。

---

## 3. 优先级

| 优先级 | 功能 | 原因 |
|---|---|---|
| P0 | SQLite + 多服务器模型 | 当前最大缺口 |
| P0 | 只读列表/状态/日志 | 低风险，先验证架构 |
| P1 | SSH/任务/终端 | 所有写操作的基础 |
| P1 | init/upgrade | 核心价值 |
| P2 | Docker/Nginx | 日常运维 |
| P2 | 数据库备份/迁移 | 高风险，需充分测试 |
| P3 | SSL/隧道/phpMyAdmin | 低频但复杂 |
| P3 | 自动更新/签名 | 发布阶段 |

---

## 4. 风险

| 风险 | 影响 | 对策 |
|---|---|---|
| Rust 重写行为偏差 | 部署失败/数据风险 | PHP 作为 oracle，golden/diff 测试 |
| SQLite 与 PHP 配置双写 | 状态分叉 | SQLite 单写，PHP 仅导出/回退 |
| SSH/SFTP 跨平台差异 | Windows/macOS 行为不一致 | CI 矩阵 + Docker sshd 集成测试 |
| 密钥泄漏 | 安全事件 | OS keychain、日志脱敏、权限 |
| 并发写同一 deployment | 配置覆盖 | `(deployment_id)` 锁 + 串行队列 |
| AI 生成代码缺少边界 | 隐性 bug | 测试门禁、真实服务器回归 |
| 旧 vbox 配置干扰 | 误部署 | 旧 `deploy/projects/vbox*` 仅作参考，导入时显式确认 |

---

## 5. 暂不做

- 服务器端 Web 控制台；
- 多用户/权限系统；
- 云端同步；
- 移动端；
- 自动发现服务器；
- 替代 Phalcon Admin 业务后台。

---

## 6. 当前进度

- [x] Tauri 脚手架
- [x] UI 设计稿
- [x] 静态 UI 原型（mock 数据，不含后端）
- [x] SQLite 三表 + seed（`servers/projects/deployments`）
- [x] 只读 `load_workspace` + UI 从 SQLite 加载
- [x] 架构/数据模型/开发文档
- [x] legacy `config:export` 导入
- [x] sync_items / runs 表（schema + 导入）
- [x] 基础任务执行（本地/ssh + stdout/stderr + runs）
- [x] 任务历史面板 / 取消按钮
- [x] runs 日志文件 + 历史日志查看
- [x] 容器状态解析（`docker compose ps --format json`）
- [x] 容器启动/停止/重启/日志操作
- [x] 概览容器统计 / 最近活动接入真实数据
- [x] 终端白名单命令（argv 解析，无 shell 拼接）
- [x] 升级命令执行（单命令）
- [x] 升级成功后串行重启 PHP
- [x] 多步骤部署编排（legacy upgrade + compose + nginx + restart）
- [x] sync_items / deployment_settings 读取与部署面板展示
- [x] 原生 git sync（本地 filesystem / SSH，clone / pull）
- [x] 本地 bundle sync
- [x] SSH bundle sync（scp + 远程 git）
- [x] 同步项级并发锁
- [x] 部署配置只读预览（sync items + JSON）
- [x] ftp / SFTP 基础同步（本地复制 / scp -r）
- [x] `.env` / Nginx 渲染预览
- [x] 本地 generated 目录写入（.env / nginx / compose）
- [x] 部署计划真实步骤预览
- [x] 本地 generated 状态检测
- [x] 任务面板最近部署记录
- [x] vbox 远程 git / bundle / scp 实测通过（key_path 登录）
- [x] 远程 generated 配置上传 + `.bak` 备份 + 差异检查 + 恢复 + 清理（vbox 实测）
- [x] config.php 预览 / 生成 / 上传
- [x] Deploy 面板“同步全部”（顺序执行并等待 git run；vbox 远程链路实测）
- [x] 模板与 legacy 逐行对齐（env/compose/config.php）
- [ ] 远程配置写入产品策略
- [ ] PHP 回退验收
