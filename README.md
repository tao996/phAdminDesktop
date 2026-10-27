# PhAdmin Desktop

Phalcon Admin 的主机侧部署控制台，使用 Tauri 2 + Rust + React + TypeScript + SQLite 构建。

> 当前状态：`php admin` 的全部功能已原生化，桌面端是唯一执行内核。首次启动为空工作区（不再自动灌演示数据）。完整进度与剩余缺口见 [docs/current-state.md](docs/current-state.md)。

![项目概览](docs/screenshots/overview.png)

## 已实现能力

- SQLite 工作区、多服务器/多项目 deployment；
- 本地 / SSH 命令执行、流式输出、取消、run 历史与日志；
- 容器真实状态、启动/停止/重启/日志；
- 部署：同步全部 → 生成并上传配置 → compose up → Nginx reload → 可选重启 PHP → HTTP 健康检查；失败自动恢复配置备份；
- 原生同步：git / bundle / ftp，本地与 SSH，同步项级锁；
- 配置预览与生成：`.env` / `nginx.conf` / `docker-compose.yaml` / `config.php`；
- 配置上传后回读校验、`.bak-<millis>` 备份、差异检查、恢复、可配置保留数量清理；
- 宿主机 Nginx：发布 / 移除 / 语法检查并 reload / 访问与错误日志 / 环境检查；
- SSL：certbot webroot 申请证书、证书链接、切换 HTTPS 配置；
- 数据库：mysqldump 备份到本地、备份列表、恢复（恢复前自动快照）、结构迁移、seed、服务器 crontab 定时备份；
- 运维：重置代码、生成部署密钥、SSH 隧道、临时 phpMyAdmin；
- 密钥：密码类环境变量存 OS 凭据管理器，SQLite 只存引用；
- 部署计划真实步骤、最近部署历史、配置状态检测。

## 目标

- 多服务器、多项目部署管理（`server × project`）
- 部署 / 升级 / 重置 / Docker / Nginx / 数据库备份 / SSL
- SQLite 管理配置、部署关系、运行历史
- 终端流式输出、任务取消、日志归档
- 已取代仓库根目录的 `php admin`，不再调用 legacy CLI

## 快速开始

```bash
npm install
npm run tauri dev
```

其他命令：

```bash
npm run typecheck
npm run build
npm run tauri build
```

Rust 检查：

```bash
cd src-tauri
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

## 文档

- [docs/README.md](docs/README.md) — 文档索引
- [docs/architecture.md](docs/architecture.md) — 总体架构
- [docs/data-model.md](docs/data-model.md) — SQLite 数据模型
- [docs/development.md](docs/development.md) — 开发指南
- [docs/roadmap.md](docs/roadmap.md) — 路线图
- [docs/ui-preview.md](docs/ui-preview.md) — 界面设计稿

## 目录

```text
src/           React + TypeScript 前端
src-tauri/     Rust + Tauri 后端
docs/          设计与开发文档
```

## Legacy

仓库 `D:\demo\code\flutter\library\projects\phalcon-admin` 仅作历史参考：

- 桌面端不调用 `php admin`，也不读取 `deploy/projects/*/server.php`；
- 不再提供配置导入，部署数据直接维护在 SQLite；
- 旧仓库保留与否不影响桌面端运行。
