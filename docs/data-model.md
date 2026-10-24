# 数据模型

> 状态：设计基线
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

### 3.1 `servers`

```sql
CREATE TABLE servers (
  id            TEXT PRIMARY KEY,          -- UUID
  name          TEXT NOT NULL UNIQUE,      -- prod / staging / local
  kind          TEXT NOT NULL CHECK (kind IN ('ssh', 'filesystem')),
  host          TEXT,                      -- kind=ssh 必填
  port          INTEGER NOT NULL DEFAULT 22,
  username      TEXT,
  auth_type     TEXT CHECK (auth_type IN ('password', 'key', 'agent')),
  key_path      TEXT,
  password_ref  TEXT,                      -- keychain 引用，不存明文
  created_at    TEXT NOT NULL,
  updated_at    TEXT NOT NULL
);
```

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
- 导出 legacy 配置时，从 keychain 取值，只写入临时文件，不能写回 DB/仓库。

### 3.7 `deployment_settings`

```sql
CREATE TABLE deployment_settings (
  deployment_id     TEXT PRIMARY KEY REFERENCES deployments(id) ON DELETE CASCADE,
  config_json       TEXT NOT NULL DEFAULT '{}',  -- app.config 覆盖
  hooks_json        TEXT NOT NULL DEFAULT '{}',  -- afterInit / afterUpgrade
  docker_images_json TEXT NOT NULL DEFAULT '{}',
  router_json       TEXT NOT NULL DEFAULT '{}',
  nginx_json        TEXT NOT NULL DEFAULT '{}'
);
```

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
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;
PRAGMA synchronous = NORMAL;
```

建议：

- 单进程写者：Rust 侧一个 `Mutex<Connection>` 或 sqlx pool 的写连接；
- 读多写少：WAL 允许读并发；
- 所有多表写入放在事务中；
- 不在前端开 SQL 连接，不把 SQL 暴露给 WebView；
- 每次 migration 前执行 `VACUUM INTO '<backup>'`。

---

## 5. 本地目录

`<app_data_dir>` 由 Tauri 解析（随安装用户变化）：

```text
<app_data_dir>/
  phadmin.db
  phadmin.db-wal
  phadmin.db-shm
  backups/
    phadmin-<timestamp>.db
  deployments/
    <deployment_id>/
      generated/
        .env
        docker-compose.yaml
        docker/nginx/sites/default.conf
        docker/php/php.ini
        src/config/config.php
      backups/
        <db>-<timestamp>.sql.gz
      logs/
        <run_id>.log
  exports/
    <deployment_id>-<timestamp>/
      server.php
      project-server.php
```

原则：

- 不默认写入旧仓库的 `deploy/projects/`；
- 只有“导出 legacy 回退配置”时才写 `exports/`；
- `generated/` 可按需重建，不是事实源；
- 备份/日志可清理，SQLite 只保留元数据。

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
