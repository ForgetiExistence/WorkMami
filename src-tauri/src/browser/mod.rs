//! 成长计划人工操作窗口：为每个账号拉起一个**独立浏览器 profile**。
//!
//! 背景：WorkMami 只保存插件 Bearer token（供 /v2/plugin、/billing 等 JSON 接口），
//! 不持有网页 cookie 会话；成长计划 SPA（www.codebuddy.cn/profile/growth-space）
//! 靠浏览器自身登录态判定身份，插件 token 无法注入网页。因此给每个 uid 分配独立的
//! `--user-data-dir`，由浏览器自己保存该账号的网页登录。
//!
//! 门禁：只有当独立目录里检测到网页登录态（workbuddy.cn / codebuddy.cn 的 cookie），
//! 才允许直接「打开成长计划」；未记忆的账号拒绝启动，要求先「登录/记忆」（重新扫码）。

use std::path::PathBuf;
use std::process::Command;

use rusqlite::{Connection, OpenFlags};
use serde_json::Value;

use crate::api::GROWTH_CENTER_URL;
use crate::error::{WmError, WmResult};
use crate::platform::workmami_paths;

/// 账号专属浏览器 profile 根目录：<数据目录>/browser-profiles。
pub fn profiles_root() -> PathBuf {
    workmami_paths().root.join("browser-profiles")
}

/// 账号专属浏览器 profile 目录：<数据目录>/browser-profiles/<uid>。
/// 若该账号经 OAuth 登录在临时目录（pending-*）里建立了登录态，则返回别名指向的目录。
pub fn profile_dir(uid: &str) -> PathBuf {
    let root = profiles_root();
    if let Some(dir) = alias_get(&root, uid) {
        return root.join(dir);
    }
    root.join(safe_seg(uid))
}

/// OAuth 新增账号用的临时登录 profile 目录名：pending-<session>。
pub fn temp_profile_name(session_id: &str) -> String {
    format!("pending-{}", safe_seg(session_id))
}

/// OAuth 新增账号用的临时登录 profile 目录。
pub fn temp_profile_dir(session_id: &str) -> PathBuf {
    profiles_root().join(temp_profile_name(session_id))
}

/// 记忆 profile 位置：uid → 目录名别名，持久化到 <根>/index.json。
/// OAuth 登录成功（拿到 uid）后立即调用，此后 open_growth_center 直接复用该目录。
pub fn bind_profile(uid: &str, dir_name: &str) {
    alias_set(&profiles_root(), uid, dir_name);
}

fn index_file(root: &std::path::Path) -> PathBuf {
    root.join("index.json")
}

fn load_index(root: &std::path::Path) -> std::collections::HashMap<String, String> {
    std::fs::read(index_file(root))
        .ok()
        .and_then(|raw| serde_json::from_slice(&raw).ok())
        .unwrap_or_default()
}

fn alias_set(root: &std::path::Path, uid: &str, dir_name: &str) {
    let mut m = load_index(root);
    m.insert(uid.to_string(), dir_name.to_string());
    if std::fs::create_dir_all(root).is_ok() {
        if let Ok(json) = serde_json::to_vec_pretty(&m) {
            let _ = std::fs::write(index_file(root), json);
        }
    }
}

fn alias_get(root: &std::path::Path, uid: &str) -> Option<String> {
    let dir = load_index(root).get(uid)?.clone();
    // 别名指向的目录仍存在才生效，否则回落 uid 本名目录（兼容旧数据）
    if root.join(&dir).exists() {
        Some(dir)
    } else {
        None
    }
}

/// uid 规整成安全目录名：保留字母数字、连字符、下划线，其余替换为下划线。
fn safe_seg(uid: &str) -> String {
    uid.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

struct BrowserCandidate {
    label: &'static str,
    exe: PathBuf,
}

/// 按优先级列出候选浏览器可执行文件（调用方再判存在）。
fn candidates() -> Vec<BrowserCandidate> {
    let mut v = Vec::new();

    #[cfg(target_os = "windows")]
    {
        let pf = std::env::var("ProgramFiles").ok();
        let pf_x86 = std::env::var("ProgramFiles(x86)").ok();
        let local = std::env::var("LOCALAPPDATA").ok();

        // Edge：Windows 系统自带，优先
        for base in pf_x86.iter().chain(pf.iter()) {
            v.push(BrowserCandidate {
                label: "Microsoft Edge",
                exe: PathBuf::from(base).join("Microsoft/Edge/Application/msedge.exe"),
            });
        }
        // Chrome：系统目录 + 用户目录
        for base in pf.iter().chain(pf_x86.iter()).chain(local.iter()) {
            v.push(BrowserCandidate {
                label: "Google Chrome",
                exe: PathBuf::from(base).join("Google/Chrome/Application/chrome.exe"),
            });
        }
    }

    #[cfg(target_os = "macos")]
    {
        v.push(BrowserCandidate {
            label: "Google Chrome",
            exe: PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
        });
        v.push(BrowserCandidate {
            label: "Microsoft Edge",
            exe: PathBuf::from("/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge"),
        });
    }

    v
}

/// 探测第一个实际存在的浏览器，返回（展示名, 可执行路径）。
pub fn detect() -> Option<(&'static str, PathBuf)> {
    candidates()
        .into_iter()
        .find(|c| c.exe.is_file())
        .map(|c| (c.label, c.exe))
}

/// Chromium profile 内 cookie 库候选位置（新版在 Network/，旧版直接在 Default/）。
fn cookie_files(dir: &PathBuf) -> Vec<PathBuf> {
    let default = dir.join("Default");
    vec![
        default.join("Network").join("Cookies"),
        default.join("Cookies"),
    ]
}

/// 只读查询 cookie 库中目标域记录数；打不开/无表返回 None（交由调用方回落）。
fn target_cookie_count(path: &PathBuf) -> Option<i64> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
    conn.query_row(
        "SELECT COUNT(*) FROM cookies \
         WHERE host_key LIKE '%workbuddy.cn%' OR host_key LIKE '%codebuddy.cn%'",
        [],
        |r| r.get::<_, i64>(0),
    )
    .ok()
}

/// 是否已在独立目录中保存网页登录态（检测目标域 cookie）。
pub fn is_bound(uid: &str) -> bool {
    let dir = profile_dir(uid);
    if !dir.exists() {
        return false;
    }
    for f in cookie_files(&dir) {
        if !f.is_file() {
            continue;
        }
        match target_cookie_count(&f) {
            // 查到目标域 cookie → 已记忆
            Some(n) if n > 0 => return true,
            // 库可读但无目标域 cookie → 继续看下一个候选文件
            Some(_) => continue,
            // 库被锁/读不了 → 回落：文件非空视为已有会话（避免误拒）
            None => {
                if let Ok(meta) = std::fs::metadata(&f) {
                    if meta.len() > 0 {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// 用独立 profile 打开指定 URL；返回浏览器子进程句柄（spawn 后不等待）。
pub fn launch_child(exe: &PathBuf, dir: &PathBuf, url: &str) -> WmResult<std::process::Child> {
    std::fs::create_dir_all(dir)?;
    Command::new(exe)
        .arg(format!("--user-data-dir={}", dir.display()))
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg(url)
        .spawn()
        .map_err(WmError::from)
}

/// 用独立 profile 拉起浏览器直达成长计划；spawn 后不等待，浏览器独立运行。
fn launch(label: &str, exe: &PathBuf, dir: &PathBuf) -> WmResult<Value> {
    launch_child(exe, dir, GROWTH_CENTER_URL)?;
    Ok(serde_json::json!({
        "browser": label,
        "profileDir": dir.to_string_lossy().into_owned(),
        "url": GROWTH_CENTER_URL,
    }))
}

/// 在指定 profile 目录中打开成长计划（OAuth 登录成功后自动衔接；
/// 若该 profile 的浏览器仍在运行，只会在既有窗口里新开一个标签页）。
pub fn open_growth_in(dir: &PathBuf) -> WmResult<Value> {
    let (label, exe) = detect().ok_or_else(|| {
        WmError::Other("未找到可用浏览器（Microsoft Edge 或 Google Chrome），请安装其一后重试".into())
    })?;
    launch(&label, &exe, dir)
}

/// 直接打开成长计划：仅当已记忆网页登录态才放行；未记忆**不启动浏览器**，
/// 返回 `{bound:false, reason}` 由前端提示用户先退出并重新登录（扫码记忆）。
pub fn open_growth_center(uid: &str) -> WmResult<Value> {
    if !is_bound(uid) {
        return Ok(serde_json::json!({
            "bound": false,
            "reason": "该账号尚未记忆成长计划登录态，请先退出账号并重新登录（在独立窗口扫码一次）后再打开",
        }));
    }
    let (label, exe) = detect().ok_or_else(|| {
        WmError::Other("未找到可用浏览器（Microsoft Edge 或 Google Chrome），请安装其一后重试".into())
    })?;
    let dir = profile_dir(uid);
    let mut v = launch(label, &exe, &dir)?;
    v["bound"] = serde_json::json!(true);
    Ok(v)
}

/// 登录/记忆：不校验状态，直接拉起独立窗口供扫码建立登录态
/// （即“退出账号重新登录”的实际落点），成功后该目录即被记忆。
pub fn bind_growth_center(uid: &str) -> WmResult<Value> {
    let (label, exe) = detect().ok_or_else(|| {
        WmError::Other("未找到可用浏览器（Microsoft Edge 或 Google Chrome），请安装其一后重试".into())
    })?;
    let dir = profile_dir(uid);
    let mut v = launch(label, &exe, &dir)?;
    v["bound"] = serde_json::json!(false);
    Ok(v)
}
