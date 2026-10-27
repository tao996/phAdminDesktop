# 数据模型

> 状态：Phase 1 已实现 `servers/projects/deployments` + `domains/env_vars/sync_items/deployment_settings/runs/deploy_runs` 表、seed、`load_workspace`、legacy 导入
> 数据库：SQLite 3（WAL）
> 关联：[architecture.md](architecture.md)

---

## 1. 设计原则

1. **SQLite 是唯一可写事实源**：服务器、项目、部署关系、运行历史都在 SQLite；
2. **文件只存大对象**：日志、备份、生成的配置放文件系统，SQLite 存路径/元数据；
3. **多服务器用 `deployment` 隔离**：同一项目可绑定多台服务器；
4. **敏感信息不落库**：SSH 密码、私钥口令、环境变量 secret 存 OS keychain，仅存引用；
5. **迁移可回滚**：每次 schema migration 前自动备份数据库。

---

## 2. 实体关系

```text
servers 1 ─── n deployments n ─── 1 projects
                    │
                    ├── sync_items
                    ├── domains
                    ├── env_vars
                    ├── deployment_settings (1:1)
                    └── runs
```

- `servers`：SSH 服务器或本地 filesystem 目标；
- `projects`：代码项目（逻辑实体）；
- `deployments`：项目在某台服务器上的具体部署；
- `sync_items`：该部署的代码同步条目（git/bundle/ftp）；
- `runs`：该部署的命令执行记录。

**核心唯一约束**：`deployments(server_id, project_id)` 唯一。
同一项目可以同时存在 `prod` 和 `staging` 两条 deployment，互不覆盖。

---

## 3. 表结构

### 3.0 `templates`

只存用户覆盖/自定义模板；内置默认编译在应用里（`src-tauri/templates/`）。

```sql
CREATE TABLE templates (
  path        TEXT PRIMARY KEY,          -- 项目内相对路径，如 docker/mysql/my.cnf
  content     TEXT NOT NULL,
  kind        TEXT NOT NULL DEFAULT 'override'
              CHECK (kind IN ('override', 'custom')),
  updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
```

解析顺序：`templates` 行 → 内置默认。删除行即恢复默认；自定义路径同样落在该表，
渲染后写入 `generated/<deployment>/`，并通过 `template-manifest.json` 在上传阶段按原路径上传。

### 3.1 `servers`

```sql
CREATE TABLE servers (
  id            TEXT PRIMARY KEY,
  name          TEXT NOT NULL UNIQUE,      -- 自动生成 user@host:port
  host          TEXT NOT NULL,
  environment   TEXT NOT NULL,             -- 当前固定 production
  status        TEXT NOT NULL,             -- 当前固定 checking
  latency       TEXT NOT NULL,             -- 占位 —
  kind          TEXT NOT NULL DEFAULT 'ssh',
  port          INTEGER NOT NULL DEFAULT 22,
  username      TEXT NOT NULL DEFAULT '',  -- SSH 用户
  key_path      TEXT NOT NULL DEFAULT '',  -- 私钥认证时的私钥路径
  password      TEXT NOT NULL DEFAULT '',  -- 密码认证（明文存 SQLite，用户明确要求）
  passphrase    TEXT NOT NULL DEFAULT '',  -- 私钥口令（同上）
  docker_network TEXT NOT NULL DEFAULT 'phalcon-shared',  -- 共享 Docker 网络名
  created_at    TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
);
```

认证方式由字段组合决定：

- `key_path` 非空 → 私钥认证（`passphrase` 可选）；
- 否则 → 密码认证（`password`）。

> 注意：SSH 密码/口令按用户要求直接存 SQLite 明文（不再写 OS 凭据管理器）。
> 数据库文件与备份需要按敏感文件管理。环境变量（`env_vars`）的密文仍走 keychain。

`docker_network` 用于 compose 的共享外部网络：

- 渲染 `.env` 时写入 `DOCKER_NETWORK`；
- `docker-compose.yaml` 以 `external: true` 引用该网络；
- 部署前若网络不存在，会自动执行 `docker network create <name>`。

### 3.2 `projects`

```sql
CREATE TABLE projects (
  id            TEXT PRIMARY KEY,
  name          TEXT NOT NULL UNIQUE,
  repo_url      TEXT,
  local_path    TEXT,
  created_at    TEXT NOT NULL,
  updated_at    TEXT NOT NULL
);
```

### 3.3 `deployments`

```sql
CREATE TABLE deployments (
  id            TEXT PRIMARY KEY,
  server_id     TEXT NOT NULL REFERENCES servers(id) ON DELETE RESTRICT,
  project_id    TEXT NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
  remote_path   TEXT NOT NULL,
  nginx_port    INTEGER NOT NULL DEFAULT 8071,
  ssl           INTEGER NOT NULL DEFAULT 0 CHECK (ssl IN (0, 1)),
  sync_target   TEXT NOT NULL DEFAULT 'remote'
                CHECK (sync_target IN ('remote', 'filesystem')),
  status        TEXT NOT NULL DEFAULT 'unknown',
  created_at    TEXT NOT NULL,
  updated_at    TEXT NOT NULL,
  UNIQUE (server_id, project_id)
);
```

`status` 建议值：`unknown / not_deployed / running / degraded / failed / disabled`。
`status` 是 UI 缓存，不替代 `runs` 的真实结果。

### 3.4 `sync_items`

```sql
CREATE TABLE sync_items (
  id            TEXT PRIMARY KEY,
  deployment_id TEXT NOT NULL REFERENCES deployments(id) ON DELETE CASCADE,
  sort          INTEGER NOT NULL DEFAULT 0,
  method        TEXT NOT NULL CHECK (method IN ('git', 'bundle', 'ftp')),
  name          TEXT NOT NULL DEFAULT '',
  path          TEXT NOT NULL DEFAULT '',
  repo          TEXT NOT NULL DEFAULT '',
  branch        TEXT NOT NULL DEFAULT 'main',
  excludes_json TEXT NOT NULL DEFAULT '[]'
);
```

- `path=''` 表示主仓库；
- `name` 用于按条目过滤；
- `ftp` 不支持主仓库；
- `excludes_json` 是 JSON 数组字符串。

### 3.5 `domains`

```sql
CREATE TABLE domains (
  id            TEXT PRIMARY KEY,
  deployment_id TEXT NOT NULL REFERENCES deployments(id) ON DELETE CASCADE,
  domain        TEXT NOT NULL,
  sort          INTEGER NOT NULL DEFAULT 0,
  UNIQUE (deployment_id, domain)
);
```

### 3.6 `env_vars`

```sql
CREATE TABLE env_vars (
  id            TEXT PRIMARY KEY,
  deployment_id TEXT NOT NULL REFERENCES deployments(id) ON DELETE CASCADE,
  key           TEXT NOT NULL,
  value         TEXT,
  is_secret     INTEGER NOT NULL DEFAULT 0 CHECK (is_secret IN (0, 1)),
  secret_ref    TEXT,                      -- is_secret=1 时使用 keychain 引用
  UNIQUE (deployment_id, key)
);
```

规则：

- `is_secret=0`：`value` 存明文（如 `TZ`、`MYSQL_DATABASE`）；
- `is_secret=1`：`value` 必须为 `NULL`，值在 OS keychain，`secret_ref` 指向；
- 不再有 legacy 配置导入/导出；密钥只经系统凭据管理器读写，不写文件、不回写 DB。

当前实现（`src-tauri/src/secrets.rs`、`env_vars.rs`）：

- 变量名含 `PASSWORD` / `SECRET` / `TOKEN` / `PRIVATE_KEY` / `APP_KEY` 时自动判为密文；
- keychain service 固定为 `PhAdmin Desktop`，account 为 `"{deployment_id}/{key}"`，`secret_ref` 存的就是这个 account；
- 写入顺序：先写 keychain，成功后才写 `env_vars`（`value = NULL`）；keychain 不可用时导入只记 warning，不落明文；
- 读取：渲染 `.env`、数据库备份/恢复取密码都走 keychain，读不到按空字符串处理；
- 遗留行（`is_secret=1` 且 `value`、`secret_ref` 都为 `NULL`）视为“未设置”，需要在「配置 → 环境变量」重填；
- 删除变量或改成明文时同步删除对应 keychain 凭据。

`MYSQL_PASSWORD`、`REDIS_PASSWORD` 是部署必需项；缺失时 compose 模板会渲染出空口令。

### 3.7 `deployment_settings`

```sql
CREATE TABLE deployment_settings (
  deployment_id     TEXT PRIMARY KEY REFERENCES deployments(id) ON DELETE CASCADE,
  config_json       TEXT NOT NULL DEFAULT '{}',  -- app.config 覆盖（JSON，兼容旧数据）
  config_php        TEXT NOT NULL DEFAULT '',    -- 项目创建时填写的 PHP 配置数组
  hooks_json        TEXT NOT NULL DEFAULT '{}',  -- afterInit / afterUpgrade
  docker_images_json TEXT NOT NULL DEFAULT '{}',
  router_json       TEXT NOT NULL DEFAULT '{}',
  nginx_json        TEXT NOT NULL DEFAULT '{}'
);
```

`config_php` 非空时优先作为 `{{CONFIG_OVERRIDES}}` 注入 `src/config/config.php`；
为空时回退到 `config_json` 转 PHP 数组。

用 JSON 存低频、结构不固定的配置；高频查询字段才单独建列。

### 3.8 `runs`

```sql
CREATE TABLE runs (
  id            TEXT PRIMARY KEY,
  deployment_id TEXT NOT NULL REFERENCES deployments(id) ON DELETE CASCADE,
  command       TEXT NOT NULL,
  args_json     TEXT NOT NULL DEFAULT '[]',
  status        TEXT NOT NULL CHECK (status IN ('queued', 'running', 'succeeded', 'failed', 'canceled')),
  exit_code     INTEGER,
  started_at    TEXT,
  ended_at      TEXT,
  log_path      TEXT,
  error_summary TEXT
);
```

- 日志正文写 `log_path` 对应文件，不存 BLOB；
- `runs` 不更新历史记录（除状态流转），作为审计；
- 可按 `deployment_id, started_at DESC` 建索引。

```sql
CREATE INDEX idx_runs_deployment_started
  ON runs (deployment_id, started_at DESC);
```

### 3.9 `settings`

```sql
CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
```

存 UI 偏好、最近项目、legacy 导入指纹等。窗口位置/主题也可放 Tauri Store，避免和业务数据混在一起。

### 3.10 `schema_migrations`

使用 sqlx migrations 时由 sqlx 管理，不手写业务表。若自行管理：

```sql
CREATE TABLE schema_migrations (
  version    INTEGER PRIMARY KEY,
  applied_at TEXT NOT NULL
);
```

---

## 4. SQLite 运行参数

每个连接初始化：

```sql
PRAGMA journal_mode = DELETE;   -- 单文件；写入期间短暂出现 -journal
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;
```

说明：

- 桌面端是单进程访问，不需要 WAL 的并发读能力，因此使用 DELETE 日志模式，
  数据库目录通常只有一个 `phadmin.db`；
- 写入事务进行中会短暂出现 `phadmin.db-journal`，提交或回滚后自动删除；
- 如果从旧版本（WAL 模式）升级，下一次启动会把 `-wal` / `-shm` 合并回主库并删除；
- 设备异常断电时，SQLite 会依赖 journal 自动回滚未完成事务。

建议：

- 单进程写者：Rust 侧一个 `Mutex<Connection>`；
- 所有多表写入放在事务中；
- 不在前端开 SQL 连接，不把 SQL 暴露给 WebView；
- 每次 migration 前执行 `VACUUM INTO '<backup>'`。

---

## 5. 本地目录

`<app_data_dir>` 由 Tauri 解析（随安装用户变化）：

```text
<app_data_dir>/
  phadmin.db            # 单文件；写入时短暂出现 phadmin.db-journal
  backups/
    phadmin-<timestamp>.db
  generated/
    <deployment_id>/
      .env
      nginx.conf
      docker-compose.yaml
      config.php
  logs/
    <run_id>.log
  tmp/
    bundle-<millis>.bundle
  exports/
    <deployment_id>-<timestamp>/
      server.php
      project-server.php
```

原则：

- 不默认写入旧仓库的 `deploy/projects/`；
- 只有“导出 legacy 回退配置”时才写 `exports/`；
- `generated/` 可按需重建，不是事实源；远程覆盖前写 `<file>.bak-<millis>`；
- 日志/临时 bundle 可清理，SQLite 只保留元数据。

---

## 6. 从 PHP deploy 导入

### 6.1 来源

优先使用：

```bash
php admin config:export
```

输出 `deploy/.cache/config.json`，结构为：

```json
{
  "项目名": {
    "ssh": {},
    "project": {},
    "domains": [],
    "env": {},
    "config": {},
    "hooks": {},
    "docker": {},
    "router": {},
    "sync": {},
    "nginx": {}
  }
}
```

### 6.2 映射

| PHP 配置 | SQLite |
|---|---|
| `ssh.host/port/user` | `servers`（按 host:port:user 去重） |
| 项目名 | `projects.name` |
| `project.path`、`nginxPort` | `deployments.remote_path`、`nginx_port` |
| `domains` | `domains` |
| `env` | `env_vars`（密码识别为 secret） |
| `sync.items` | `sync_items` |
| `config` / `hooks` / `docker.images` | `deployment_settings` |
| `nginx.ssl` | `deployments.ssl` |

### 6.3 幂等与漂移

> `deploy/projects/vbox`、`vbox2` 是历史 vbox 测试配置，不自动导入为生产 deployment；如需保留，手工核对后新建。

- 导入前记录来源文件路径、mtime、sha256；
- 重复导入按 `(server_id, project_id)` upsert；
- 如果 PHP 文件在导入后被修改，UI 提示“检测到 legacy 配置变化”，让用户选择重新导入，**不要自动双向同步**；
- 首次导入生成迁移报告：新增/更新/跳过/冲突数量。

---

## 7. 导出 legacy 回退配置

仅在需要 `php admin` 应急时执行：

1. 选择单个 `deployment`；
2. Rust 读取 SQLite + keychain；
3. 渲染 `server.php` 与项目 `server.php`；
4. 写入 `<app_data_dir>/exports/<deployment_id>-<timestamp>/`；
5. 在 UI 显示路径和“复制回退命令”。

注意：

- 现有 PHP CLI 是单服务器、单项目目录模型；
- 一次只导出一个 deployment；
- 导出文件含敏感信息，必须限制权限并在使用后清理；
- 不把导出文件写回 SQLite。

---

## 8. 备份与恢复

- 应用启动时检查 `schema_migrations`；
- migration 前 `VACUUM INTO backups/phadmin-<ts>.db`；
- migration 失败：关闭连接，恢复备份，报告错误；
- 提供“导出数据库”/“导入数据库”给用户迁移机器；
- 不把 `phadmin.db` 放进 git。

---

## 9. 索引建议

```sql
CREATE INDEX idx_deployments_server ON deployments(server_id);
CREATE INDEX idx_deployments_project ON deployments(project_id);
CREATE INDEX idx_sync_items_deployment ON sync_items(deployment_id, sort);
CREATE INDEX idx_domains_deployment ON domains(deployment_id, sort);
CREATE INDEX idx_env_vars_deployment ON env_vars(deployment_id);
CREATE INDEX idx_runs_deployment_started ON runs(deployment_id, started_at DESC);
```

---

## 10. 待定问题

1. ID 使用 UUID v7 还是自增整数？（建议 UUID，便于导入/导出）
2. 部署配置用 JSON 还是完全规范化？（当前折中）
3. 是否需要对 SQLite 做 SQLCipher 加密？（先 keychain + 文件权限）
4. 是否需要多窗口同时写？若需要，改为单实例 + 后端串行写。
## 全局设置

`settings(key, value)` 保存应用级配置；目前使用 `local_repo_path`（本地仓库路径）。bundle / ftp 同步在 `projects.local_path` 为空时回退到该路径。
## 服务器名称

- `servers.name` 由用户填写（最长 64 字符，唯一），不再自动生成 `user@host:port`；`servers.id` 仍由名称生成稳定 slug；
- 添加/修改服务器面板第一个字段为「名称」；
- 项目创建/部署面板中的服务器选择只显示名称；服务器菜单显示「名称 + 用户 · SSH」，概览不显示 IP；IP 仅在服务器编辑表单中维护。
