# 文档索引

| 文档 | 内容 |
|---|---|
| [architecture.md](architecture.md) | 总体架构、模块划分、任务模型、PHP 回退边界 |
| [data-model.md](data-model.md) | SQLite 表结构、目录布局、导入/导出、密钥策略 |
| [development.md](development.md) | 环境、命令、目录、开发顺序、测试要求 |
| [roadmap.md](roadmap.md) | 分阶段计划、验收、风险 |
| [ui-preview.md](ui-preview.md) | 界面布局、页面、交互稿 |
| [adr/0001-tauri-sqlite-primary.md](adr/0001-tauri-sqlite-primary.md) | 为什么选择 Tauri 原生 + SQLite |

## 界面截图

当前 UI 的服务器/项目/部署来自 SQLite（首次启动 seed）；容器、活动、备份仍为 mock，按钮只写入模拟终端。

| 页面 | 截图 |
|---|---|
| 项目概览 | [overview.png](screenshots/overview.png) |
| 部署与升级 | [deploy.png](screenshots/deploy.png) |
| 容器 | [containers.png](screenshots/containers.png) |
| 容器批量操作 | [containers-menu.png](screenshots/containers-menu.png) |
| Nginx | [nginx.png](screenshots/nginx.png) |
| 数据库 | [database.png](screenshots/database.png) |
| 日志 | [logs.png](screenshots/logs.png) |
| 本地 vbox | [local-vbox.png](screenshots/local-vbox.png) |
| 项目更多菜单 | [project-more.png](screenshots/project-more.png) |
| 添加项目 | [add-project.png](screenshots/add-project.png) |
| 命令面板 | [command-palette.png](screenshots/command-palette.png) |
| 设置 | [settings.png](screenshots/settings.png) |


## 阅读顺序

1. 新加入项目：`architecture.md` → `data-model.md` → `development.md`
2. 做 UI：`ui-preview.md` + `architecture.md`
3. 做数据库：`data-model.md`
4. 规划任务：`roadmap.md`

## 文档规则

- 设计变更先改文档，再改代码；
- 文档中的“当前状态”必须和代码一致；
- 不把临时排障记录写进设计文档；
- 不在文档中写真实密码、密钥、服务器凭据。
