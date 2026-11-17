use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticCliInfo {
    pub command: String,
    pub available: bool,
    pub installed: bool,
}

fn binary_name() -> &'static str {
    if cfg!(windows) {
        "phadmin-cli.exe"
    } else {
        "phadmin-cli"
    }
}

fn installed_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|error| format!("获取数据目录失败: {error}"))?
        .join("bin")
        .join(binary_name()))
}

fn sibling_path() -> Result<PathBuf, String> {
    let current =
        std::env::current_exe().map_err(|error| format!("定位应用可执行文件失败: {error}"))?;
    let parent = current
        .parent()
        .ok_or_else(|| "无法确定应用安装目录".to_string())?;
    Ok(parent.join(binary_name()))
}

fn describe(path: Option<&Path>) -> DiagnosticCliInfo {
    match path {
        Some(path) if path.is_file() => DiagnosticCliInfo {
            command: format!("\"{}\"", path.display()),
            available: true,
            installed: true,
        },
        _ => DiagnosticCliInfo {
            command: "phadmin-cli".to_string(),
            available: false,
            installed: false,
        },
    }
}

#[tauri::command]
pub fn diagnostic_cli_info(app: AppHandle) -> Result<DiagnosticCliInfo, String> {
    let installed = installed_path(&app)?;
    if installed.is_file() {
        return Ok(describe(Some(&installed)));
    }
    let sibling = sibling_path()?;
    if sibling.is_file() {
        let mut info = describe(Some(&sibling));
        info.installed = false;
        return Ok(info);
    }
    Ok(describe(None))
}

#[tauri::command]
pub fn install_diagnostic_cli(app: AppHandle) -> Result<DiagnosticCliInfo, String> {
    let destination = installed_path(&app)?;
    if destination.is_file() {
        return Ok(describe(Some(&destination)));
    }
    let source = sibling_path()?;
    if !source.is_file() {
        return Err(format!(
            "未找到 {}，请先用 cargo build --bin phadmin-cli 构建命令行工具",
            source.display()
        ));
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建 CLI 目录失败: {error}"))?;
    }
    fs::copy(&source, &destination).map_err(|error| {
        format!(
            "复制 {} 到 {} 失败: {error}",
            source.display(),
            destination.display()
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&destination, fs::Permissions::from_mode(0o755));
    }
    Ok(describe(Some(&destination)))
}

#[tauri::command]
pub fn write_export_file(path: String, content: String) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("导出路径为空".to_string());
    }
    if content.len() > 10 * 1024 * 1024 {
        return Err("导出内容超过 10 MB 限制".to_string());
    }
    fs::write(&path, content).map_err(|error| format!("写入 {path} 失败: {error}"))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticCliStatus {
    pub command: String,
    pub installed: bool,
    pub on_path: bool,
    pub bin_dir: String,
}

fn cli_status(app: &AppHandle) -> Result<DiagnosticCliStatus, String> {
    let installed = installed_path(app)?;
    let installed_exists = installed.is_file();
    let sibling = sibling_path()?;
    let command = if installed_exists {
        format!("\"{}\"", installed.display())
    } else if sibling.is_file() {
        format!("\"{}\"", sibling.display())
    } else {
        "phadmin-cli".to_string()
    };
    let bin_dir = installed
        .parent()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_default();
    let on_path = user_path_contains(Path::new(&bin_dir))?;
    Ok(DiagnosticCliStatus {
        command,
        installed: installed_exists,
        on_path,
        bin_dir,
    })
}

#[tauri::command]
pub fn diagnostic_cli_status(app: AppHandle) -> Result<DiagnosticCliStatus, String> {
    cli_status(&app)
}

#[tauri::command]
pub fn add_diagnostic_cli_to_path(app: AppHandle) -> Result<DiagnosticCliStatus, String> {
    install_diagnostic_cli(app.clone())?;
    let installed = installed_path(&app)?;
    let bin_dir = installed
        .parent()
        .ok_or_else(|| "无法确定 CLI 目录".to_string())?;
    add_user_path(bin_dir)?;
    cli_status(&app)
}

#[cfg(target_os = "windows")]
fn user_path_contains(dir: &Path) -> Result<bool, String> {
    let (current, _) = read_user_path()?;
    Ok(path_contains(&current, &dir.to_string_lossy()))
}

#[cfg(not(target_os = "windows"))]
fn user_path_contains(dir: &Path) -> Result<bool, String> {
    let current = std::env::var("PATH").unwrap_or_default();
    Ok(path_contains(&current, &dir.to_string_lossy()))
}

#[cfg(target_os = "windows")]
fn path_contains(current: &str, dir: &str) -> bool {
    current
        .split(';')
        .any(|entry| entry.trim().trim_matches('"').eq_ignore_ascii_case(dir))
}

#[cfg(not(target_os = "windows"))]
fn path_contains(current: &str, dir: &str) -> bool {
    current.split(':').any(|entry| entry == dir)
}

#[cfg(target_os = "windows")]
fn read_user_path() -> Result<(String, winreg::enums::RegType), String> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, REG_EXPAND_SZ};

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let environment = hkcu
        .open_subkey_with_flags("Environment", KEY_READ)
        .map_err(|error| format!("读取用户环境变量失败: {error}"))?;
    match environment.get_raw_value("Path") {
        Ok(raw) => {
            let mut units = Vec::with_capacity(raw.bytes.len() / 2);
            for chunk in raw.bytes.chunks_exact(2) {
                units.push(u16::from_le_bytes([chunk[0], chunk[1]]));
            }
            let value = String::from_utf16_lossy(&units)
                .trim_end_matches('\0')
                .to_string();
            Ok((value, raw.vtype))
        }
        Err(_) => Ok((String::new(), REG_EXPAND_SZ)),
    }
}

#[cfg(target_os = "windows")]
fn write_user_path(value: &str, vtype: winreg::enums::RegType) -> Result<(), String> {
    use winreg::RegKey;
    use winreg::RegValue;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_WRITE};

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let environment = hkcu
        .open_subkey_with_flags("Environment", KEY_WRITE)
        .map_err(|error| format!("打开用户环境变量失败: {error}"))?;
    let mut bytes = Vec::new();
    for unit in value.encode_utf16().chain(std::iter::once(0)) {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    environment
        .set_raw_value("Path", &RegValue { bytes, vtype })
        .map_err(|error| format!("写入用户 PATH 失败: {error}"))
}

#[cfg(target_os = "windows")]
fn broadcast_environment_change() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
    };
    let mut environment: Vec<u16> = "Environment\0".encode_utf16().collect();
    let mut result = 0usize;
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            environment.as_mut_ptr() as isize,
            SMTO_ABORTIFHUNG,
            5000,
            &mut result,
        );
    }
}

#[cfg(target_os = "windows")]
fn add_user_path(dir: &Path) -> Result<(), String> {
    let (current, vtype) = read_user_path()?;
    let dir = dir.to_string_lossy();
    if path_contains(&current, &dir) {
        return Ok(());
    }
    let value = if current.trim().is_empty() {
        dir.to_string()
    } else {
        format!("{};{}", current.trim_end_matches(';'), dir)
    };
    write_user_path(&value, vtype)?;
    broadcast_environment_change();
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn add_user_path(_dir: &Path) -> Result<(), String> {
    Err("当前版本仅支持在 Windows 上自动添加 PATH".to_string())
}
