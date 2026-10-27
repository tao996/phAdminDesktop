# 当前状态与交接

> 最后更新：2026-10-25
> 代码基线：工作树（`php admin` 全量原生化，未提交）
> 工作目录：`D:\demo\code\tauri\phAdminDesktop`

---

## 1. 一句话状态

桌面端是唯一执行内核：SQLite 管理配置，原生同步 / 配置生成上传 / 部署 / 健康检查 / 失败配置回滚 / 数据库备份恢复 / 迁移 seed / 宿主机 Nginx 与 SSL / cron / 隧道 / phpMyAdmin 全部由 Rust 实现，本地 Docker 与 vbox 真实 Phalcon 项目主链路已跑通。

`php admin` 依赖已移除：没有 legacy 导入、没有 upgrade 步骤、没有 PHP fallback，终端白名单也不再含 `php`。新增的 Nginx / SSL / 隧道 / phpMyAdmin / cron 命令已完成单测与编译验证，尚未全部在真实服务器回归。

启动不再自动 seed：空库只建表，工作区从 0 开始；服务器 / 项目 / 部署的创建流程尚未实现（见缺口 P0）。

---

## 2. 已验证场景

| 场景 | 结果 |
|---|---|
| 本地 / SSH 命令、流式输出、取消、日志 | 已验证 |
| 容器 ps / start / stop / restart / logs | 已验证 |
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
| 密钥写入系统凭据管理器 | Windows Credential Manager 真机读写验证 |
| 宿主机 Nginx 发布 / SSL / cron / 隧道 / phpMyAdmin | 已实现 + 单测；待真实服务器回归 |

当前测试基线：

```text
cargo test: 32 passed, 3 ignored
  - ignored: docker compose 实测（PHADMIN_COMPOSE_DIR）
  - ignored: 系统凭据存储实测（cargo test keychain -- --ignored）
  - ignored: 密钥链路实测（cargo test secret_round_trips -- --ignored）
cargo clippy -- -D warnings: pass
npm run typecheck: pass
npm run build: pass
```

---

## 3. 关键流程

### 3.1 部署

Deploy 面板“开始部署”（不再有原生/legacy 开关）：

```text
1. 记录部署前最新配置备份
2. 同步全部（git / bundle / ftp），失败中止
3. prepare_runtime_assets
   - default.example.conf -> default.conf
   - php.prod.ini -> php.ini
   - my.example.cnf -> my.cnf
   - 创建日志 / storage 目录
4. 生成 .env / nginx.conf / docker-compose.yaml / config.php
5. 上传配置并逐文件回读校验
6. start_deploy
   - compose up -d
   - nginx -s reload（容器内）
   - 可选 docker compose restart php
   - curl 健康检查
7. 失败时恢复部署前配置备份
```

宿主机 Nginx 站点配置是独立动作：Nginx 面板「发布配置」→
`/etc/nginx/conf.d/<project>.conf` + `nginx -t` + reload，失败自动回滚。

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

### 3.3 环境变量与密钥

- 入口：配置弹窗 →「环境变量」页；可增删改 `MYSQL_PASSWORD` / `REDIS_PASSWORD` 等
- 变量名含 `PASSWORD` / `SECRET` / `TOKEN` / `PRIVATE_KEY` / `APP_KEY` 时自动按密文处理
- 密文值写入 OS 凭据管理器（service `PhAdmin Desktop`，account `<deployment_id>/<key>`），
  SQLite 的 `env_vars.value` 恒为 `NULL`，只存 `secret_ref`
- `.env` 渲染、数据库备份/恢复取密码都从凭据管理器读取
- 缺少必需密码时配置弹窗顶部告警（尚未阻断部署）

### 3.4 数据库

- 原生备份：
  ```bash
  docker compose exec -T mysql mysqldump ...
  ```
- 文件：
  ```text
  <app_data>/backups/<deployment_id>/<millis>.sql
  ```
- 恢复：先自动 dump 当前数据库，再通过 stdin 执行 `mysql`
- 迁移：`docker compose exec -T php php artisan migration r`
- Seed：上传 `<app_data>/db/<deployment_id>/*.sql` 到 `<remote>/src/db/`，再执行
  `docker compose exec -T php php artisan db:seed --dir=/var/www/db`
- 定时备份：写入服务器 crontab 标记块（`# BEGIN phadmin-backup <project>`），
  每天 mysqldump 压缩到 `<remote>/docker/storage/backup/mysql`，`find -mtime +N -delete` 保留
- 服务器备份列表：`list_remote_backups` 读取同一个目录

### 3.5 宿主机 Nginx / SSL

- `publish_nginx_config`：按 `domains` 与 `nginx_port` 渲染，上传
  `/etc/nginx/conf.d/<project>.conf`，`nginx -t` 通过才 reload，失败自动恢复旧文件
- `remove_nginx_config`：删除站点配置并 reload
- `read_nginx_log`：读取宿主机 `/var/log/nginx/{access,error}.log` 最近 200 行
- `check_server`：检测 docker / nginx / certbot / 80·443 端口 / conf.d / 证书
- `enable_ssl`：certbot webroot（`<remote>/src/public`）签发，证书链接到
  `/etc/nginx/ssl/<primary>.pem|.key`，`deployments.ssl=1` 并切换到 HTTPS 配置

### 3.6 隧道 / phpMyAdmin / 重置

- `start_ssh_tunnel`：反查 MySQL 容器网络 IP，`ssh -N -L 127.0.0.1:<local>:<ip>:3306`，
  进程保存在 `AppState.tunnels`，`stop_ssh_tunnel` 终止；filesystem 目标不可用
- `start_phpmyadmin`：反查 MySQL 容器网络后 `docker run --rm -p <port>:80 phpmyadmin/phpmyadmin`
- `reset_project`：主仓库 + 每个 git/bundle 同步目录执行 `git reset --hard`（不删配置）
- `generate_git_deploy_key`：服务器生成/读取 `~/.ssh/id_ed25519.pub`

---

## 4. 重要实现约束

### SSH

- 服务器配置只有 `host` / `port` / `user` + 认证方式（密码，或私钥 + 可选口令）+ `docker.network`（默认 `phalcon-shared`）
- 密码认证通过 `SSH_ASKPASS` 非交互喂给 `ssh` / `scp`（Windows OpenSSH 9.5 已验证）
- 密钥认证使用私钥文件，可选 `keyPassphrase`
- 密码 / 私钥口令按用户要求**明文存 SQLite** `servers.password` / `servers.passphrase`；
  数据库文件与备份需按敏感文件管理（环境变量密文仍走 keychain）
- 服务器名自动生成为 `user@host:port`，不单独维护名称/环境/类型
- 部署使用服务器的共享网络：`.env` 写入 `DOCKER_NETWORK`，compose 以外部网络引用，部署前自动创建网络

### 模板管理

- 顶栏「模板管理」直接打开两栏面板：左侧模板列表（含「添加模板文件」），右侧编辑器，点击左侧条目即切换；
- 内置 6 个模板：`.env`、`docker-compose.yaml`、`src/config/config.php`、`docker/mysql/my.cnf`、`docker/nginx/sites/default.conf`、`docker/php/php.ini`；
- 点击列表条目直接编辑：等宽编辑器 + 提示「模板内容支持 `{{xxx}}`，以后在项目中设置值生成配置文件」；保存、恢复默认/删除；
- 模板管理面板不做预览（预览属于项目级能力，模板是全局的）；
- 数据库只存覆盖/自定义；解析顺序是「DB 行 → 内置默认」，空库不 seed；
- 渲染支持 `.env` / compose / config.php / 三个容器配置，自定义模板按路径上传；配置上传目标 `src/config/config.php`（不再写项目根目录）；
- 暂不做模板历史版本/上传前备份（用户确认）；`remote_config_status` 仍只覆盖 4 个根目录文件。

### 项目创建

- 表单顺序：项目名称/模式/服务器 → 项目配置（部署目录、OPEN_PORT、域名、SSL）→ 环境配置（TZ、APP_NAME、MYSQL_DATABASE、MYSQL_USER、密码、DATA_PATH_HOST）→ 代码同步 → PHP 配置；
- 代码同步项按方式显示字段：`git`（仓库必填、分支默认 main、目录可空、名称）、`bundle`（分支默认 main、名称）、`ftp`（目录必填、名称、排除目录按顺序英文逗号分隔）；按列表顺序写入 `sync_items`；
- `APP_NAME`、`MYSQL_DATABASE` 可编辑，默认分别是项目名、`<项目名>_db`；密码写入系统凭据管理器；
- `OPEN_PORT` 在同一目标（同服务器 / 本机）内唯一；
- 部署目录可填写：Remote 为空时默认 `/data/<name>`，Local 必填并自动创建 `kind=filesystem` 的「本地」服务器；
- PHP 配置原样写入 `deployment_settings.config_php`，渲染时作为 `{{CONFIG_OVERRIDES}}` 注入；
- 创建失败（如凭据写入失败）会回滚项目/部署记录；
- 项目列表右键「修改项目」打开同一表单（项目名只读），保存走 `update_project`：更新目标、环境变量、端口、域名、SSL、同步列表、PHP 配置；
- 应用内右键已禁用 WebView 默认菜单（Windows 通过 WebView2 设置关闭，前端同时 preventDefault）。
- 工具顶栏「服务器管理」列出全部服务器，点击打开修改弹窗，底部可新增服务器
- 修改弹窗支持删除：点击「删除」先弹确认框并检查引用；有引用时「确定」禁用并显示引用数量，无引用时确认后才真正删除
- 左侧面板只放项目列表（不再按服务器分组）
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

1. 从 0 创建数据：服务器创建/修改/删除已落 SQLite；项目创建与修改已实现（Remote/Local + 环境变量 + 端口/域名/SSL + 同步列表 + PHP 配置）；部署编排仍待补
2. 新增运维命令的真实服务器回归：Nginx 发布 / SSL / cron / 隧道 / phpMyAdmin 目前只有单测与编译验证
3. 失败后容器清理 / 回滚（目前只回滚配置）
4. 部署前强校验必需密钥：现在只有 UI 告警，空口令要到 compose 启动才失败

### P1

5. phpMyAdmin / 隧道状态持久化（当前只存进程内状态，重启后需重新操作）
6. 恢复前快照的真实恢复回归
7. 备份保留策略只覆盖服务器 cron 备份，本地 `<app_data>/backups` 仍不自动清理

### P2

8. 容器日志/状态细节、SSL 自动续期提示
9. CI / 安装包 / 跨平台 SSH 回归（含 macOS / Linux 凭据管理器）
10. 日志脱敏尚未实现（`run_command` 仍存原始 argv）

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

1. 实现从 0 创建数据：新增服务器 → 新增项目 → 新增 deployment（写 SQLite，替换 AddProjectModal 的内存实现）
2. 在真实服务器回归 Nginx 发布 / SSL / cron / 隧道 / phpMyAdmin
3. 在真实 MySQL 容器上验证“恢复前自动快照”确实生成文件
4. 失败后容器处理策略
5. 部署前校验 `MYSQL_PASSWORD` / `REDIS_PASSWORD`，缺失直接拒绝部署
6. 本地备份保留数量 + 日志脱敏
### 文件预览

- 工作区新增「文件预览」：调用 `preview_project_files` 返回按部署渲染后的 `.env`、`docker-compose.yaml`、`src/config/config.php`、三个容器配置和全部自定义模板；只读，不写磁盘、不连接服务器；
- 内容与部署时上传的文件一致（项目配置 + 模板管理合并结果）。
### vbox3 实测（2026-10-26）

- 全新部署通过：`/data/vbox3` 创建 → bundle(clone -b main) → ftp → `.env`/compose/`config.php`/容器配置上传 → `docker compose up -d` → nginx reload → php restart → 健康检查；
- 远程 `curl http://127.0.0.1:8073/` = 200（2193 bytes），四个容器 `vbox3-{mysql,nginx,php,redis}` 运行；
- 宿主机 Nginx 发布 `/etc/nginx/conf.d/vbox3.conf` 并 reload；Windows `curl --resolve vbox3.test:80:192.168.56.120` = 200；
- 数据库备份成功（本地 `backups/.../1793008807288.sql`）；
- 修复：渲染变量改读 env（APP_NAME/MYSQL_DATABASE/MYSQL_USER/DATA_PATH_HOST/TZ）；远程 bundle 先建目标目录；bundle clone 指定分支；浅克隆仓库给出明确错误；
- 测试环境：本地仓库已 `git fetch --unshallow`（331 commits）。
## 完整测试记录（vbox3 / vbox，2026-10-26）

### 已通过

- 服务器：新增 / 读取 / 修改 / 引用计数 / 删除（无引用可删，有引用拒绝）；
- 模板：列表、读取、自定义保存、恢复默认；预览项目文件（.env / compose / config.php / 容器配置）；
- 项目：配置读取、环境变量修改往返、部署目录 / APP_NAME / MYSQL_DATABASE、右键修改；
- 部署：vbox3 全新部署、vbox3 重部署（fetch/merge）、vbox 已有环境部署；健康检查 200；
- 同步：bundle、ftp 单项执行；同步全部；
- 容器：测试连接（compose ps）、容器面板刷新；
- Nginx：发布、reload、读取 access/error 日志、移除、重新发布；vbox/vbox3 测试域名均 200；
- 数据库：备份、列出、恢复（恢复前快照）、migrate、seed（seed 目录不存在时跳过）；
- cron：安装 / 移除；SSH 隧道：启动 / 列表 / 停止；phpMyAdmin：启动 / 停止；部署密钥生成；
- 终端：白名单拦截 `rm`，`docker compose ps` 正常；运行记录与日志读取；reset 项目；
- SSL：certbot 未安装，返回明确失败信息；
- UI：设置（本地仓库读写）、服务器/模板/添加项目弹窗、概览/部署/文件预览/容器/Nginx/数据库/日志页签、项目右键菜单。

### 本轮修复

- 部署完成后更新 `deployments.status` / `last_deploy`，并在前端刷新工作区；
- 渲染变量改为读取 `env_vars`（APP_NAME / MYSQL_DATABASE / MYSQL_USER / DATA_PATH_HOST / TZ），不再被项目名覆盖；
- 远程 bundle 先创建目标目录；clone 指定分支；历史不一致时强制对齐并提示；浅克隆仓库给出明确错误；
- 日志页改为读取真实 `runs`，时间本地化；概览容器空状态提示「点击测试连接刷新」。

### 已知限制

- certbot 未安装（测试机），SSL 无法真实通过；
- 项目删除尚未实现；
- 旧环境（vbox）中历史 nginx 容器网络与 `phalcon-shared` 不一致时，`compose up` 不会自动重建，需要 `--force-recreate nginx` 一次；
- 设置里的主题 / 危险操作确认 / 终端行数仍是占位项，未持久化。
### 服务器名称

- 服务器表单新增「名称」必填项，用于项目与部署界面展示；重名会被拒绝；
- 已有服务器可通过「服务器管理 → 修改服务器」改名（id 与部署引用不变）；
- 添加项目、部署面板、服务器菜单不再以 IP 作为主要显示文本。
### 服务器/项目列表展示

- 修改服务器时，密码输入框右侧有「显示/隐藏密码」图标，可在明文与密文之间切换（仅界面切换，保存逻辑不变）；
- 「服务器管理」菜单副标题显示 IP 地址，主标题为服务器名称；
- 项目列表副标题改为「服务器名称 · 状态」，不再重复显示项目名称。
## UI 调整（概览 / 部署 / 文件预览 / Nginx / 数据库）

- 测试连接完成后刷新概览容器状态（`refreshVersion` 变化才拉取，项目切换仍不自动连接）；
- 概览 Nginx 卡片显示 `http(s)://域名`，点击用系统默认浏览器打开；
- 移除部署面板「预览配置」（与文件预览重复）；增加部署密钥作用说明；
- 文件预览右侧改为可编辑 textarea，支持复制、重置修改（仅本地查看，不写回模板）；
- 双栏卡片补充底部留白；
- 原生备份：改为 `.sql.gz` 压缩保存，列表支持旧 `.sql` 与新 `.gz`，新增「下载」导出到系统下载目录；原生备份区域移动到数据库页下方。
### 数据库按钮说明

- 「本地备份」：刷新本机备份列表并滚动到「原生备份」，顶部显示加载数量（原先无反馈）；
- 「Seed 目录」：打开本地 seed 目录（`%APPDATA%/.../db/<deployment>`），放入 `.sql` 后上传到服务器 `src/db`；
- 「结构迁移」：在服务器 php 容器执行 `php artisan migration`；
- 「Seed」：把本地 seed 目录的 `.sql` 上传到服务器并执行 `php artisan db:seed --dir=/var/www/db`；
- 以上按钮均增加悬停提示。
### phpMyAdmin 提示

- 启动临时 phpMyAdmin 后，终端输出访问地址、用户名、密码（密码仅显示在终端，不写运行日志）；
- 访问地址渲染为可点击链接，点击用系统浏览器打开；
- 临时容器仍为 `--rm`，用完点「删除」清理。
### 文件预览语法高亮

- 使用 CodeMirror 6（静态打进主包），右侧改为带行号的代码编辑器，保留编辑/复制/重置草稿；
- 类型映射：`.env`/`.ini`/`.cnf` → properties，`.yaml/.yml` → YAML，`.conf` → Nginx，`.php` → PHP，其它纯文本；
- 新增「添加文件」：输入相对路径与内容后写入模板表（自定义模板），立即参与预览、生成与上传；
- 主包体积约从 331 KB 增至 919 KB（未拆包，按用户要求）。
