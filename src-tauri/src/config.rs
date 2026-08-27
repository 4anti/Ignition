use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ItemKind {
    App,
    File,
    Folder,
    Url,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchItem {
    pub id: String,
    pub name: String,
    pub target: String,
    #[serde(default)]
    pub args: String,
    pub kind: ItemKind,
    pub enabled: bool,
    #[serde(default)]
    pub delay_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchConfig {
    #[serde(default)]
    pub login_delay_ms: u64,
    #[serde(default = "default_stagger")]
    pub stagger_ms: u64,
    #[serde(default)]
    pub items: Vec<LaunchItem>,
}

fn default_stagger() -> u64 {
    350
}

impl Default for LaunchConfig {
    fn default() -> Self {
        Self {
            login_delay_ms: 0,
            stagger_ms: default_stagger(),
            items: Vec::new(),
        }
    }
}

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("config dir: {e}"))?;
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| format!("create config dir: {e}"))?;
    }
    Ok(dir.join("launch.json"))
}

pub fn load(app: &AppHandle) -> Result<LaunchConfig, String> {
    let path = config_path(app)?;
    if !path.exists() {
        let cfg = LaunchConfig::default();
        save(app, &cfg)?;
        return Ok(cfg);
    }
    let raw = fs::read_to_string(&path).map_err(|e| format!("read config: {e}"))?;
    if raw.trim().is_empty() {
        return Ok(LaunchConfig::default());
    }
    serde_json::from_str(&raw).map_err(|e| format!("parse config: {e}"))
}

pub fn save(app: &AppHandle, cfg: &LaunchConfig) -> Result<(), String> {
    let path = config_path(app)?;
    let json = serde_json::to_string_pretty(cfg).map_err(|e| format!("serialize: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(|e| format!("write config: {e}"))?;
    fs::rename(&tmp, &path).map_err(|e| format!("commit config: {e}"))?;
    Ok(())
}

pub fn same_target(a: &str, b: &str) -> bool {
    #[cfg(windows)]
    {
        a.trim().eq_ignore_ascii_case(b.trim())
    }
    #[cfg(not(windows))]
    {
        a.trim() == b.trim()
    }
}

pub fn infer_kind(target: &str) -> ItemKind {
    let t = target.trim();
    if t.starts_with("http://") || t.starts_with("https://") {
        return ItemKind::Url;
    }
    let path = Path::new(t);
    if path.is_dir() {
        return ItemKind::Folder;
    }
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("exe" | "bat" | "cmd" | "com" | "lnk" | "msc" | "ps1") => ItemKind::App,
        _ => ItemKind::File,
    }
}

pub fn infer_name(target: &str, kind: &ItemKind) -> String {
    match kind {
        ItemKind::Url => url_name(target),
        _ => Path::new(target.trim())
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| target.trim().to_string()),
    }
}

fn url_name(target: &str) -> String {
    let trimmed = target.trim().trim_end_matches('/');
    if let Some(rest) = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
    {
        rest.split('/').next().unwrap_or(rest).to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn new_item(target: String) -> LaunchItem {
    let kind = infer_kind(&target);
    let name = infer_name(&target, &kind);
    LaunchItem {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        target: target.trim().to_string(),
        args: String::new(),
        kind,
        enabled: true,
        delay_ms: 0,
    }
}

pub fn add_target(cfg: &mut LaunchConfig, target: String) -> Result<LaunchItem, String> {
    let target = target.trim().to_string();
    if target.is_empty() {
        return Err("Empty target".into());
    }
    if cfg.items.iter().any(|i| same_target(&i.target, &target)) {
        return Err("Already in the list".into());
    }
    let item = new_item(target);
    cfg.items.push(item.clone());
    Ok(item)
}

pub fn clamp_delays(login_delay_ms: u64, stagger_ms: u64) -> (u64, u64) {
    (login_delay_ms.min(60_000), stagger_ms.min(5_000))
}

pub fn clamp_item_delay(delay_ms: u64) -> u64 {
    delay_ms.min(120_000)
}
