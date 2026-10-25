//! IDE 自动配置统一架构：
//! - 每个 IDE 实现 IdeSyncTarget trait（detect + sync + manual_guide 可选）
//! - sync_all(state) 统一执行，供自定义模型启动 / 模型刷新 / settings 保存 / 前端按钮四处调用
//! - 防污染：各 target 只动自己 managedBy 标记的条目

use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;

use crate::error::WmError;
use crate::state::{AppState, ProxySettings};

pub mod workbuddy;

pub type R<T> = Result<T, WmError>;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResult {
    pub id: String,
    pub label: String,
    pub detected: bool,
    pub synced: bool,
    pub written: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ManualGuideJson {
    pub title: String,
    pub subtitle: String,
    pub fields: Vec<ManualField>,
    pub footer: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ManualField {
    pub label: String,
    pub value: String,
    pub copy: bool,
}

pub trait IdeSyncTarget: Send + Sync {
    fn id(&self) -> &'static str;
    fn label(&self) -> &'static str;
    fn detect(&self) -> Option<PathBuf>;
    fn sync(&self, settings: &ProxySettings, models: &[Value]) -> R<usize>;
    /// 磁盘上是否已存在与当前代理配置一致的托管条目（状态查询用，不写盘）
    fn is_synced(&self, _settings: &ProxySettings) -> bool { false }
    fn manual_guide(&self) -> Option<ManualGuideJson> { None }
}

pub fn all_targets() -> Vec<Box<dyn IdeSyncTarget>> {
    vec![Box::new(workbuddy::WorkBuddyTarget)]
}

pub fn sync_all(state: &Arc<AppState>) -> Vec<SyncResult> {
    let targets = all_targets();
    let models = state.models_cache.lock().unwrap().clone();
    let proxy_settings = state.settings.read().unwrap().clone().proxy;

    if models.is_empty() {
        return targets.into_iter().map(|t| SyncResult {
            id: t.id().into(),
            label: t.label().into(),
            detected: t.detect().is_some(),
            synced: false,
            written: 0,
            error: Some("模型列表未拉取，跳过自动配置".into()),
        }).collect();
    }

    targets.into_iter().map(|t| {
        let detected = t.detect().is_some();
        if !detected {
            return SyncResult {
                id: t.id().into(),
                label: t.label().into(),
                detected: false,
                synced: false,
                written: 0,
                error: None,
            };
        }
        match t.sync(&proxy_settings, &models) {
            Ok(n) => SyncResult {
                id: t.id().into(),
                label: t.label().into(),
                detected: true,
                synced: true,
                written: n,
                error: None,
            },
            Err(e) => SyncResult {
                id: t.id().into(),
                label: t.label().into(),
                detected: true,
                synced: false,
                written: 0,
                error: Some(e.to_string()),
            },
        }
    }).collect()
}

pub fn status_only(state: &Arc<AppState>) -> Vec<SyncResult> {
    let ps = state.settings.read().unwrap().clone().proxy;
    all_targets().into_iter().map(|t| {
        let detected = t.detect().is_some();
        SyncResult {
            id: t.id().into(),
            label: t.label().into(),
            detected,
            synced: detected && t.is_synced(&ps),
            written: 0,
            error: None,
        }
    }).collect()
}
