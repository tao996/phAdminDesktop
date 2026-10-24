# PhAdmin Desktop

Phalcon Admin 的主机侧部署控制台，使用 Tauri 2 + Rust + React + TypeScript + SQLite 构建。

> 当前状态：静态 UI + SQLite 只读层已实现；数据来自本地 SQLite（首次启动 seed），部署/SSH 逻辑尚未接入。

![项目概览](docs/screenshots/overview.png)

## 目标

- 多服务器、多项目部署管理（`server × project`）
- 部署 / 升级 / 重置 / Docker / Nginx / 数据库备份 / SSL
- SQLite 管理配置、部署关系、运行历史
- 终端流式输出、任务取消、日志归档
- 最终替代仓库根目录的 `php admin`

`php admin` 在过渡期保留为迁移来源、差分验收 oracle 和应急回退路径。

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

旧部署工具与配置：

```text
D:\demo\code\flutter\library\projects\phalcon-admin
```

迁移入口：

```bash
php admin config:export
```

在桌面端稳定并完成验收前，不要删除 legacy `deploy`。
