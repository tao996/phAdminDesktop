# 当前状态与交接

> 最后更新：2026-10-25
> 代码基线：`899d745`（数据库恢复前自动快照）
> 工作目录：`D:\demo\code\tauri\phAdminDesktop`

---

## 1. 一句话状态

桌面端主链路已可用：SQLite 管理配置，原生同步 / 配置生成上传 / 原生部署 / 健康检查 / 失败配置回滚 / 数据库备份恢复均已实现，并在本地 Docker 与 vbox 真实 Phalcon 项目上跑通。

`php admin` 仍保留为迁移、legacy 模式回退、数据库迁移/seed/download/list 的过渡依赖，但不再是原生部署的默认内核。

---

## 2. 已验证场景

| 场景 | 结果 |
|---|---|
| 本地 / SSH 命令、流式输出、取消、日志 | 已验证 |
| 容器 ps / start / stop / restart / logs | 已验证 |
| 多步骤 legacy 部署 | 已验证 |
| 原生部署（本地最小 compose） | 已验证 |
| 原生部署（vbox 最小 compose） | 已验证 |
| 原生部署（本地真实 Phalcon 项目） | HTTP 200 |
| 原生部署（vbox 真实 Phalcon 项目 + 5.23 镜像） | HTTP 200 |
| 部署后健康检查 | vbox 200 成功 / 500 失败均验证 |
| 原生部署失败配置回滚 | vbox 验证 `.env` 恢复 |
| git / bundle / ftp 同步 | vbox 验证 |
| 同步全部顺序执行 + 失败中止 | 本地 / vbox 验证 |
| 配置生成 / 上传回读 / 备份 / 恢复 / 清理 | vbox 验证 |
| 原生 mysqldump 备份 | 真实 MySQL 容器验证 |
| 原生备份恢复 | 真实 MySQL 容器验证 |
| 恢复前自动快照 | 已实现，编译/类型验证；建议再跑一次真实恢复确认 |

当前测试基线：

```text
cargo test: 18 passed, 1 ignored
cargo clippy -- -D warnings: pass
npm run typecheck: pass
npm run build: pass
```

---

## 3. 关键流程

### 3.1 原生部署

Deploy 面板勾选“原生部署（实验）”后：

```text
1. 记录部署前最新配置备份
2. 同步全部（git / bundle / ftp）
3. prepare_runtime_assets
   - default.example.conf -> default.conf
   - php.prod.ini -> php.ini
   - my.example.cnf -> my.cnf
   - 创建日志 / storage 目录
4. 生成 .env / nginx.conf / docker-compose.yaml / config.php
5. 上传配置并逐文件回读校验
6. start_deploy(native=true)
   - compose up -d
   - nginx -s reload
   - 可选 restart php
   - curl 健康检查
7. 失败时恢复部署前配置备份
```

### 3.2 配置

- 生成目录：
  ```text
  <app_data>/generated/<deployment_id>/
    .env
    nginx.conf
    docker-compose.yaml
    config.php
  ```
- 上传目标：deployment `remote_path`
- 覆盖前：
  ```text
  <file>.bak-<millis>
  ```
- 上传后逐文件读取远程内容比较，不一致报错
- 备份支持列表 / 恢复 / 可配置保留数量清理

### 3.3 数据库

- 原生备份：
  ```bash
  docker compose exec -T mysql mysqldump ...
  ```
- 文件：
  ```text
  <app_data>/backups/<deployment_id>/<millis>.sql
  ```
- 恢复：
  - 先自动 dump 当前数据库
  - 再通过 stdin 执行 `mysql`
- 迁移 / seed / 下载 / 备份列表仍走 legacy CLI

---

## 4. 重要实现约束

### SSH

- 只支持 `key_path` + OpenSSH
- 不支持密码直连
- 远程命令必须作为一个 shell 字符串整体传给 `ssh`，否则 `cd` 会被拆散
- 远程目标目录使用独立 `mkdir -p`，不能先 `cd` 到不存在的目录

### 密码

`.env` 的以下值来自 `env_vars`：

```text
MYSQL_USER
MYSQL_DATABASE
MYSQL_PASSWORD
REDIS_PASSWORD
```

缺省用户/库使用项目名。空密码会导致 MySQL/Redis 启动失败。

### 测试注意

- React StrictMode 会让挂载 effect 执行两次；临时测试 hook 必须考虑重复触发
- 测试后要清理：
  - 容器 / compose volume
  - `/tmp/phadmin-*`
  - 测试项目 / deployment / sync_items / env_vars / runs / deploy_runs
  - 本地 generated / backups
- 不要把真实密码、密钥、主机地址写进文档

---

## 5. 当前缺口

### P0

1. 原生部署与 legacy `php admin upgrade` 的产物/结果对比
2. PHP 回退验收
3. 失败后容器清理 / 回滚（目前只回滚配置）

### P1

4. 数据库迁移 / seed / 下载 / 备份列表完全原生化
5. 备份保留策略（当前不自动清理数据库备份）
6. Nginx 上传配置原生化
7. 恢复前快照的真实恢复回归

### P2

8. 容器日志/状态细节、SSL、reset、添加项目等剩余 mock
9. CI / 安装包 / 跨平台 SSH 回归

---

## 6. 常用命令

在 `D:\demo\code\tauri\phAdminDesktop`：

```bash
npm run typecheck
npm run build
npm run tauri dev
```

```bash
cd src-tauri
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

---

## 7. 下一步建议

优先顺序：

1. 在真实 MySQL 容器上验证“恢复前自动快照”确实生成文件
2. 原生部署 vs legacy `upgrade` 同项目产物 diff
3. 失败后容器处理策略
4. 数据库备份保留数量
5. PHP 回退验收
