# 模板管理需求评估

> 状态：已实现（全局模板 + 覆盖模型 + 编辑/预览/自定义路径；暂不做版本备份）  
> 日期：2026-10-26  
> 需求来源：顶部工具栏新增「模板管理」，支持两种方案；模板内容来自旧仓库 `deploy/template/`

---

## 1. 需求理解

### 1.1 目标文件

| 部署内路径 | 用途 | 旧模板文件 |
|---|---|---|
| `docker/mysql/my.cnf` | MySQL 容器配置 | `mysql/my.cnf` |
| `docker/nginx/default.conf` | **容器内** Nginx 站点配置 | `nginx/default.conf` |
| `docker/php/php.ini` | PHP 容器配置 | `php/php.ini` |
| `.env` | compose 环境变量 | `.env.example` |
| `src/config/config.php` | 应用配置 | `config.php.template` |
| `docker-compose.yaml` | 容器编排 | `docker-compose.yaml` |
| 自定义路径 | 「添加模板文件」 | 无 |

注意：这里的 `docker/nginx/default.conf` 是**项目容器内**的 Nginx 配置，和桌面端生成的宿主机 Nginx 配置（`render_nginx`）是两回事，UI 上必须区分，避免用户改错。

### 1.2 两种方案

- **方法一**：模板存 SQLite（路径 + 内容），「模板管理」一级菜单，二级菜单列出文件；点击弹出编辑对话框；支持「添加模板文件」。
- **方法二**：在项目目录创建模板目录，点击「模板管理」调用本机 VS Code 打开目录编辑。

### 1.3 需求中的矛盾点

需求最后一句「点击的弹出修改表单对话框」属于方法一；方法二则是外部编辑器。两者不能同时作为主路径，需要先定一个事实源。

另外「在项目目录中创建模板目录」会带来两个问题：

1. 模板是**应用级**还是**项目级**？如果放在项目仓库里，同一台机器多个项目会有多份模板，部署行为随项目漂移，且会污染项目 git 工作区；
2. 「模板管理」入口不依赖当前选中项目时，无法确定该打开哪个目录。

建议：模板目录放在**应用数据目录**（`%APPDATA%\cn.gu19.phadmin.desktop\templates\`），而不是项目仓库；项目级覆盖以后再做。

---

## 2. 现状事实

1. 当前 Rust 端模板是**编译进二进制**的：
   - `src-tauri/templates/env.example`
   - `src-tauri/templates/docker-compose.yaml`
   - `src-tauri/templates/config.php.template`
   - 通过 `include_str!` 加载，改模板必须重新编译。
2. `docker/mysql/my.cnf`、`docker/nginx/default.conf`、`docker/php/php.ini` **目前不是模板渲染**：
   - `prepare_runtime_assets` 只是从项目仓库里的 `my.example.cnf` / `default.example.conf` / `php.prod.ini` 复制一份到目标文件；
   - 要支持这三项模板，需要新增渲染 + 上传逻辑。
3. 旧仓库模板占位符情况：
   - `.env.example`：12 个 `{{KEY}}`
   - `config.php.template`：`{{CONFIG_OVERRIDES}}`
   - `php/php.ini`：`{{TZ}}`
   - `nginx/default.conf`、`mysql/my.cnf`：无占位符
   - `docker-compose.yaml`：使用 `${VAR}`，这是 **Docker Compose 自身的插值**，不是应用渲染
4. 旧渲染器 `TemplateRenderer` 只做 `str_replace('{{KEY}}', value)`，没有未知变量校验、没有预览、没有版本。
5. 当前 `.env` / `docker-compose.yaml` 已经和旧模板分叉（新增 `DOCKER_NETWORK`、共享外部网络），**不能直接用旧文件覆盖当前默认模板**。
6. 项目/部署创建流程尚未实现；模板预览暂时只能使用默认变量或已有 deployment。

---

## 3. 方案对比

| 维度 | 方法一：SQLite + 应用内编辑 | 方法二：本地目录 + VS Code |
|---|---|---|
| 事实源 | SQLite，应用完全掌控 | 磁盘文件，外部程序可改 |
| 编辑体验 | 普通 textarea / 后续可换编辑器 | VS Code，体验最好 |
| 变量提示 / 校验 | 可做（未知变量、YAML/ENV 校验） | 做不了，只能部署时失败 |
| 预览 | 可按 deployment 渲染预览 | 无 |
| 版本/回滚 | 可加修订表；至少可恢复默认 | 依赖用户自己的 git/备份 |
| 依赖 | 无外部依赖 | 必须安装 VS Code，且要定位 `code` 命令 |
| 一致性 | 强，DB 是唯一入口 | 弱，磁盘被手改后应用无感知 |
| 多项目 | 天然全局 | 目录放在项目里会分叉 |
| 实现成本 | 约 1.5–2 天（含 UI/校验/测试） | 约 0.5 天，但后续校验成本高 |
| 适合场景 | 产品化、需要可控部署 | 个人临时改配置 |

### 3.1 方法一的优点

- 与现有架构一致：SQLite 已是唯一事实源，部署渲染从 DB 读模板是自然延伸；
- 可以做**保存前预览**：选一个 deployment，渲染出最终文件，确认无误再保存；
- 可以做**未知变量检查**，避免把 `{{TYPO}}` 原样发到服务器；
- 可做「恢复内置默认」：内置模板随应用升级，用户改动只作为覆盖层；
- 不依赖用户装 VS Code，跨平台一致。

### 3.2 方法一的缺点

- 普通 textarea 编辑 2–3 KB 的 nginx/php.ini 尚可，再大就需要引入代码编辑器（CodeMirror/Monaco），增加前端体积；
- 需要设计 schema、迁移、默认模板回退、渲染改造；
- 模板是全局的，改一次影响所有项目/服务器，需要明确的确认与预览。

### 3.3 方法二的优点

- 实现最快，几乎不动渲染管线（部署时读文件即可）；
- VS Code 的语法高亮、搜索替换、多光标体验远好于自研弹窗；
- 用户可以直接用 git 管理模板目录。

### 3.4 方法二的缺点

- 无法在应用内校验/预览，模板写错只能在部署时暴露，甚至部署到一半失败；
- 需要处理 VS Code 不存在、路径含空格、`code` 命令未加入 PATH 的降级；
- 文件被外部修改后应用无感知，容易出现「UI 显示的模板」和「实际部署的模板」不一致；
- 如果目录放在项目仓库，会污染 git 工作区，并且多项目行为不一致；
- 与需求里的「弹出修改表单」直接冲突。

---

## 4. 推荐方案

**以方法一为主，方法二作为辅助入口（混合方案）**：

1. **SQLite 为唯一事实源**：`templates` 表只存「覆盖」和「自定义」模板；内置默认仍留在 `src-tauri/templates/`，通过 `include_str!` 编译进应用。
2. **解析顺序**：`templates` 行存在 → 用行内容；否则用内置默认。这样：
   - 空库不需要 seed 模板；
   - 应用升级时内置模板自动更新，除非用户改过；
   - 「恢复默认」= 删除该行。
3. **编辑弹窗**内提供：
   - 全屏/大尺寸等宽 textarea；
   - 变量面板（列出可用 `{{KEY}}`，点击插入）；
   - 「预览」：选 deployment 渲染，显示结果与未解析变量；
   - 「保存」「取消」「恢复默认」（内置）/「删除」（自定义）。
4. **方法二作为按钮**：编辑弹窗里放「用 VS Code 打开」——把当前模板导出到应用数据目录并打开该文件/目录，仅作为高级用户 escape hatch；不把外部文件作为部署事实源。
5. **变量语法保持 `{{KEY}}`**：
   - 与旧模板、当前 `.env`/`config.php` 一致；
   - `docker-compose.yaml` 里的 `${VAR}` 保留不动，由 compose 自己插值；
   - 变量名限制 `[A-Z0-9_]+`，渲染时未知变量直接报错，而不是留空。

---

## 5. 详细设计（建议）

### 5.1 数据表

```sql
CREATE TABLE IF NOT EXISTS templates (
  path        TEXT PRIMARY KEY,          -- 部署内相对路径，如 docker/mysql/my.cnf
  content     TEXT NOT NULL,
  kind        TEXT NOT NULL DEFAULT 'override'
              CHECK (kind IN ('override', 'custom')),
  updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
```

- `override`：六个内置路径之一，删除即恢复默认；
- `custom`：「添加模板文件」创建的路径，可删除；
- 可选后续加 `template_revisions(path, content, created_at)` 做历史版本，MVP 可不做。

### 5.2 内置模板清单

```text
.env                         -> templates/env.example
docker-compose.yaml          -> templates/docker-compose.yaml
src/config/config.php        -> templates/config.php.template
docker/mysql/my.cnf          -> templates/mysql/my.cnf        （新增，来自旧模板）
docker/nginx/default.conf    -> templates/nginx/default.conf  （新增，来自旧模板）
docker/php/php.ini           -> templates/php/php.ini         （新增，来自旧模板）
```

迁移时以**当前应用模板**为准，旧仓库只用于补齐 my.cnf / default.conf / php.ini，避免把已经修正过的 `DOCKER_NETWORK`、共享网络改回旧版。

### 5.3 渲染流程改造

现在：

```text
render_env() / COMPOSE_TEMPLATE / render_config_php()
prepare_runtime_assets() 复制 my.cnf / default.conf / php.ini
apply_generated_config() 上传 .env / nginx.conf / compose / config.php
```

改为：

```text
resolve_template(conn, path) -> String   // DB 覆盖 ?? 内置默认
render_template(content, vars) -> Result<String>  // 替换 {{KEY}}，未知变量报错
```

- `render_env`、compose、config.php 都走 `resolve_template`；
- `my.cnf` / `default.conf` / `php.ini` 渲染后与其它生成文件一起上传；
- `prepare_runtime_assets` 的复制逻辑只在「模板不存在」时作为兜底，最终会被渲染文件覆盖；
- 宿主机 nginx 配置仍由代码生成，不进入模板表（避免和容器内 nginx 混淆）。

### 5.4 变量集合

建议固定一份变量注册表（Rust 常量 + 前端展示）：

```text
APP_NAME, PROJECT_NAME, PROJECT_PATH, REMOTE_PATH, DATA_PATH_HOST,
TZ, NGINX_PORT, DOCKER_NETWORK,
MYSQL_USER, MYSQL_DATABASE, MYSQL_PASSWORD,
REDIS_PASSWORD,
NGINX_IMAGE, PHP_IMAGE, MYSQL_IMAGE, REDIS_IMAGE
```

`config.php` 额外保留 `{{CONFIG_OVERRIDES}}`，由现有 JSON → PHP 渲染逻辑注入，不允许用户手改其含义。

### 5.5 校验

- 未解析的 `{{...}}` → 保存时警告、部署前报错；
- `docker-compose.yaml`：`serde_yaml` 解析校验；
- `.env`：每行 `KEY=VALUE` 校验；
- `my.cnf` / `php.ini` / `default.conf`：至少做非空、UTF-8、无 NUL 检查；
- 真正的 nginx 语法仍以部署时容器内 `nginx -t` 为准（已有 reload 步骤会暴露错误）；
- 上传前对目标文件做快照（复用现有配置备份机制）。

### 5.6 自定义路径安全

「添加模板文件」需要严格校验：

- 只允许项目内相对路径，禁止绝对路径、盘符、`..`、`.` 段；
- 字符集 `[A-Za-z0-9._/-]`，禁止空段和结尾 `/`；
- 单文件大小上限（如 512 KB）；
- 上传范围必须在 `remote_path` 内；
- 自定义模板可能覆盖 `src/` 代码文件，保存/部署时要二次确认。

### 5.7 UI

- 顶栏「模板管理」下拉，列出六个内置模板（已覆盖的显示标记）+ 分隔线 + 「添加模板文件」；
- 点击打开大尺寸编辑弹窗（约 90vw × 85vh）：
  - 顶部：路径、覆盖状态、更新时间；
  - 中间：等宽 textarea；
  - 右侧/顶部：变量 chips、预览、保存、取消、恢复默认、删除、用 VS Code 打开；
- 预览弹窗显示渲染结果和未解析变量；
- 危险操作（删除自定义、覆盖内置）二次确认。

---

## 6. 工作量与分期

| 阶段 | 内容 | 预估 |
|---|---|---|
| P1 | 表结构、模板解析、内置默认、渲染接入、单元测试 | 0.5–1 天 |
| P2 | 顶栏菜单、编辑弹窗、变量面板、保存/恢复默认/删除、添加模板 | 1 天 |
| P3 | 预览、YAML/ENV 校验、上传前备份、VS Code 打开 | 0.5 天 |

方法二单独实现约 0.5 天，但后续要补校验/一致性，总成本不会更低。

---

## 7. 待确认

1. 模板是**全局**（所有项目共用）还是**按项目**覆盖？建议先全局，后续再加项目级覆盖。
2. 是否接受「只存覆盖，内置默认随版本升级」的模型？（推荐）
3. 变量语法是否确认用 `{{KEY}}`，compose 内 `${VAR}` 保留？
4. 「添加模板文件」是否允许写入 `src/` 代码路径，还是只允许 `docker/`、`.env`、`docker-compose.yaml` 等配置路径？
5. 是否需要模板历史版本/回滚，还是「恢复默认 + 部署前备份」就够？
