# PhAdmin Desktop UI Preview

> 状态：静态 UI 原型已实现；业务逻辑未接入
> 主数据源：SQLite
> 执行内核：Tauri 2 / Rust 原生（`php admin` 仅作迁移与回退参考）
> 日期：2026-10-24

> 说明：本文保留 `php admin` 命令映射，用于**功能对照和回退参考**；桌面端默认执行路径以 [architecture.md](architecture.md) 的 Rust 原生实现为准。

---

## 一、定位与边界

### 1.1 产品定位

PhAdmin Desktop 是 **主机侧部署控制台**，不是服务器端 Web 后台，也不是 Phalcon Admin 业务后台的替代品。

它解决的是：

- 不想围绕 `php admin app:<项目> ...` 命令来回切换终端；
- 想在桌面端同时管理多台服务器、多个项目；
- 想在一个窗口里完成“看状态 → 预览 → 部署 → 看日志 → 备份”的闭环；
- 想把每次命令执行结果、耗时、退出码留痕，方便排障。

### 1.2 明确不做

- 不把 WebView 当作部署引擎：SSH/SFTP/git/Docker 等逻辑在 Rust 侧实现；
- 不把 `php admin` 作为默认执行路径，仅保留迁移/差分/回退；
- 不提供远程服务器上的常驻 Web 服务；
- 不允许 WebView 直接执行任意 shell 命令；
- 暂不做多用户、权限系统、云端同步。

### 1.3 设计原则

1. **能力对齐**：UI 功能与 legacy `php admin` 命令语义逐项对齐，并保留命令对照，便于回退。
2. **预览先行**：部署、Nginx、配置类操作先展示影响范围，再允许执行。
3. **终端即事实**：命令原始输出完整流式展示，状态卡片只是辅助，不作为唯一依据。
4. **危险操作可恢复**：删除、重置、覆盖配置必须二次确认，确认文案包含项目和服务器。
5. **项目隔离**：每个 `(server, project)` 部署独立 Tab、终端、运行历史，互不串台。
6. **跨平台**：Windows / macOS / Linux 都能运行，路径与密钥配置可迁移。

---

## 二、用户与典型场景

### 2.1 目标用户

- 项目开发者：本地改完代码后，一键升级测试服/生产服；
- 运维人员：检查服务器环境、Nginx、容器、数据库备份；
- 项目负责人：查看部署状态和最近一次操作记录。

### 2.2 核心场景

| 场景 | 用户动作 | 对应命令 |
|---|---|---|
| 首次连服务器 | 选择服务器 → 检查环境 | `php admin server:init` |
| 初始化服务器 | 检查报告 → 点击“安装” | `php admin server:init -y` |
| 新项目上线 | 添加项目 → 预览配置 → 正式部署 | `app:<项目> init` / `init -y` |
| 日常发版 | 项目 → 部署 → 升级代码 → 重启 PHP | `app:<项目> upgrade` + `dc:restart:php` |
| 排查 502 | 看容器状态 → 看 PHP 日志 | `dc:status` / `dc:log:php` |
| 配置 HTTPS | 项目 → Nginx → 申请证书 | `nginx:ssl -email=...` |
| 数据库备份 | 项目 → 数据库 → 立即备份/下载 | `db:backup` / `db:backup:get` |
| 临时查库 | 开启 SSH 隧道或临时 phpMyAdmin | `db:proxy` / `db:pma` |

---

## 三、信息架构

```text
PhAdmin Desktop
├── 顶栏
│   ├── 服务器选择器（prod / staging / local）
│   ├── 连接状态（未连接 / 检测中 / 已连接 / 失败）
│   ├── 全局命令面板（⌘K / Ctrl+K）
│   └── 设置
├── 左侧项目栏
│   ├── 项目搜索
│   ├── 项目列表（来自 SQLite deployments；仅 legacy 导入时读 deploy/projects）
│   ├── 项目状态点（未配置 / 未部署 / 运行中 / 异常）
│   └── + 添加项目
├── 主工作区（项目 Tab）
│   ├── 概览
│   ├── 部署与升级
│   ├── Docker 容器
│   ├── Nginx
│   ├── 数据库
│   ├── 脚本与日志
│   └── 项目配置
└── 底部终端抽屉
    ├── 当前项目终端
    ├── 全局服务器终端
    └── 运行历史
```

### 3.1 与原草案的对应

| 原草案 | 本稿落点 |
|---|---|
| 上方工具栏：服务器 / Nginx / 项目 | 保留为顶栏“服务器”菜单、全局“Nginx”菜单、项目区“+ 添加项目” |
| 下方左侧面板 Tabs（项目名称 + 每个 Tab 自己的终端） | 项目 Tab 放在主工作区顶部；每个项目保留独立终端，终端 Tabs 放在底部抽屉 |
| 下方右侧项目命令列表 | 右侧“操作面板 / 命令面板”，按当前项目上下文展示可用命令 |
| 部署 | 独立“部署与升级”页 + 概览页快捷按钮 |

> 说明：项目多时，横向 Tab + 底部终端比“左下 Tabs + 右下命令列表”更符合桌面习惯；原草案的“每个项目独立终端”完整保留。
>
> 实现口径：项目/部署数据来自 SQLite；`deploy/projects/*/server.php` 只在首次 legacy 导入时读取。

---

## 四、主界面布局

### 4.1 主窗口（1440 × 900）

```text
┌──────────────────────────────────────────────────────────────────────────────────────┐
│  PhAdmin Desktop      [服务器: prod ▾]  [● 已连接]        [⌘K 命令]      [⚙ 设置]  │
├──────────────────┬───────────────────────────────────────────────────────────────────┤
│  项目             │  [boyu ×]  [yihe ×]  [vbox2 ×]                    [+ 添加项目]  │
│  ┌────────────┐  │ ┌───────────────────────────────────────────────────────────────┐ │
│  │ 🔍 搜索     │  │ │ boyu  ›  概览                                                │ │
│  └────────────┘  │ │                                                               │ │
│  ● boyu    运行中 │ │  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐  │ │
│  ● yihe    运行中 │ │  │ 服务器      │ │ 容器        │ │ Nginx      │ │ 最近部署    │  │ │
│  ○ vbox2   未部署 │ │  │ 已连接      │ │ 4/4 运行    │ │ SSL 已启用  │ │ 2 小时前    │  │ │
│                   │ │  └────────────┘ └────────────┘ └────────────┘ └────────────┘  │ │
│  ───────────────  │ │                                                               │ │
│  + 添加项目       │ │  快捷操作                                                     │ │
│                   │ │  [部署] [升级代码] [重启 PHP] [查看日志] [备份数据库]          │ │
│                   │ │                                                               │ │
│                   │ │  最近活动                                                     │ │
│                   │ │  12:30  upgrade 成功  2m 18s                                  │ │
│                   │ │  11:05  数据库备份成功  18.4 MB                               │ │
│                   │ └───────────────────────────────────────────────────────────────┘ │
│                   │ ┌─ 终端: boyu ────────────────────────────── [清空] [复制] [▾] ┐ │
│                   │ │ $ php admin app:boyu upgrade -y                             │ │
│                   │ │ → [1/2] [bundle] 主仓库 ...                                 │ │
│                   │ │ ✔ 代码同步完成                                              │ │
│                   │ └───────────────────────────────────────────────────────────────┘ │
├──────────────────┴───────────────────────────────────────────────────────────────────┤
│ 就绪  │  最近命令：app:boyu upgrade -y  │  退出码 0  │  日志已保存                     │
└──────────────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 布局规则

- 左侧项目栏宽度 `240px`，可拖拽到 `200–320px`，可折叠为 `56px` 图标栏；
- 项目 Tab 横向滚动，最多同时打开 8 个，超出后通过“更多”下拉；
- 终端抽屉高度默认 `260px`，可拖拽到 `160px – 60%`，可完全收起；
- 主工作区最小宽度 `720px`；窗口最小尺寸建议 `1024 × 700`；
- 状态栏常驻：当前服务器、当前项目、最后一条命令、退出码、日志路径。

---

## 五、页面设计

### 5.1 服务器页（全局）

入口：顶栏“服务器”菜单 → “环境检查 / 安装依赖 / 服务器列表”。

#### 服务器选择器

```text
┌ 服务器 ─────────────────────────────┐
│ ● prod      47.106.226.129:22  已连接 │
│ ○ staging   192.168.56.120:22  未连接 │
│ ○ local     filesystem         本地   │
│ ─────────────────────────────────── │
│ + 新建服务器配置                     │
└─────────────────────────────────────┘
```

- 服务器列表来源：SQLite `servers`；首次可从 `deploy/server.php` / `server.<env>.php` 导入；
- 显示 `host:port` 与连接状态，不显示密码；
- 切换服务器时，项目列表与状态全部刷新，并终止当前服务器上的运行任务（需确认）；
- 新建/编辑服务器配置首版建议“打开配置文件”，不在 UI 内编辑密码；后续再做表单化。

#### 环境检查 / 安装

- 页面按钮：
  - **检查环境** → `php admin server:init`
  - **安装/初始化** → `php admin server:init -y`（危险操作，二次确认）
- 检查结果以卡片展示：
  - 操作系统、Nginx 进程、Nginx 命令、Certbot、80/443 端口、Docker、Compose；
  - 每项状态：`正常 / 未安装 / 占用 / 未知`；
  - 卡片下方保留“原始输出”折叠区。
- 首版可以只展示终端；卡片解析作为增强项（见 §9.2）。

> 下列 `php admin` 命令是 legacy 行为对照；桌面端由 Rust 原生执行 `server:init` 对应逻辑。

### 5.2 项目列表与添加

#### 项目列表

- 项目/部署来自 SQLite `projects` / `deployments`；不再扫描 `deploy/projects/*/server.php`；
- 项目状态：
  - `未配置`：有项目记录，没有 deployment；
  - `未部署`：有 deployment，但服务器目录/容器不存在；
  - `运行中`：`dc:status` 显示容器运行；
  - `异常`：最近一次命令失败或容器异常退出。
- 右键菜单：打开项目目录、复制项目名、移除 Tab、查看配置。

#### 添加项目向导

> 下列步骤中的 `php admin init` 仅作 legacy 行为对照；桌面端直接写 SQLite + 生成到 `deployments/<id>/generated/`。

步骤 1：基本信息

```text
项目名      [ boyu            ]  仅允许字母/数字/下划线/短横线
远程路径    [ /data/phalcon-admin ]
Nginx 端口  [ 8071            ]
域名        [ gu19.cn          ]  [+ 添加域名]
同步方式    [✓] bundle  [ ] git  [ ] ftp
```

步骤 2：高级配置（可折叠）

- `sync.items` 条目：method / path / repo / branch / name / excludes；
- `env`：`MYSQL_DATABASE`、`MYSQL_USER`、`MYSQL_PASSWORD`、`REDIS_PASSWORD`、`TZ`；
- `docker.images`：PHP/Nginx/MySQL/Redis 镜像覆盖；
- `nginx.ssl`：是否已启用 SSL。

步骤 3：预览与创建

- 若 `deploy/projects/<name>/server.php` 不存在：
  - 调用 `php admin app:<name> init`；
  - CLI 会从 `deploy/server.php` 的 `application` 段生成项目配置后退出；
  - UI 提示“配置已生成，请检查后继续”，并打开配置预览。
- 若已存在：
  - 调用 `php admin app:<name> init`（不带 `-y`）生成 `.env`、compose、nginx、php.ini、config.php 等预览文件；
  - 展示生成文件清单与差异（首版可只列文件 + “打开目录”）。
- 步骤 4：正式部署
  - 按钮 **正式部署** → `php admin app:<name> init -y`；
  - 确认弹窗必须显示：项目名、服务器、远程路径、域名、将执行的命令。

> 注意：`init` 首次会创建配置并 `exit(0)`，UI 需要把它当成“配置生成”而不是失败，并自动进入下一步。
>
> 首次生成依赖 `deploy/server.php` 中存在 `application` 段；若不存在，CLI 会报错，UI 应提示先补全服务器配置模板。

### 5.3 项目概览

顶部状态卡（从左到右）：

1. **服务器**：当前服务器 host、连接状态；
2. **容器**：`docker compose ps` 摘要（运行/总数）；
3. **Nginx**：配置是否存在、SSL 是否启用、最近 reload 时间；
4. **最近部署**：最近一次 `init/upgrade` 的结果、耗时、时间；
5. **数据库备份**：最近备份时间、备份数量（可选）。

快捷操作区：

- `部署`（首次） → 跳转“部署与升级”；
- `升级代码` → 打开升级面板；
- `重启 PHP` → `app:<项目> dc:restart:php`；
- `查看日志` → 跳转 Docker 日志；
- `备份数据库` → 打开数据库页并预选“立即备份”。

底部“最近活动”列表：时间、命令、结果、耗时、查看日志。

### 5.4 部署与升级

#### 首次部署

```text
┌ 部署 boyu ─────────────────────────────────────────────┐
│ 目标服务器  prod (47.106.226.129)                       │
│ 远程路径    /data/phalcon-admin                         │
│ 同步项      bundle 主仓库 / ftp src/App/Projects/boyu   │
│ 域名        gu19.cn, www.gu19.cn                        │
│                                                        │
│ 执行命令                                                │
│ php admin app:boyu init -y                              │
│                                                        │
│ [预览配置]  [正式部署]                                  │
└────────────────────────────────────────────────────────┘
```

- **预览配置**：`app:<name> init`，只生成本地文件，不连服务器；
- **正式部署**：`app:<name> init -y`，执行目录准备、代码同步、配置上传、compose up、nginx、hooks；
- 首次部署前建议先跑 `server:init` 检查环境。

#### 升级代码

```text
┌ 升级 boyu ─────────────────────────────────────────────┐
│ 同步过滤   ( ) 全部   ( ) 按方式 [git ▾]   ( ) 按条目 [主仓库 ▾] │
│ FTP 选项   [ ] full（强制全量上传，忽略增量清单）        │
│ 配置       [ ] 同时更新配置（加 -y）                     │
│ 完成后     [✓] 重启 php 容器                             │
│                                                        │
│ 执行命令                                                │
│ php admin app:boyu upgrade                              │
│ php admin app:boyu dc:restart:php                       │
│                                                        │
│ [开始升级]                                              │
└────────────────────────────────────────────────────────┘
```

规则：

- `method` 与 `name` 二选一；UI 用单选/下拉保证不会同时传；
- 不带 `-y`：只同步代码，不覆盖配置、不重启；
- 带 `-y`：代码 + 配置 + hooks；
- CLI 帮助明确提示“更新代码后需要重启容器”，UI 提供“完成后重启 php”勾选项（本质是追加执行 `dc:restart:php`，需在预览中列出两条命令）；
- `full=1` 仅对 `ftp` 有意义，选择其他方式时禁用；
- 若项目 `sync.target=filesystem`，`init -y` / `upgrade -y` 只写本地目标目录，不会走 SSH/Docker；UI 应在预览中明确标注“本地文件系统目标”。

#### 运行过程

- 顶部步骤条：`准备目录 → 同步代码 → 生成配置 → 启动容器 → 更新 Nginx → 执行钩子`；
- 步骤状态来自终端输出关键字 + 退出码，解析失败时回退为“运行中”；
- 右侧显示耗时、当前步骤、取消按钮；
- 完成后弹出结果：成功/失败、耗时、日志路径、下一步建议。

### 5.5 Docker 容器

```text
容器状态
┌────────────┬──────────────┬─────────┬──────────────┐
│ 服务        │ 状态          │ 端口     │ 操作          │
├────────────┼──────────────┼─────────┼──────────────┤
│ nginx      │ ● running     │ 8071    │ [日志] [重启] │
│ php        │ ● running     │ -       │ [日志] [重启] │
│ mysql      │ ● running     │ -       │ [日志] [重启] │
│ redis      │ ● running     │ -       │ [日志] [重启] │
└────────────┴──────────────┴─────────┴──────────────┘
[刷新状态]  [全部操作 ▾：启动 / 重启 / 停止]
```

- PHP 镜像：`authus/phalcon:5.23.0`；旧 `5.13.0` 存在 PHP 版本漏洞，页面需明确提示；
- 全部重启使用 `docker compose up -d`，镜像 tag 变更时会重建容器、使用新镜像；
- 刷新：`app:<name> dc:status`（首版解析 `docker compose ps` 文本，失败则只显示终端）；
- 重启全部：`app:<name> dc:restart`（等价 `up -d`）；
- 重启单个：`app:<name> dc:restart:<服务>`；单服务 `restart` 不会重建镜像；
- 日志：`app:<name> dc:log[:<服务>]`，默认 `--tail=50`；
- 日志窗口支持关键字过滤、复制、保存；首版不做 follow，后续可加“持续跟踪”。

> 项目头部右侧“更多”按钮使用显式菜单：Nginx 配置 / 容器日志 / 复制项目名 / 重置代码，不再直接触发单一动作。

### 5.6 Nginx

#### 项目级

| 操作 | 命令 | 说明 |
|---|---|---|
| 上传/更新配置 | `app:<name> nginx:add` | 本地生成 + 上传 `/etc/nginx/conf.d/` |
| 强制重新生成 | `app:<name> nginx:add force=1` | 覆盖本地人工修改，需确认 |
| 移除域名 | `app:<name> nginx:remove` | 危险操作 |
| 申请 SSL | `app:<name> nginx:ssl -email=...` | 需要邮箱；会调用 certbot |

#### 全局

| 操作 | 命令 |
|---|---|
| 语法检查并重载 | `php admin nginx:reload` |
| 错误日志 | `php admin nginx:log:error` |
| 下载错误日志 | `php admin nginx:log:error --save` |
| 访问日志 | `php admin nginx:log:access` |
| 下载访问日志 | `php admin nginx:log:access --save` |

UI 细节：

- SSL 表单：邮箱必填，显示域名列表；
- 申请成功后提示“`nginx.ssl` 已写入项目配置”，并刷新概览；
- `nginx:remove` 必须输入项目名确认；
- 日志页支持“实时/快照”切换，首版快照即可。

### 5.7 数据库

```text
数据库
┌──────────────────────────────────────────────────────────────┐
│ [结构迁移]  [Seed 基础数据]  [立即备份]  [刷新备份列表]       │
├──────────────────────────────────────────────────────────────┤
│ 备份列表                                                     │
│ 2026-10-24_030001.sql.gz   18.4 MB   [下载]                  │
│ 2026-10-23_030001.sql.gz   18.2 MB   [下载]                  │
├──────────────────────────────────────────────────────────────┤
│ 定时备份  [✓ 已启用]  时间 [03:00]  保留 [7] 天  [更新] [移除]│
├──────────────────────────────────────────────────────────────┤
│ 临时访问  [SSH 隧道] 本地端口 [13306]  [启动]                │
│           [phpMyAdmin] 端口 [13307]  [部署] [删除]           │
└──────────────────────────────────────────────────────────────┘
```

命令映射：

| 操作 | 命令 |
|---|---|
| 结构迁移 | `app:<name> db:migrate` |
| Seed | `app:<name> db:seed` |
| 立即备份 | `app:<name> db:backup` |
| 备份并下载 | `app:<name> db:backup download=1` |
| 备份列表 | `app:<name> db:backup:list` |
| 下载指定备份 | `app:<name> db:backup:get <文件名>` |
| 安装定时备份 | `app:<name> db:backup:cron -y time=03:00 keep=7` |
| 移除定时备份 | `app:<name> db:backup:cron-rm` |
| SSH 隧道 | `app:<name> db:proxy local=13306` |
| 部署 phpMyAdmin | `app:<name> db:pma host=13307` |
| 删除 phpMyAdmin | `app:<name> db:pma-rm` |

特殊交互：

- **SSH 隧道是阻塞命令**：`db:proxy` 会一直运行到用户停止。UI 必须把它作为后台任务，显示“隧道运行中”，提供“停止隧道”，不能锁死窗口；
- **phpMyAdmin 首次部署可能等待镜像拉取**（最长约 3 分钟）：显示进度/日志，不要超时退出；
- 备份下载落到 `deploy/backups/<项目>/`，完成后提供“打开目录”；
- `db:backup:cron` 必须带 `-y` 才会真正安装，UI 用确认弹窗替代；
- 下载文件名从 `db:backup:list` 输出解析；解析失败时提供手动输入。

### 5.8 脚本与日志

脚本来源：项目 `server.php` 的 `scripts` 目录 / 内置脚本。

内置脚本：

| 脚本 | 命令 | 用途 |
|---|---|---|
| doctor | `app:<name> run doctor` | 一键健康检查 |
| log-tail | `app:<name> run log-tail <类型> [行数]` | 查看日志（error/app/sql/nginx/access/mysql-slow） |
| log-errors | `app:<name> run log-errors [日期]` | 错误汇总 |
| log-slow | `app:<name> run log-slow [阈值] [行数]` | 慢查询分析 |

UI：

- “推送脚本”按钮 → `app:<name> script:push`；
- 脚本列表以按钮/下拉呈现，参数用表单；
- 运行结果进入当前项目终端；
- 常用命令（doctor、错误日志）固定在“脚本与日志”页顶部。

### 5.9 终端抽屉

每个项目一个终端实例：

```text
┌─ 终端 ─────────────────────────────────────────────────────────────┐
│ [boyu] [yihe] [server]                         [清空] [复制] [保存] │
├────────────────────────────────────────────────────────────────────┤
│ $ php admin app:boyu upgrade -y                                    │
│ → [1/2] [bundle] 主仓库                                             │
│ ✔ 代码同步完成                                                      │
│ ...                                                                │
└────────────────────────────────────────────────────────────────────┘
```

能力：

- ANSI 颜色渲染（CLI 输出含 `\033[...]`）；
- stdout / stderr 合并显示，stderr 用淡红色前缀或颜色区分；
- 自动滚动到底部，用户上滚时暂停自动滚动；
- 显示当前命令、开始时间、耗时、退出码；
- “停止”发送中断信号（Windows 使用 `taskkill /T`，Unix 发送 `SIGINT`/`SIGTERM`）；
- 输出写入本地日志文件，路径展示在状态栏；
- 命令执行前在终端顶部显示等价 CLI 命令，方便复制到真实终端。

### 5.10 设置

| 分组 | 项 |
|---|---|
| 常规 | 主题（跟随系统/浅色/深色）、语言（中文/English）、启动时恢复上次窗口 |
| 运行环境 | 数据目录、SSH 可执行文件路径、日志目录 |
| 终端 | 字号、字体、滚动行数、是否保存日志 |
| 安全 | 危险操作二次确认、是否在日志中隐藏密码/密钥 |
| 服务器 | 服务器配置文件列表、默认服务器 |
| 高级 | 命令超时、并发任务数、日志目录、打开数据目录 |

设置持久化到 Tauri Store；不把 SSH 密码写入前端 localStorage。

---

## 六、CLI 命令映射总表

> 下表是 **legacy 行为对照/回退参考**；桌面端由 Rust 原生 service 实现，不默认执行 `php admin`。保留命令列是为了验收语义与应急回退。

### 6.1 全局

| UI 操作 | 命令 | 危险级别 |
|---|---|---|
| 环境检查 | `php admin server:init` | 只读 |
| 安装/初始化服务器 | `php admin server:init -y` | 高 |
| Nginx 语法检查并重载 | `php admin nginx:reload` | 中 |
| Nginx 错误日志 | `php admin nginx:log:error` | 只读 |
| 下载错误日志 | `php admin nginx:log:error --save` | 只读 |
| Nginx 访问日志 | `php admin nginx:log:access` | 只读 |
| 下载访问日志 | `php admin nginx:log:access --save` | 只读 |
| 导出配置（内部/高级） | `php admin config:export [项目]` | 只读 |

### 6.2 项目

| UI 操作 | 命令 | 危险级别 |
|---|---|---|
| 生成配置预览 | `php admin app:<p> init` | 低（本地写文件） |
| 正式部署 | `php admin app:<p> init -y` | 高 |
| 升级代码 | `php admin app:<p> upgrade` | 中 |
| 升级代码 + 配置 | `php admin app:<p> upgrade -y` | 高 |
| 按方式过滤 | `... upgrade method=git` | 中 |
| 按条目过滤 | `... upgrade name=<条目>` | 中 |
| FTP 全量 | `... upgrade full=1` | 中 |
| 推送配置 | `php admin app:<p> push` | 中 |
| 推送脚本 | `php admin app:<p> script:push` | 中 |
| 运行脚本 | `php admin app:<p> run <脚本> [参数...]` | 低/中 |
| 重置代码 | `php admin app:<p> reset` | 高 |
| 生成 Git SSH key | `php admin app:<p> git:ssh` | 中 |
| 验证 Git SSH | `php admin app:<p> git:ssh -T` | 只读 |

### 6.3 Docker / Nginx / 数据库

| UI 操作 | 命令 | 危险级别 |
|---|---|---|
| 容器状态 | `php admin app:<p> dc:status` | 只读 |
| 重启全部 | `php admin app:<p> dc:restart` | 中 |
| 重启指定容器 | `php admin app:<p> dc:restart:<服务>` | 中 |
| 全部容器日志 | `php admin app:<p> dc:log` | 只读 |
| 指定容器日志 | `php admin app:<p> dc:log:<服务>` | 只读 |
| 上传 Nginx 配置 | `php admin app:<p> nginx:add` | 中 |
| 强制重新生成 Nginx | `php admin app:<p> nginx:add force=1` | 中 |
| 移除域名 | `php admin app:<p> nginx:remove` | 高 |
| 申请 SSL | `php admin app:<p> nginx:ssl -email=<email>` | 中 |
| 结构迁移 | `php admin app:<p> db:migrate` | 高 |
| Seed | `php admin app:<p> db:seed` | 高 |
| 立即备份 | `php admin app:<p> db:backup` | 中 |
| 备份并下载 | `php admin app:<p> db:backup download=1` | 中 |
| 备份列表 | `php admin app:<p> db:backup:list` | 只读 |
| 下载备份 | `php admin app:<p> db:backup:get [文件]` | 只读 |
| 安装定时备份 | `php admin app:<p> db:backup:cron -y [time=03:00] [keep=7]` | 高 |
| 移除定时备份 | `php admin app:<p> db:backup:cron-rm` | 高 |
| SSH 隧道 | `php admin app:<p> db:proxy [local=13306]` | 中（阻塞） |
| 部署 phpMyAdmin | `php admin app:<p> db:pma [host=13307]` | 中 |
| 删除 phpMyAdmin | `php admin app:<p> db:pma-rm` | 中 |

### 6.4 多服务器

- `php admin --env=staging <命令>` 选择 `deploy/server.staging.php`；
- UI 的服务器选择器对应 `--env`，禁止把 `--env` 混入项目参数；
- 切换服务器后，项目列表保持不变，但状态、终端、运行历史按服务器隔离。

---

## 七、交互状态与错误处理

### 7.1 任务状态

```text
queued → running → succeeded
                 ├→ failed（非 0 退出码）
                 ├→ canceled（用户停止）
                 └→ timeout（可选）
```

- 每个任务显示：项目、服务器、命令、开始/结束时间、退出码、日志路径；
- 退出码非 0 时，错误卡片显示最后 20 行 stderr，并提供“打开完整日志”；
- 取消任务后不自动重试，保留已产生输出；
- 同一项目默认串行执行，避免同时改配置/容器；全局服务器命令可并行，但需在 UI 标注。

### 7.2 首次使用流程

```text
启动
  → 检测项目根目录（含 admin）
  → 检测 PHP 可执行文件（设置可改）
  → 读取 deploy/server.php / server.*.php
  → 选择服务器
  → 环境检查（建议）
  → 扫描 deploy/projects/*
  → 选择项目 / 添加项目
```

异常分支：

- 找不到 `admin`：引导选择项目根目录；
- 找不到 PHP：引导设置 PHP 路径，并显示 `php -v` 结果；
- 服务器配置不存在：提示复制 `deploy/server.example.php`，提供“打开目录”；
- SSH 连接失败：展示 stderr，不弹无意义 toast；
- 项目 `server.php` 解析失败：显示文件路径和 PHP 错误，提供“打开文件”。

### 7.3 危险操作确认

必须二次确认的操作：

- `server:init -y`
- `app:<p> init -y`
- `app:<p> upgrade -y`
- `app:<p> reset`
- `app:<p> nginx:remove`
- `app:<p> db:migrate`
- `app:<p> db:seed`
- `app:<p> db:backup:cron` / `cron-rm`
- 切换服务器时终止运行中的任务

确认弹窗必须包含：

- 项目名、服务器、远程路径；
- 将执行的完整命令；
- 影响说明（如“会覆盖远程配置”“会删除 Nginx 配置”“会重建数据库结构”）；
- 主按钮使用危险色，默认焦点在“取消”。

---

## 八、视觉规范

### 8.1 风格

- 开发者工具风格：信息密度高、边界清晰、少装饰；
- 浅色/深色双主题，默认跟随系统；
- 状态色只用于状态点/边框，不作为唯一信息载体（同时显示文字）。

### 8.2 颜色 Token（建议）

| Token | 浅色 | 深色 | 用途 |
|---|---|---|---|
| `bg` | `#f6f7f9` | `#0f1115` | 窗口背景 |
| `panel` | `#ffffff` | `#171a21` | 卡片/面板 |
| `border` | `#e5e7eb` | `#2a2f3a` | 分割线 |
| `text` | `#111827` | `#e5e7eb` | 主文本 |
| `muted` | `#6b7280` | `#9ca3af` | 次要文本 |
| `primary` | `#2563eb` | `#60a5fa` | 主按钮/选中 |
| `success` | `#16a34a` | `#4ade80` | 运行中/成功 |
| `warning` | `#d97706` | `#fbbf24` | 警告 |
| `danger` | `#dc2626` | `#f87171` | 失败/危险操作 |
| `terminal` | `#0b1020` | `#0b1020` | 终端背景 |

### 8.3 字体与尺寸

- UI 字体：`system-ui, -apple-system, "Segoe UI", "Noto Sans SC", sans-serif`；
- 终端字体：`"JetBrains Mono", "Cascadia Code", Consolas, monospace`；
- 字号：正文 `13px`，标题 `15–20px`，终端 `12–13px`；
- 圆角：`6px`（卡片/按钮）、`8px`（弹窗）；
- 间距：`4 / 8 / 12 / 16 / 24`。

### 8.4 图标与动效

- 图标使用 Lucide 或同等线性图标；
- 动效只用于状态变化（执行中旋转、Tab 切换淡入），时长 `120–180ms`；
- 终端输出不做逐字动画。

---

## 九、Tauri 实现建议

> 本节以 [architecture.md](architecture.md) 为准；以下只保留 UI 相关实现要点。

### 9.1 进程模型

```text
┌─────────────────────────────┐
│ Tauri WebView（UI）          │
│  React + TypeScript          │
└──────────────┬──────────────┘
               │ invoke / event
┌──────────────▼──────────────┐
│ Rust 后端                     │
│ - SQLite repository          │
│ - deployments / runs         │
│ - SSH / SFTP / git / Docker  │
│ - Nginx / DB / SSL           │
│ - task manager / lock        │
└──────────────┬──────────────┘
               │ 仅迁移/回退
┌──────────────▼──────────────┐
│ php admin（legacy，可选）      │
└─────────────────────────────┘
```

建议：

- Tauri 2；
- 前端只通过 Rust command 访问数据/执行任务；
- 进程参数使用 argv 数组，禁止 `shell: true` 拼接；
- 终端使用 PTY（`portable-pty` + `xterm.js`）；
- stdout/stderr 通过 Tauri event 流式推送，事件带 `runId`；
- 取消任务终止整个进程树；
- 桌面端不依赖 PHP；PHP 仅用于 legacy 导入/回退。

### 9.2 结构化数据

- SQLite 是唯一可写事实源；
- 状态卡片直接读 `deployments` / `runs`，不解析 CLI 文本；
- 旧 CLI 的文本输出只用于 legacy 导入和差分测试；
- 需要兼容旧输出时，解析失败必须降级为“查看原始日志”，不能误报成功。

### 9.3 本地数据

| 数据 | 位置/方式 |
|---|---|
| 服务器/项目/部署/同步项 | SQLite |
| 运行历史 | SQLite `runs` |
| 命令日志 | `<app_data>/deployments/<id>/logs/<runId>.log` |
| 生成配置 | `<app_data>/deployments/<id>/generated/` |
| 备份文件 | `<app_data>/deployments/<id>/backups/` |
| 窗口/主题/最近项目 | Tauri Store |
| 密钥/密码 | OS keychain，SQLite 只存引用 |

### 9.4 安全

- WebView 不直接执行 SQL 或任意 shell；
- Rust 侧校验参数，项目名 `[a-zA-Z0-9_-]+`；
- 日志脱敏 `password`、`secret`、`token`、`key`；
- 禁用远程 URL，CSP 只允许本地资源；
- 危险操作必须经 Rust 侧再次校验。

---

## 十、MVP 与迭代

### Phase 0：可运行骨架

- 选择数据目录 / 导入 legacy 配置；
- 服务器列表与项目列表；
- 终端流式输出；
- 只读能力：服务器检查、容器状态、容器日志、nginx 日志。

### Phase 1：部署闭环

- 添加项目向导；
- 原生 init 预览/正式部署；
- 原生 upgrade（method/name/full）；
- 容器 restart/logs；
- Nginx reload/add/remove；
- 运行历史与日志保存。

### Phase 2：数据库与 SSL

- 数据库页：migrate/seed/backup/list/download；
- 定时备份；
- SSL 申请；
- SSH 隧道与 phpMyAdmin（后台任务 + 停止）。

### Phase 3：体验增强

- 状态卡片直接读 SQLite；
- 配置编辑器；
- 多服务器并发任务；
- 命令收藏、最近使用、快捷键；
- 自动更新、签名、安装包。

### 非目标

- 不做 Web 版；
- 不做服务器端 agent；
- 不长期保留 PHP 作为桌面端执行主路径。

---

## 十一、验收标准

1. 每个 legacy `php admin` 能力都能在 UI 中找到对应入口，且预览语义一致；
2. 执行 init/upgrade 前能看到目标服务器、项目、远程路径和影响范围；
3. 命令输出实时流式显示，ANSI 颜色正确，stderr 可见；
4. 长任务可取消，取消后进程树被终止，UI 不残留“运行中”；
5. `db:proxy` 不阻塞主界面，可随时停止；
6. 危险操作必须二次确认，确认文案包含项目和服务器；
7. 切换服务器后项目状态、终端、历史正确隔离；
8. 关闭并重启应用后，窗口布局、主题、最近项目、运行历史可恢复；
9. 日志中不出现明文密码/密钥；
10. Windows 与 macOS 至少各完成一次真实服务器部署验证。

---

## 十二、待确认问题

已确定：

- 前端：React + TypeScript + Vite；
- 桌面：Tauri 2；
- 执行内核：Rust 原生，不依赖 PHP；
- 主数据源：SQLite；
- PHP deploy：仅迁移/差分/回退。

仍需确认：

1. SQLite 访问层用 `sqlx` 还是 `rusqlite`？
2. SSH 用纯 Rust（russh/ssh2）还是系统 `ssh`？
3. 密钥首版是否直接接 OS keychain，还是先用环境变量过渡？
4. 旧 vbox/vbox2 配置是否只做手工参考，不自动导入？
5. 是否需要中英文双语？
6. 是否需要自动更新与代码签名？
7. 终端是否需要完整 PTY 交互（如 certbot 提示输入），还是先做只读流式输出？
