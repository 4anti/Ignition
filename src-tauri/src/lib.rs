mod config;
mod icon;
mod launch;

use std::collections::BTreeMap;
use std::time::Duration;

use serde::Serialize;
use tauri::webview::WebviewWindowBuilder;
use tauri::{AppHandle, DragDropEvent, Emitter, WebviewEvent, WebviewUrl};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_dialog::DialogExt;

use config::{LaunchConfig, LaunchItem};
use launch::{LaunchReport, LaunchResult};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppView {
    config: LaunchConfig,
    autostart: bool,
    icons: BTreeMap<String, String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BootProgress {
    index: usize,
    total: usize,
    name: String,
    ok: bool,
    error: Option<String>,
}

fn view(app: &AppHandle) -> Result<AppView, String> {
    let config = config::load(app)?;
    Ok(view_from(app, config))
}

fn view_from(app: &AppHandle, config: LaunchConfig) -> AppView {
    AppView {
        icons: icon::for_items(&config.items),
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        config,
    }
}

fn persist(app: &AppHandle, cfg: LaunchConfig) -> Result<AppView, String> {
    config::save(app, &cfg)?;
    Ok(view_from(app, cfg))
}

fn sleep_ms(ms: u64) {
    if ms == 0 {
        return;
    }
    std::thread::sleep(Duration::from_millis(ms));
}

#[tauri::command]
fn get_state(app: AppHandle) -> Result<AppView, String> {
    view(&app)
}

#[tauri::command]
fn pick_and_add(app: AppHandle, mode: String) -> Result<AppView, String> {
    let picked = match mode.as_str() {
        "folder" => app
            .dialog()
            .file()
            .set_title("Add folder")
            .blocking_pick_folder(),
        _ => app
            .dialog()
            .file()
            .set_title("Add program or file")
            .add_filter("Programs", &["exe", "lnk", "bat", "cmd", "com"])
            .add_filter("All files", &["*"])
            .blocking_pick_file(),
    };

    let Some(path) = picked else {
        return view(&app);
    };

    let target = path.into_path().map_err(|e| e.to_string())?;
    let mut cfg = config::load(&app)?;
    config::add_target(&mut cfg, target.to_string_lossy().into_owned())?;
    persist(&app, cfg)
}

#[tauri::command]
fn add_url(app: AppHandle, url: String) -> Result<AppView, String> {
    let url = url.trim().to_string();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("URL must start with http:// or https://".into());
    }
    let mut cfg = config::load(&app)?;
    config::add_target(&mut cfg, url)?;
    persist(&app, cfg)
}

#[tauri::command]
fn add_targets(app: AppHandle, paths: Vec<String>) -> Result<AppView, String> {
    let mut cfg = config::load(&app)?;
    let mut last_err = None;
    let mut added = 0usize;
    for path in paths {
        match config::add_target(&mut cfg, path) {
            Ok(_) => added += 1,
            Err(e) => last_err = Some(e),
        }
    }
    if added == 0 {
        return Err(last_err.unwrap_or_else(|| "Nothing to add".into()));
    }
    persist(&app, cfg)
}

#[tauri::command]
fn remove_item(app: AppHandle, id: String) -> Result<AppView, String> {
    let mut cfg = config::load(&app)?;
    let before = cfg.items.len();
    cfg.items.retain(|i| i.id != id);
    if cfg.items.len() == before {
        return Err("Item not found".into());
    }
    persist(&app, cfg)
}

#[tauri::command]
fn set_enabled(app: AppHandle, id: String, enabled: bool) -> Result<AppView, String> {
    let mut cfg = config::load(&app)?;
    let item = cfg
        .items
        .iter_mut()
        .find(|i| i.id == id)
        .ok_or_else(|| "Item not found".to_string())?;
    item.enabled = enabled;
    persist(&app, cfg)
}

#[tauri::command]
fn set_args(app: AppHandle, id: String, args: String) -> Result<AppView, String> {
    let mut cfg = config::load(&app)?;
    let item = cfg
        .items
        .iter_mut()
        .find(|i| i.id == id)
        .ok_or_else(|| "Item not found".to_string())?;
    item.args = args;
    persist(&app, cfg)
}

#[tauri::command]
fn set_item_delay(app: AppHandle, id: String, delay_ms: u64) -> Result<AppView, String> {
    let mut cfg = config::load(&app)?;
    let item = cfg
        .items
        .iter_mut()
        .find(|i| i.id == id)
        .ok_or_else(|| "Item not found".to_string())?;
    item.delay_ms = config::clamp_item_delay(delay_ms);
    persist(&app, cfg)
}

#[tauri::command]
fn set_delays(app: AppHandle, login_delay_ms: u64, stagger_ms: u64) -> Result<AppView, String> {
    let mut cfg = config::load(&app)?;
    let (login, stagger) = config::clamp_delays(login_delay_ms, stagger_ms);
    cfg.login_delay_ms = login;
    cfg.stagger_ms = stagger;
    persist(&app, cfg)
}

#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> Result<AppView, String> {
    let mgr = app.autolaunch();
    if enabled {
        mgr.enable().map_err(|e| e.to_string())?;
    } else {
        mgr.disable().map_err(|e| e.to_string())?;
    }
    view(&app)
}

#[tauri::command]
fn launch_one(app: AppHandle, id: String) -> Result<LaunchResult, String> {
    let cfg = config::load(&app)?;
    let item = cfg
        .items
        .iter()
        .find(|i| i.id == id)
        .ok_or_else(|| "Item not found".to_string())?;
    match launch::launch_item(item) {
        Ok(()) => Ok(LaunchResult {
            id: item.id.clone(),
            name: item.name.clone(),
            ok: true,
            error: None,
        }),
        Err(error) => Ok(LaunchResult {
            id: item.id.clone(),
            name: item.name.clone(),
            ok: false,
            error: Some(error),
        }),
    }
}

#[tauri::command]
async fn launch_enabled(app: AppHandle) -> Result<LaunchReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cfg = config::load(&app)?;
        Ok(launch::launch_enabled(&cfg.items, cfg.stagger_ms))
    })
    .await
    .map_err(|e| e.to_string())?
}

fn boot_sequence(app: AppHandle) -> Result<LaunchReport, String> {
    let cfg = config::load(&app)?;
    let enabled: Vec<LaunchItem> = cfg.items.into_iter().filter(|i| i.enabled).collect();
    let total = enabled.len();

    app.emit(
        "boot-status",
        serde_json::json!({
            "phase": "wait",
            "loginDelayMs": cfg.login_delay_ms,
            "total": total
        }),
    )
    .ok();

    sleep_ms(cfg.login_delay_ms.max(120));

    let mut results = Vec::new();
    for (index, item) in enabled.iter().enumerate() {
        sleep_ms(item.delay_ms);
        let outcome = launch::launch_item(item);
        let (ok, error) = match outcome {
            Ok(()) => (true, None),
            Err(e) => (false, Some(e)),
        };
        let progress = BootProgress {
            index,
            total,
            name: item.name.clone(),
            ok,
            error: error.clone(),
        };
        app.emit("boot-progress", &progress).ok();
        results.push(LaunchResult {
            id: item.id.clone(),
            name: item.name.clone(),
            ok,
            error,
        });
        if index + 1 < total {
            sleep_ms(cfg.stagger_ms);
        }
    }

    let launched = results.iter().filter(|r| r.ok).count();
    let failed = results.len().saturating_sub(launched);
    let report = LaunchReport {
        launched,
        failed,
        results,
    };
    app.emit("boot-done", &report).ok();
    sleep_ms(450);
    app.exit(0);
    Ok(report)
}

#[tauri::command]
async fn start_boot(app: AppHandle) -> Result<LaunchReport, String> {
    tauri::async_runtime::spawn_blocking(move || boot_sequence(app))
        .await
        .map_err(|e| e.to_string())?
}

fn is_launch_mode() -> bool {
    std::env::args().any(|a| a == "--launch")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .args(["--launch"])
                .app_name("Ignition")
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            get_state,
            pick_and_add,
            add_url,
            add_targets,
            remove_item,
            set_enabled,
            set_args,
            set_item_delay,
            set_delays,
            set_autostart,
            launch_one,
            launch_enabled,
            start_boot
        ])
        .setup(|app| {
            if is_launch_mode() {
                WebviewWindowBuilder::new(app, "splash", WebviewUrl::App("splash.html".into()))
                    .title("Ignition")
                    .inner_size(380.0, 196.0)
                    .resizable(false)
                    .maximizable(false)
                    .minimizable(false)
                    .decorations(false)
                    .always_on_top(true)
                    .skip_taskbar(true)
                    .center()
                    .visible(true)
                    .build()?;
            } else {
                let window =
                    WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                        .title("Ignition")
                        .inner_size(1080.0, 640.0)
                        .min_inner_size(880.0, 540.0)
                        .decorations(false)
                        .resizable(true)
                        .center()
                        .visible(true)
                        .build()?;

                let handle = app.handle().clone();
                window.on_webview_event(move |event| match event {
                    WebviewEvent::DragDrop(DragDropEvent::Enter { .. }) => {
                        let _ = handle.emit("drag-state", true);
                    }
                    WebviewEvent::DragDrop(DragDropEvent::Leave) => {
                        let _ = handle.emit("drag-state", false);
                    }
                    WebviewEvent::DragDrop(DragDropEvent::Drop { paths, .. }) => {
                        let _ = handle.emit("drag-state", false);
                        let paths: Vec<String> = paths
                            .iter()
                            .map(|p| p.to_string_lossy().into_owned())
                            .collect();
                        if !paths.is_empty() {
                            let _ = handle.emit("paths-dropped", paths);
                        }
                    }
                    _ => {}
                });
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Ignition");
}
