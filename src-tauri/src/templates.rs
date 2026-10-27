use std::collections::HashMap;

use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

use crate::AppState;

/// 内置模板：路径 → 编译进应用的默认内容。
/// 数据库表 `templates` 只存用户覆盖（override）或自定义（custom）模板。
pub(crate) const BUILTINS: &[(&str, &str)] = &[
    (".env", include_str!("../templates/env.example")),
    (
        "docker-compose.yaml",
        include_str!("../templates/docker-compose.yaml"),
    ),
    (
        "src/config/config.php",
        include_str!("../templates/config.php.template"),
    ),
    (
        "docker/mysql/my.cnf",
        include_str!("../templates/mysql/my.cnf"),
    ),
    (
        "docker/nginx/sites/default.conf",
        include_str!("../templates/nginx/default.conf"),
    ),
    (
        "docker/php/php.ini",
        include_str!("../templates/php/php.ini"),
    ),
];

/// 渲染时可用的变量；用于保存提示和预览校验。
pub(crate) const KNOWN_VARS: &[&str] = &[
    "APP_NAME",
    "PROJECT_NAME",
    "PROJECT_PATH",
    "REMOTE_PATH",
    "DATA_PATH_HOST",
    "TZ",
    "NGINX_PORT",
    "DOCKER_NETWORK",
    "MYSQL_USER",
    "MYSQL_DATABASE",
    "MYSQL_PASSWORD",
    "REDIS_PASSWORD",
    "NGINX_IMAGE",
    "PHP_IMAGE",
    "MYSQL_IMAGE",
    "REDIS_IMAGE",
    "CONFIG_OVERRIDES",
];

const MAX_TEMPLATE_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateInfo {
    pub path: String,
    /// builtin：未覆盖；override：覆盖内置；custom：新增的自定义模板。
    pub kind: String,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateDetail {
    pub path: String,
    pub content: String,
    pub kind: String,
    pub builtin: bool,
    pub updated_at: Option<String>,
}

pub(crate) fn builtin_content(path: &str) -> Option<&'static str> {
    BUILTINS
        .iter()
        .find(|(name, _)| *name == path)
        .map(|(_, content)| *content)
}

pub(crate) fn normalize_path(path: &str) -> Result<String, String> {
    let path = path.trim().replace('\\', "/");
    if path.is_empty() {
        return Err("模板路径不能为空".to_string());
    }
    if path.len() > 200 {
        return Err("模板路径过长".to_string());
    }
    if path.starts_with('/') || path.contains(':') {
        return Err("模板路径必须是项目内相对路径".to_string());
    }
    if path.ends_with('/') || path.contains("//") {
        return Err("模板路径格式不正确".to_string());
    }
    if path
        .split('/')
        .any(|segment| segment == ".." || segment == ".")
    {
        return Err("模板路径不能包含 . 或 ..".to_string());
    }
    if !path
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || "._/-".contains(character))
    {
        return Err("模板路径只能包含字母、数字、点、下划线、短横线、斜杠".to_string());
    }
    Ok(path)
}

pub(crate) fn resolve(connection: &Connection, path: &str) -> Result<String, String> {
    let stored = crate::db::get_template_row(connection, path)?;

    if let Some((content, _)) = stored {
        return Ok(content);
    }
    builtin_content(path)
        .map(str::to_string)
        .ok_or_else(|| format!("模板不存在: {path}"))
}

pub(crate) fn custom_paths(connection: &Connection) -> Result<Vec<String>, String> {
    let mut paths: Vec<String> = crate::db::list_template_rows(connection)?
        .into_iter()
        .map(|(path, _, _)| path)
        .filter(|path| builtin_content(path).is_none())
        .collect();
    paths.sort();
    Ok(paths)
}

pub(crate) fn render(content: &str, vars: &HashMap<&str, String>) -> (String, Vec<String>) {
    let mut rendered = content.to_string();
    let mut unresolved = Vec::new();

    for key in KNOWN_VARS {
        let placeholder = format!("{{{{{key}}}}}");
        if rendered.contains(&placeholder) {
            let value = vars.get(key).cloned().unwrap_or_default();
            rendered = rendered.replace(&placeholder, &value);
        }
    }

    for segment in rendered.split("{{").skip(1) {
        if let Some(end) = segment.find("}}") {
            let name = segment[..end].trim().to_string();
            if !name.is_empty() && !unresolved.contains(&name) {
                unresolved.push(name);
            }
        }
    }

    (rendered, unresolved)
}

pub(crate) fn empty_vars() -> HashMap<&'static str, String> {
    KNOWN_VARS.iter().map(|key| (*key, String::new())).collect()
}

#[tauri::command]
pub fn list_templates(state: State<'_, AppState>) -> Result<Vec<TemplateInfo>, String> {
    let connection = state
        .db
        .lock()
        .map_err(|error| format!("数据库连接被占用: {error}"))?;

    let mut rows = HashMap::new();
    for (path, kind, updated_at) in crate::db::list_template_rows(&connection)? {
        rows.insert(path, (kind, updated_at));
    }

    let mut templates: Vec<TemplateInfo> = BUILTINS
        .iter()
        .map(|(path, _)| match rows.get(*path) {
            Some((_, updated_at)) => TemplateInfo {
                path: (*path).to_string(),
                kind: "override".to_string(),
                updated_at: Some(updated_at.clone()),
            },
            None => TemplateInfo {
                path: (*path).to_string(),
                kind: "builtin".to_string(),
                updated_at: None,
            },
        })
        .collect();

    for (path, (kind, updated_at)) in rows {
        if builtin_content(&path).is_none() {
            templates.push(TemplateInfo {
                path,
                kind,
                updated_at: Some(updated_at),
            });
        }
    }
    templates.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(templates)
}

#[tauri::command]
pub fn get_template(state: State<'_, AppState>, path: String) -> Result<TemplateDetail, String> {
    let connection = state
        .db
        .lock()
        .map_err(|error| format!("数据库连接被占用: {error}"))?;
    load_detail(&connection, &path)
}

fn load_detail(connection: &Connection, path: &str) -> Result<TemplateDetail, String> {
    let stored = crate::db::get_template_row(connection, path)?;

    match stored {
        Some((content, updated_at)) => Ok(TemplateDetail {
            path: path.to_string(),
            content,
            kind: if builtin_content(path).is_some() {
                "override".to_string()
            } else {
                "custom".to_string()
            },
            builtin: builtin_content(path).is_some(),
            updated_at: Some(updated_at),
        }),
        None => {
            let content = builtin_content(path)
                .ok_or_else(|| format!("模板不存在: {path}"))?
                .to_string();
            Ok(TemplateDetail {
                path: path.to_string(),
                content,
                kind: "builtin".to_string(),
                builtin: true,
                updated_at: None,
            })
        }
    }
}

#[tauri::command]
pub fn save_template(
    state: State<'_, AppState>,
    path: String,
    content: String,
) -> Result<TemplateDetail, String> {
    let path = normalize_path(&path)?;
    if content.len() > MAX_TEMPLATE_BYTES {
        return Err("模板内容超过 512 KB 限制".to_string());
    }
    if content.trim().is_empty() {
        return Err("模板内容不能为空".to_string());
    }

    let connection = state
        .db
        .lock()
        .map_err(|error| format!("数据库连接被占用: {error}"))?;
    let kind = if builtin_content(&path).is_some() {
        "override"
    } else {
        "custom"
    };
    crate::db::upsert_template(&connection, &path, &content, kind)?;
    load_detail(&connection, &path)
}

#[tauri::command]
pub fn reset_template(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let path = normalize_path(&path)?;
    let connection = state
        .db
        .lock()
        .map_err(|error| format!("数据库连接被占用: {error}"))?;
    crate::db::delete_template(&connection, &path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connection() -> Connection {
        let connection = Connection::open_in_memory().expect("open memory db");
        crate::db::migrate(&connection).expect("migrate");
        connection
    }

    #[test]
    fn resolves_builtin_until_overridden() {
        let connection = connection();
        let builtin = resolve(&connection, ".env").expect("builtin");
        assert!(builtin.contains("APP_NAME={{APP_NAME}}"));

        let detail = save_template_row(&connection, ".env", "APP_NAME=custom\n");
        assert_eq!(detail.kind, "override");
        assert_eq!(
            resolve(&connection, ".env").expect("override"),
            "APP_NAME=custom\n"
        );

        crate::db::delete_template(&connection, ".env").expect("reset");
        assert_eq!(resolve(&connection, ".env").expect("builtin"), builtin);
    }

    fn save_template_row(connection: &Connection, path: &str, content: &str) -> TemplateDetail {
        crate::db::upsert_template(connection, path, content, "override").expect("insert");
        load_detail(connection, path).expect("detail")
    }

    #[test]
    fn renders_known_vars_and_reports_unknown() {
        let mut vars = empty_vars();
        vars.insert("APP_NAME", "demo".to_string());
        let (rendered, unresolved) = render("name={{APP_NAME}} typo={{NOPE}}", &vars);
        assert_eq!(rendered, "name=demo typo={{NOPE}}");
        assert_eq!(unresolved, vec!["NOPE".to_string()]);
    }

    #[test]
    fn rejects_unsafe_paths() {
        assert!(normalize_path("../etc/passwd").is_err());
        assert!(normalize_path("C:/Windows/win.ini").is_err());
        assert!(normalize_path("/etc/nginx/nginx.conf").is_err());
        assert!(normalize_path("docker/../../x").is_err());
        assert_eq!(
            normalize_path("docker\\php\\php.ini").expect("normalized"),
            "docker/php/php.ini"
        );
    }
}
