use serde::Serialize;
use std::io::ErrorKind;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::config::{ItemKind, LaunchItem};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchResult {
    pub id: String,
    pub name: String,
    pub ok: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchReport {
    pub launched: usize,
    pub failed: usize,
    pub results: Vec<LaunchResult>,
}

pub fn launch_item(item: &LaunchItem) -> Result<(), String> {
    match item.kind {
        ItemKind::App if !item.args.trim().is_empty() && !is_shortcut(&item.target) => {
            launch_app(&item.target, &item.args)
        }
        _ => shell_open(&item.target),
    }
}

fn is_shortcut(target: &str) -> bool {
    Path::new(target)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.eq_ignore_ascii_case("lnk"))
        .unwrap_or(false)
}

fn shell_open(target: &str) -> Result<(), String> {
    open::that_detached(target).map_err(map_io)
}

fn launch_app(target: &str, args: &str) -> Result<(), String> {
    let extra = split_args(args);
    let mut cmd = Command::new(target);
    cmd.args(extra);
    spawn_detached(cmd)
}

fn spawn_detached(mut cmd: Command) -> Result<(), String> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x00000008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x01000000;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let base = DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW;
        cmd.creation_flags(base | CREATE_BREAKAWAY_FROM_JOB);
        match cmd.spawn() {
            Ok(_) => return Ok(()),
            Err(e) if e.raw_os_error() == Some(5) => {
                cmd.creation_flags(base);
            }
            Err(e) => return Err(map_io(e)),
        }
    }

    match cmd.spawn() {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => {
            Err("Not found. The path may have moved.".into())
        }
        Err(e) => Err(map_io(e)),
    }
}

fn map_io(err: std::io::Error) -> String {
    match err.raw_os_error() {
        Some(5) => "Windows blocked this open (access denied).".into(),
        Some(2) => "Not found. The path may have moved.".into(),
        _ => err.to_string(),
    }
}

pub fn split_args(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    for c in input.chars() {
        match c {
            '"' => in_quotes = !in_quotes,
            c if c.is_whitespace() && !in_quotes => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn sleep_ms(ms: u64) {
    if ms == 0 {
        return;
    }
    std::thread::sleep(Duration::from_millis(ms));
}

pub fn launch_enabled(items: &[LaunchItem], stagger_ms: u64) -> LaunchReport {
    let enabled: Vec<&LaunchItem> = items.iter().filter(|i| i.enabled).collect();
    let mut results = Vec::new();
    for (index, item) in enabled.iter().enumerate() {
        sleep_ms(item.delay_ms);
        match launch_item(item) {
            Ok(()) => results.push(LaunchResult {
                id: item.id.clone(),
                name: item.name.clone(),
                ok: true,
                error: None,
            }),
            Err(error) => results.push(LaunchResult {
                id: item.id.clone(),
                name: item.name.clone(),
                ok: false,
                error: Some(error),
            }),
        }
        if index + 1 < enabled.len() {
            sleep_ms(stagger_ms);
        }
    }
    let launched = results.iter().filter(|r| r.ok).count();
    let failed = results.len().saturating_sub(launched);
    LaunchReport {
        launched,
        failed,
        results,
    }
}
