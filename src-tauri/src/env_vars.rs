use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use tauri::State;

use crate::AppState;
use crate::secrets::{self, SecretStore};

/// 部署缺这些密码就起不来（compose 模板把它们作为 MySQL root / Redis 口令）。
pub const REQUIRED_SECRETS: &[&str] = &["MYSQL_PASSWORD", "REDIS_PASSWORD"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvVarInfo {
    pub key: String,
    pub is_secret: bool,
    pub has_value: bool,
    /// 密文变量的值永远不回传给前端，只回传是否已设置。
    pub value: Option<String>,
    pub required: bool,
}

pub fn is_secret_key(key: &str) -> bool {
    let upper = key.to_ascii_uppercase();
    ["PASSWORD", "SECRET", "TOKEN", "PRIVATE_KEY", "APP_KEY"]
        .iter()
        .any(|needle| upper.contains(needle))
}

fn normalize_key(raw: &str) -> Result<String, String> {
    let key = raw.trim().to_ascii_uppercase();
    if key.is_empty() {
        return Err("变量名不能为空".to_string());
    }
    if key.len() > 64 {
        return Err(format!("变量名过长：{key}"));
    }
    let mut chars = key.chars();
    let valid = matches!(chars.next(), Some(first) if first.is_ascii_alphabetic() || first == '_')
        && chars.all(|character| character.is_ascii_alphanumeric() || character == '_');
    if !valid {
        return Err(format!("变量名只能用字母、数字、下划线：{key}"));
    }
    Ok(key)
}

fn validate_value(value: &str) -> Result<(), String> {
    if value.contains('\n') || value.contains('\r') {
        return Err("变量值不能包含换行".to_string());
    }
    if value.len() > 1024 {
        return Err("变量值过长（最多 1024 字符）".to_string());
    }
    Ok(())
}

fn ensure_deployment(connection: &Connection, deployment_id: &str) -> Result<(), String> {
    let exists: Option<i64> = connection
        .query_row(
            "SELECT 1 FROM deployments WHERE id = ?1",
            [deployment_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("读取部署失败: {error}"))?;
    if exists.is_none() {
        return Err(format!("部署不存在: {deployment_id}"));
    }
    Ok(())
}

fn secret_ref(
    connection: &Connection,
    deployment_id: &str,
    key: &str,
) -> Result<Option<String>, String> {
    connection
        .query_row(
            "SELECT secret_ref FROM env_vars WHERE deployment_id = ?1 AND key = ?2",
            params![deployment_id, key],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map(|row| row.flatten())
        .map_err(|error| format!("读取环境变量失败: {error}"))
}

/// 密文取系统密钥环，明文取 SQLite；两者都没配置时返回 None。
pub fn resolve(
    connection: &Connection,
    store: &dyn SecretStore,
    deployment_id: &str,
    key: &str,
) -> Result<Option<String>, String> {
    let row: Option<(Option<String>, i64, Option<String>)> = connection
        .query_row(
            "SELECT value, is_secret, secret_ref FROM env_vars
             WHERE deployment_id = ?1 AND key = ?2",
            params![deployment_id, key],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .optional()
        .map_err(|error| format!("读取环境变量失败: {error}"))?;

    match row {
        None => Ok(None),
        Some((value, is_secret, reference)) => {
            if is_secret == 1 {
                let account = reference.unwrap_or_else(|| secrets::account(deployment_id, key));
                store.get(&account)
            } else {
                Ok(value)
            }
        }
    }
}

pub fn list(
    connection: &Connection,
    store: &dyn SecretStore,
    deployment_id: &str,
) -> Result<Vec<EnvVarInfo>, String> {
    let mut statement = connection
        .prepare(
            "SELECT key, value, is_secret, secret_ref FROM env_vars
             WHERE deployment_id = ?1 ORDER BY key",
        )
        .map_err(|error| format!("读取环境变量失败: {error}"))?;
    let rows = statement
        .query_map([deployment_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(|error| format!("读取环境变量失败: {error}"))?;

    let mut items = Vec::new();
    for row in rows {
        let (key, value, is_secret, reference) =
            row.map_err(|error| format!("读取环境变量失败: {error}"))?;
        let (has_value, visible) = if is_secret == 1 {
            let account = reference.unwrap_or_else(|| secrets::account(deployment_id, &key));
            (store.get(&account)?.is_some(), None)
        } else {
            (value.is_some(), value)
        };
        items.push(EnvVarInfo {
            required: REQUIRED_SECRETS.contains(&key.as_str()),
            key,
            is_secret: is_secret == 1,
            has_value,
            value: visible,
        });
    }
    Ok(items)
}

pub fn set(
    connection: &Connection,
    store: &dyn SecretStore,
    deployment_id: &str,
    raw_key: &str,
    value: &str,
) -> Result<EnvVarInfo, String> {
    let key = normalize_key(raw_key)?;
    validate_value(value)?;
    ensure_deployment(connection, deployment_id)?;

    let is_secret = is_secret_key(&key);
    if is_secret && value.is_empty() {
        return Err(format!("{key} 不能为空；如需删除请用移除"));
    }

    let previous_ref = secret_ref(connection, deployment_id, &key)?;
    let account = secrets::account(deployment_id, &key);

    if is_secret {
        store.set(&account, value)?;
        connection
            .execute(
                "INSERT INTO env_vars (id, deployment_id, key, value, is_secret, secret_ref)
                 VALUES (?1, ?2, ?3, NULL, 1, ?4)
                 ON CONFLICT(deployment_id, key) DO UPDATE SET
                    value = NULL,
                    is_secret = 1,
                    secret_ref = excluded.secret_ref",
                params![
                    format!("{deployment_id}-env-{key}"),
                    deployment_id,
                    &key,
                    &account
                ],
            )
            .map_err(|error| format!("保存环境变量失败: {error}"))?;
    } else {
        if let Some(reference) = previous_ref {
            store.delete(&reference)?;
        }
        connection
            .execute(
                "INSERT INTO env_vars (id, deployment_id, key, value, is_secret, secret_ref)
                 VALUES (?1, ?2, ?3, ?4, 0, NULL)
                 ON CONFLICT(deployment_id, key) DO UPDATE SET
                    value = excluded.value,
                    is_secret = 0,
                    secret_ref = NULL",
                params![
                    format!("{deployment_id}-env-{key}"),
                    deployment_id,
                    &key,
                    value
                ],
            )
            .map_err(|error| format!("保存环境变量失败: {error}"))?;
    }

    Ok(EnvVarInfo {
        required: REQUIRED_SECRETS.contains(&key.as_str()),
        has_value: true,
        value: if is_secret {
            None
        } else {
            Some(value.to_string())
        },
        key,
        is_secret,
    })
}

pub fn remove(
    connection: &Connection,
    store: &dyn SecretStore,
    deployment_id: &str,
    raw_key: &str,
) -> Result<(), String> {
    let key = normalize_key(raw_key)?;
    let reference = secret_ref(connection, deployment_id, &key)?;
    connection
        .execute(
            "DELETE FROM env_vars WHERE deployment_id = ?1 AND key = ?2",
            params![deployment_id, &key],
        )
        .map_err(|error| format!("删除环境变量失败: {error}"))?;
    if let Some(reference) = reference {
        store.delete(&reference)?;
    }
    Ok(())
}

#[tauri::command]
pub fn list_env_vars(
    state: State<'_, AppState>,
    deployment_id: String,
) -> Result<Vec<EnvVarInfo>, String> {
    let connection = state
        .db
        .lock()
        .map_err(|error| format!("数据库连接被占用: {error}"))?;
    list(&connection, state.secrets.as_ref(), &deployment_id)
}

#[tauri::command]
pub fn set_env_var(
    state: State<'_, AppState>,
    deployment_id: String,
    key: String,
    value: String,
) -> Result<EnvVarInfo, String> {
    let connection = state
        .db
        .lock()
        .map_err(|error| format!("数据库连接被占用: {error}"))?;
    set(
        &connection,
        state.secrets.as_ref(),
        &deployment_id,
        &key,
        &value,
    )
}

#[tauri::command]
pub fn delete_env_var(
    state: State<'_, AppState>,
    deployment_id: String,
    key: String,
) -> Result<(), String> {
    let connection = state
        .db
        .lock()
        .map_err(|error| format!("数据库连接被占用: {error}"))?;
    remove(&connection, state.secrets.as_ref(), &deployment_id, &key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::secrets::MemoryStore;

    fn seeded() -> (Connection, String) {
        let mut connection = Connection::open_in_memory().expect("open memory db");
        db::migrate(&connection).expect("migrate");
        db::seed_if_empty(&mut connection).expect("seed");
        let deployment_id: String = connection
            .query_row(
                "SELECT id FROM deployments ORDER BY id LIMIT 1",
                [],
                |row| row.get(0),
            )
            .expect("deployment");
        (connection, deployment_id)
    }

    #[test]
    fn secret_is_stored_in_keychain_not_sqlite() {
        let (connection, deployment_id) = seeded();
        let store = MemoryStore::default();

        let info = set(
            &connection,
            &store,
            &deployment_id,
            "mysql_password",
            "s3cret",
        )
        .expect("set secret");
        assert!(info.is_secret);
        assert!(info.has_value);
        assert!(info.required);
        assert_eq!(info.value, None);

        let stored: (Option<String>, i64, Option<String>) = connection
            .query_row(
                "SELECT value, is_secret, secret_ref FROM env_vars
                 WHERE deployment_id = ?1 AND key = 'MYSQL_PASSWORD'",
                [&deployment_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("env row");
        assert_eq!(stored.0, None, "密文不能落 SQLite");
        assert_eq!(stored.1, 1);
        assert_eq!(
            stored.2,
            Some(secrets::account(&deployment_id, "MYSQL_PASSWORD"))
        );
        assert!(store.contains(&secrets::account(&deployment_id, "MYSQL_PASSWORD")));

        assert_eq!(
            resolve(&connection, &store, &deployment_id, "MYSQL_PASSWORD").expect("resolve"),
            Some("s3cret".to_string())
        );

        let listed = list(&connection, &store, &deployment_id).expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].key, "MYSQL_PASSWORD");
        assert!(listed[0].has_value);
        assert_eq!(listed[0].value, None);
    }

    #[test]
    fn plain_value_round_trips_through_sqlite() {
        let (connection, deployment_id) = seeded();
        let store = MemoryStore::default();

        let info = set(&connection, &store, &deployment_id, "TZ", "Asia/Shanghai").expect("set");
        assert!(!info.is_secret);
        assert_eq!(info.value.as_deref(), Some("Asia/Shanghai"));
        assert_eq!(
            resolve(&connection, &store, &deployment_id, "TZ").expect("resolve"),
            Some("Asia/Shanghai".to_string())
        );
        assert!(
            store
                .get(&secrets::account(&deployment_id, "TZ"))
                .expect("get")
                .is_none()
        );
    }

    #[test]
    fn rejects_invalid_keys_and_values() {
        let (connection, deployment_id) = seeded();
        let store = MemoryStore::default();

        assert!(set(&connection, &store, &deployment_id, "", "x").is_err());
        assert!(set(&connection, &store, &deployment_id, "MYSQL-PASSWORD", "x").is_err());
        assert!(set(&connection, &store, &deployment_id, "1BAD", "x").is_err());
        assert!(set(&connection, &store, &deployment_id, "TZ", "a\nb").is_err());
        assert!(set(&connection, &store, &deployment_id, "MYSQL_PASSWORD", "").is_err());
    }

    #[test]
    fn remove_deletes_row_and_keychain_entry() {
        let (connection, deployment_id) = seeded();
        let store = MemoryStore::default();
        set(
            &connection,
            &store,
            &deployment_id,
            "REDIS_PASSWORD",
            "redis",
        )
        .expect("set");

        remove(&connection, &store, &deployment_id, "REDIS_PASSWORD").expect("remove");

        assert!(
            resolve(&connection, &store, &deployment_id, "REDIS_PASSWORD")
                .expect("resolve")
                .is_none()
        );
        assert!(!store.contains(&secrets::account(&deployment_id, "REDIS_PASSWORD")));
    }

    #[test]
    fn resolve_unknown_key_is_none() {
        let (connection, deployment_id) = seeded();
        let store = MemoryStore::default();
        assert!(
            resolve(&connection, &store, &deployment_id, "MYSQL_PASSWORD")
                .expect("resolve")
                .is_none()
        );
    }

    /// 真机链路：SQLite 只写引用，值进系统凭据，再读回并清理。
    #[test]
    #[ignore = "需要真实系统凭据存储，桌面环境手动运行 cargo test secret_round_trips -- --ignored"]
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    fn secret_round_trips_through_real_keychain() {
        let (connection, deployment_id) = seeded();
        let store = crate::secrets::KeychainStore;
        let account = secrets::account(&deployment_id, "MYSQL_PASSWORD");

        set(
            &connection,
            &store,
            &deployment_id,
            "MYSQL_PASSWORD",
            "selftest",
        )
        .expect("set");
        assert_eq!(
            resolve(&connection, &store, &deployment_id, "MYSQL_PASSWORD").expect("resolve"),
            Some("selftest".to_string())
        );
        let stored: Option<String> = connection
            .query_row(
                "SELECT value FROM env_vars
                 WHERE deployment_id = ?1 AND key = 'MYSQL_PASSWORD'",
                [&deployment_id],
                |row| row.get(0),
            )
            .expect("value column");
        assert_eq!(stored, None, "密文不能落 SQLite");

        remove(&connection, &store, &deployment_id, "MYSQL_PASSWORD").expect("remove");
        assert_eq!(store.get(&account).expect("get"), None);
    }
}
