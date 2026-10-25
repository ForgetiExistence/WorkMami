//! 开发模式 bridge：本地 HTTP server 接收前端（Trae 预览编辑器等）的 invoke 请求。
//! 仅在 cfg!(debug_assertions) 下启用。与 Tauri 主窗口共用同一份 AppState。
//!
//! 实现策略：commands.rs 里的函数签名是 tauri::State<'_, Arc<AppState>>，
//! 我们没法直接构造（它是 Tauri extractor 专用的包装）。
//! 所以 bridge 单独持有一份 Arc<AppState> + AppHandle，
//! 在 match 里通过 std::mem::transmute 把 &Arc<AppState> 转成 tauri::State<'static, Arc<AppState>>。
//! 生命周期在 dev 模式下是安全的：state 是长生命周期单例。

#![allow(unsafe_code)]

use std::sync::Arc;

use axum::{
    Router,
    extract::State as AxState,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
};
use serde_json::{Value, json};
use tower_http::cors::{Any, CorsLayer};

use crate::state::AppState;

/// bridge 监听端口（前端 polyfill 固定指向这个地址）。
pub const PORT: u16 = 1421;

/// POST JSON body: { "command": "app_info", "args": { ... } }
#[derive(serde::Deserialize)]
struct InvokeRequest {
    command: String,
    #[serde(default)]
    args: Value,
}

async fn invoke_handler(
    AxState(state): AxState<Arc<AppState>>,
    headers: HeaderMap,
    axum::Json(req): axum::Json<InvokeRequest>,
) -> impl IntoResponse {
    // 简易鉴权：dev 模式下只校验前端自报 header，防止被误用。
    if headers
        .get("x-wm-bridge")
        .and_then(|v| v.to_str().ok())
        .map_or(true, |v| v != "workmami-dev")
    {
        return (StatusCode::FORBIDDEN, axum::Json(json!({"error": "bad bridge header"})));
    }

    let out = match dispatch(&req.command, req.args, &state).await {
        Ok(v) => (StatusCode::OK, axum::Json(v)),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            axum::Json(json!({"error": e.to_string()})),
        ),
    };
    out
}

/// 伪造一个 tauri::State<'static, Arc<AppState>> 给 commands 用。
/// Tauri 2 的 State 是 #[repr(transparent)] newtype 包 &T，所以 transmute 是合法的。
/// 把 Arc 存到静态 STASH 里保证引用生命周期。
fn make_tauri_state(state: &Arc<AppState>) -> tauri::State<'static, Arc<AppState>> {
    // 每次覆盖写入，所以不适合 OnceLock；用 Box::leak 拿到 'static 引用
    // 避免 edition 2024 对 static mut 共享引用的 UB 警告。
    let r: &'static Arc<AppState> = Box::leak(Box::new(state.clone()));
    unsafe { std::mem::transmute::<&Arc<AppState>, tauri::State<'static, Arc<AppState>>>(r) }
}

async fn dispatch(cmd: &str, args: Value, state: &Arc<AppState>) -> Result<Value, String> {
    let s = make_tauri_state(state);

    // ----- 启动必需（store.init） -----
    match cmd {
        "app_info" => Ok(crate::commands::app_info()),
        "get_settings" => crate::commands::get_settings(s)
            .map_err(|e| e.to_string())
            .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())),
        "list_accounts" => crate::commands::list_accounts(s)
            .map_err(|e| e.to_string())
            .and_then(|v| Ok(json!(v))),
        "proxy_status" => crate::commands::proxy_status(s)
            .map_err(|e| e.to_string()),
        "ide_sync_status" => crate::commands::ide_sync_status(s)
            .map_err(|e| e.to_string())
            .and_then(|v| Ok(json!(v))),

        // ----- 设置 & 账号操作 -----
        "save_settings" => {
            let st = serde_json::from_value::<crate::state::AppSettings>(
                args.get("settings").cloned().unwrap_or(Value::Null),
            )
            .map_err(|e| format!("settings 解析失败: {e}"))?;
            crate::commands::save_settings(s, st)
                .await
                .map_err(|e| e.to_string())
                .map(|_| Value::Null)
        }
        "set_account_enabled" => {
            let uid = args["uid"].as_str().ok_or("缺少 uid")?.to_string();
            let enabled = args["enabled"].as_bool().ok_or("缺少 enabled")?;
            crate::commands::set_account_enabled(s, uid, enabled)
                .map_err(|e| e.to_string())
                .map(|_| Value::Null)
        }
        "set_account_tags" => {
            let uid = args["uid"].as_str().ok_or("缺少 uid")?.to_string();
            let tags = serde_json::from_value(args["tags"].clone()).map_err(|e| e.to_string())?;
            crate::commands::set_account_tags(s, uid, tags)
                .map_err(|e| e.to_string())
                .map(|_| Value::Null)
        }
        "delete_account" => {
            let uid = args["uid"].as_str().ok_or("缺少 uid")?.to_string();
            crate::commands::delete_account(s, uid)
                .map_err(|e| e.to_string())
                .map(|_| Value::Null)
        }
        "credential_status" => {
            let uid = args["uid"].as_str().ok_or("缺少 uid")?.to_string();
            crate::commands::credential_status(s, uid)
                .map_err(|e| e.to_string())
                .and_then(|v| Ok(json!(v)))
        }
        "credit_ledger" => {
            let uid = args["uid"].as_str().ok_or("缺少 uid")?.to_string();
            let days = args.get("days").and_then(|v| v.as_i64());
            crate::commands::credit_ledger(s, uid, days)
                .await
                .map_err(|e| e.to_string())
        }

        // ----- OAuth 网页授权 -----
        "oauth_begin" => crate::commands::oauth_begin(s)
            .await
            .map_err(|e| e.to_string())
            .and_then(|v| Ok(json!(v))),
        "oauth_poll" => {
            let sid = args["sessionId"].as_str().ok_or("缺少 sessionId")?.to_string();
            crate::commands::oauth_poll(s, sid)
                .await
                .map_err(|e| e.to_string())
        }

        // ----- 签到 -----
        "checkin_status" => {
            let uid = args["uid"].as_str().ok_or("缺少 uid")?.to_string();
            crate::commands::checkin_status(s, uid)
                .await
                .map_err(|e| e.to_string())
        }
        "checkin_all" => {
            let handle = state
                .app_handle
                .get()
                .ok_or("AppHandle 未注入")?
                .clone();
            crate::commands::checkin_all(handle, s)
                .await
                .map_err(|e| e.to_string())
                .and_then(|v| Ok(json!(v)))
        }
        "checkin_one" => {
            let uid = args["uid"].as_str().ok_or("缺少 uid")?.to_string();
            let handle = state
                .app_handle
                .get()
                .ok_or("AppHandle 未注入")?
                .clone();
            crate::commands::checkin_one(handle, s, uid)
                .await
                .map_err(|e| e.to_string())
                .and_then(|v| Ok(json!(v)))
        }

        // ----- 猫猫旅行 -----
        "travel_overview" => crate::commands::travel_overview(s)
            .await
            .map_err(|e| e.to_string()),
        "travel_depart" => {
            let uid = args["uid"].as_str().ok_or("缺少 uid")?.to_string();
            let loc = args["locationId"].as_i64().ok_or("缺少 locationId")?;
            crate::commands::travel_depart(s, uid, loc)
                .await
                .map_err(|e| e.to_string())
        }
        "travel_claim" => {
            let uid = args["uid"].as_str().ok_or("缺少 uid")?.to_string();
            crate::commands::travel_claim(s, uid)
                .await
                .map_err(|e| e.to_string())
        }

        // ----- 成长计划 -----
        "open_growth_center" => {
            let uid = args["uid"].as_str().ok_or("缺少 uid")?.to_string();
            crate::commands::open_growth_center(s, uid)
                .map_err(|e| e.to_string())
        }
        "bind_growth_center" => {
            let uid = args["uid"].as_str().ok_or("缺少 uid")?.to_string();
            crate::commands::bind_growth_center(s, uid)
                .map_err(|e| e.to_string())
        }
        "growth_bound_all" => crate::commands::growth_bound_all(s)
            .map_err(|e| e.to_string()),

        // ----- IDE 自动配置 -----
        "sync_all_ides" => crate::commands::sync_all_ides(s)
            .map_err(|e| e.to_string())
            .and_then(|v| Ok(json!(v))),

        // ----- 数据管理 -----
        "data_uids" => crate::commands::data_uids(s)
            .map_err(|e| e.to_string())
            .and_then(|v| Ok(json!(v))),
        "data_list" => {
            let category = args["category"].as_str().ok_or("缺少 category")?.to_string();
            let uid = args["uid"].as_str().map(|ss| ss.to_string());
            crate::commands::data_list(s, category, uid)
                .map_err(|e| e.to_string())
                .and_then(|v| Ok(json!(v)))
        }
        "data_read" => {
            let item: crate::data::DataItem =
                serde_json::from_value(args["item"].clone()).map_err(|e| e.to_string())?;
            crate::commands::data_read(item)
                .map_err(|e| e.to_string())
                .and_then(|v| Ok(json!(v)))
        }
        "data_delete" => {
            let items: Vec<crate::data::DataItem> =
                serde_json::from_value(args["items"].clone()).map_err(|e| e.to_string())?;
            crate::commands::data_delete(items)
                .map_err(|e| e.to_string())
                .and_then(|v| Ok(json!(v)))
        }
        "data_backup" => {
            let items: Vec<crate::data::DataItem> =
                serde_json::from_value(args["items"].clone()).map_err(|e| e.to_string())?;
            let label = args["label"].as_str().ok_or("缺少 label")?.to_string();
            let dest = args.get("destDir").and_then(|v| v.as_str()).map(|ss| ss.to_string());
            crate::commands::data_backup(items, label, dest)
                .map_err(|e| e.to_string())
                .and_then(|v| Ok(json!(v)))
        }
        "data_migrate" => {
            let from = args["fromUid"].as_str().ok_or("缺少 fromUid")?.to_string();
            let to = args["toUid"].as_str().ok_or("缺少 toUid")?.to_string();
            let cats: Vec<String> =
                serde_json::from_value(args["categories"].clone()).map_err(|e| e.to_string())?;
            crate::commands::data_migrate(from, to, cats)
                .map_err(|e| e.to_string())
                .and_then(|v| Ok(json!(v)))
        }
        "data_export" => {
            let item: crate::data::DataItem =
                serde_json::from_value(args["item"].clone()).map_err(|e| e.to_string())?;
            let dest = args["destDir"].as_str().ok_or("缺少 destDir")?.to_string();
            crate::commands::data_export(item, dest)
                .map_err(|e| e.to_string())
                .and_then(|v| Ok(json!(v)))
        }

        // ----- 看板 -----
        "dashboard_summary" => {
            let from = args["fromMs"].as_i64().ok_or("缺少 fromMs")?;
            let to = args["toMs"].as_i64().ok_or("缺少 toMs")?;
            crate::commands::dashboard_summary(s, from, to).map_err(|e| e.to_string())
        }
        "dashboard_trend" => {
            let from = args["fromMs"].as_i64().ok_or("缺少 fromMs")?;
            let to = args["toMs"].as_i64().ok_or("缺少 toMs")?;
            let g = args["granularity"].as_str().ok_or("缺少 granularity")?.to_string();
            crate::commands::dashboard_trend(s, from, to, g).map_err(|e| e.to_string())
        }
        "dashboard_top" => {
            let from = args["fromMs"].as_i64().ok_or("缺少 fromMs")?;
            let to = args["toMs"].as_i64().ok_or("缺少 toMs")?;
            let kind = args["kind"].as_str().ok_or("缺少 kind")?.to_string();
            let limit = args.get("limit").and_then(|v| v.as_i64());
            crate::commands::dashboard_top(s, from, to, kind, limit).map_err(|e| e.to_string())
        }
        "dashboard_heatmap" => {
            let from = args["fromMs"].as_i64().ok_or("缺少 fromMs")?;
            let to = args["toMs"].as_i64().ok_or("缺少 toMs")?;
            crate::commands::dashboard_heatmap(s, from, to).map_err(|e| e.to_string())
        }
        "dashboard_balances" => crate::commands::dashboard_balances(s)
            .map_err(|e| e.to_string())
            .and_then(|v| Ok(json!(v))),
        "dashboard_import_app" => {
            let uid = args.get("uid").and_then(|v| v.as_str()).map(|ss| ss.to_string());
            crate::commands::dashboard_import_app(s, uid)
                .map_err(|e| e.to_string())
                .and_then(|v| Ok(json!(v)))
        }
        "dashboard_requests" => {
            let from = args["fromMs"].as_i64().ok_or("缺少 fromMs")?;
            let to = args["toMs"].as_i64().ok_or("缺少 toMs")?;
            let limit = args.get("limit").and_then(|v| v.as_i64());
            crate::commands::dashboard_requests(s, from, to, limit).map_err(|e| e.to_string())
        }
        "dashboard_account_billing" => {
            let from = args["fromMs"].as_i64().ok_or("缺少 fromMs")?;
            let to = args["toMs"].as_i64().ok_or("缺少 toMs")?;
            crate::commands::dashboard_account_billing(s, from, to)
                .await
                .map_err(|e| e.to_string())
        }

        // ----- 异步拉数据 -----
        "list_models" => {
            let uid = args.get("uid").and_then(|v| v.as_str()).map(|ss| ss.to_string());
            crate::commands::list_models(s, uid)
                .await
                .map_err(|e| e.to_string())
                .and_then(|v| Ok(json!(v)))
        }
        "refresh_quota" => {
            let uids: Option<Vec<String>> = args
                .get("uids")
                .and_then(|v| v.as_array())
                .and_then(|arr| {
                    arr.iter()
                        .map(|x| x.as_str().map(|ss| ss.to_string()))
                        .collect::<Option<Vec<_>>>()
                });
            let handle = state
                .app_handle
                .get()
                .ok_or("AppHandle 未注入")?
                .clone();
            crate::commands::refresh_quota(handle, s, uids)
                .await
                .map_err(|e| e.to_string())
                .and_then(|v| Ok(json!(v)))
        }

        other => Err(format!("bridge 尚未实现: {other}")),
    }
}

/// 启动 bridge server。失败只 warn，不阻塞主流程。
pub fn start(state: Arc<AppState>) {
    let addr = format!("127.0.0.1:{PORT}");
    let app = Router::new()
        .route("/__invoke", post(invoke_handler))
        .layer(CorsLayer::new().allow_origin(Any).allow_headers(Any))
        .with_state(state);

    tauri::async_runtime::spawn(async move {
        let listener = match tokio::net::TcpListener::bind(
            addr.parse::<std::net::SocketAddr>()
                .expect("bridge 地址解析失败"),
        )
        .await
        {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!("dev bridge 端口绑定失败（可忽略）: {e}");
                return;
            }
        };
        match axum::serve(listener, app).await {
            Ok(_) => tracing::info!("dev bridge 已监听 http://{addr}/__invoke"),
            Err(e) => tracing::warn!("dev bridge 运行失败（可忽略）: {e}"),
        }
    });
}
