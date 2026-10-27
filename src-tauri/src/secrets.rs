use std::sync::Arc;

#[cfg(any(
    test,
    not(any(target_os = "windows", target_os = "macos", target_os = "linux"))
))]
use std::collections::HashMap;
#[cfg(any(
    test,
    not(any(target_os = "windows", target_os = "macos", target_os = "linux"))
))]
use std::sync::Mutex;

/// 系统凭据管理器里的分组名，决定凭据在 Credential Manager / Keychain 中的归属。
pub const SERVICE: &str = "PhAdmin Desktop";

pub trait SecretStore: Send + Sync {
    fn set(&self, account: &str, value: &str) -> Result<(), String>;
    fn get(&self, account: &str) -> Result<Option<String>, String>;
    fn delete(&self, account: &str) -> Result<(), String>;
}

/// 一个 deployment 的每个 key 单独一条凭据，避免多套环境互相覆盖。
pub fn account(deployment_id: &str, key: &str) -> String {
    format!("{deployment_id}/{key}")
}

pub fn system_store() -> Arc<dyn SecretStore> {
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    {
        Arc::new(KeychainStore)
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        // 其他平台没有凭据管理实现；退回内存存储，至少让 UI 明确“重启后需要重填”。
        Arc::new(MemoryStore::default())
    }
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
pub struct KeychainStore;

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
impl SecretStore for KeychainStore {
    fn set(&self, account: &str, value: &str) -> Result<(), String> {
        let entry = keyring::Entry::new(SERVICE, account)
            .map_err(|error| format!("打开系统凭据失败: {error}"))?;
        entry
            .set_password(value)
            .map_err(|error| format!("写入系统凭据失败: {error}"))
    }

    fn get(&self, account: &str) -> Result<Option<String>, String> {
        let entry = keyring::Entry::new(SERVICE, account)
            .map_err(|error| format!("打开系统凭据失败: {error}"))?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(format!("读取系统凭据失败: {error}")),
        }
    }

    fn delete(&self, account: &str) -> Result<(), String> {
        let entry = keyring::Entry::new(SERVICE, account)
            .map_err(|error| format!("打开系统凭据失败: {error}"))?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(format!("删除系统凭据失败: {error}")),
        }
    }
}

/// 测试与不支持凭据管理的平台使用；进程退出后内容即丢失。
#[cfg(any(
    test,
    not(any(target_os = "windows", target_os = "macos", target_os = "linux"))
))]
#[derive(Default)]
pub struct MemoryStore {
    entries: Mutex<HashMap<String, String>>,
}

#[cfg(any(
    test,
    not(any(target_os = "windows", target_os = "macos", target_os = "linux"))
))]
impl MemoryStore {
    pub fn contains(&self, account: &str) -> bool {
        self.entries
            .lock()
            .map(|entries| entries.contains_key(account))
            .unwrap_or(false)
    }
}

#[cfg(any(
    test,
    not(any(target_os = "windows", target_os = "macos", target_os = "linux"))
))]
impl SecretStore for MemoryStore {
    fn set(&self, account: &str, value: &str) -> Result<(), String> {
        self.entries
            .lock()
            .map_err(|error| format!("内存凭据锁被占用: {error}"))?
            .insert(account.to_string(), value.to_string());
        Ok(())
    }

    fn get(&self, account: &str) -> Result<Option<String>, String> {
        Ok(self
            .entries
            .lock()
            .map_err(|error| format!("内存凭据锁被占用: {error}"))?
            .get(account)
            .cloned())
    }

    fn delete(&self, account: &str) -> Result<(), String> {
        self.entries
            .lock()
            .map_err(|error| format!("内存凭据锁被占用: {error}"))?
            .remove(account);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_is_scoped_by_deployment() {
        assert_eq!(
            account("deployment-a", "MYSQL_PASSWORD"),
            "deployment-a/MYSQL_PASSWORD"
        );
        assert_ne!(
            account("deployment-a", "MYSQL_PASSWORD"),
            account("deployment-b", "MYSQL_PASSWORD")
        );
    }

    #[test]
    fn memory_store_round_trips_and_deletes() {
        let store = MemoryStore::default();
        assert_eq!(store.get("a").expect("get"), None);
        store.set("a", "secret").expect("set");
        assert_eq!(store.get("a").expect("get"), Some("secret".to_string()));
        assert!(store.contains("a"));
        store.delete("a").expect("delete");
        assert_eq!(store.get("a").expect("get"), None);
        store.delete("a").expect("delete missing");
    }

    /// 真机验证 Windows Credential Manager / macOS Keychain / Secret Service 可用性。
    /// 默认忽略：CI 无桌面会话时没有凭据存储。
    #[test]
    #[ignore = "需要真实系统凭据存储，桌面环境手动运行 cargo test -- --ignored"]
    fn keychain_round_trips_on_this_machine() {
        let store = KeychainStore;
        let account = "phadmin-desktop-selftest/MYSQL_PASSWORD";
        store.set(account, "round-trip").expect("set");
        assert_eq!(
            store.get(account).expect("get"),
            Some("round-trip".to_string())
        );
        store.delete(account).expect("delete");
        assert_eq!(store.get(account).expect("get"), None);
    }
}
