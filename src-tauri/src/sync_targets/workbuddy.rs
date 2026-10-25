use std::path::PathBuf;

use serde_json::{Value, json};

use crate::state::ProxySettings;

use super::{IdeSyncTarget, ManualField, ManualGuideJson, R};

pub struct WorkBuddyTarget;

impl IdeSyncTarget for WorkBuddyTarget {
    fn id(&self) -> &'static str {
        "workbuddy"
    }

    fn label(&self) -> &'static str {
        "WorkBuddy"
    }

    fn detect(&self) -> Option<PathBuf> {
        let home = crate::platform::workbuddy_paths().home.clone();
        if home.exists() {
            Some(home)
        } else {
            None
        }
    }

    fn sync(&self, ps: &ProxySettings, models: &[Value]) -> R<usize> {
        tracing::info!("WorkBuddy sync: detected={}, models={}", self.detect().is_some(), models.len());
        let display_host = if ps.host == "0.0.0.0" { "127.0.0.1" } else { ps.host.as_str() };
        let url = format!("http://{display_host}:{}/v1/chat/completions", ps.port);

        let display_map = crate::model_display::real_to_display(models);
        let ours: Vec<Value> = models
            .iter()
            .filter_map(|m| {
                let id = m.get("id").and_then(|v| v.as_str())?;
                let reasoning = m.get("raw").and_then(|r| r.get("reasoning"));
                let can_disable = reasoning
                    .and_then(|r| r.get("canDisableThinking"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                let default_effort = m
                    .get("defaultEffort")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .unwrap_or("medium");
                let credits = m.get("credits").and_then(|v| v.as_str()).unwrap_or("");
                // WorkBuddy 会按「name:id」渲染自定义模型条目，只有 name 与 id 完全一致
                // 时才只显示一个名字。这里统一用展示用 id 作为 name，倍率放到 credits 字段，
                // 由官方渲染成右侧的「0.51x」，与内置模型排版一致。
                let display_id = display_map
                    .get(id)
                    .cloned()
                    .unwrap_or_else(|| crate::model_display::pretty_model_id(id));
                Some(json!({
                    "id": display_id,
                    "name": display_id,
                    "credits": credits,
                    "url": url,
                    "apiKey": ps.api_key,
                    "maxInputTokens": m.get("contextWindow").and_then(|v| v.as_i64()).filter(|v| *v > 0).unwrap_or(131072),
                    "maxOutputTokens": m.get("maxOutputTokens").and_then(|v| v.as_i64()).filter(|v| *v > 0).unwrap_or(8192),
                    "supportsToolCall": m.get("supportsToolCall").and_then(|v| v.as_bool()).unwrap_or(true),
                    "supportsImages": m.get("supportsImages").and_then(|v| v.as_bool()).unwrap_or(false),
                    "supportsReasoning": m.get("supportsReasoning").and_then(|v| v.as_bool()).unwrap_or(false),
                    "reasoning": {
                        "defaultEffort": default_effort,
                        "supportedEfforts": ["low", "medium", "high"],
                        "canDisableThinking": can_disable,
                    },
                    "managedBy": "WorkMami",
                }))
            })
            .collect();

        if ours.is_empty() {
            return Ok(0);
        }

        let path = crate::platform::workbuddy_paths().home.join("models.json");
        let mut kept: Vec<Value> = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
            .into_iter()
            .filter(|e| {
                e.get("managedBy").and_then(|v| v.as_str()) != Some("WorkMami")
            })
            .collect();

        let base = kept.len();
        kept.extend(ours);

        let bytes = match serde_json::to_vec_pretty(&kept) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("WorkBuddy sync: 序列化 models.json 失败: {e}");
                return Err(e.into());
            }
        };
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let tmp = path.with_extension("json.wm-tmp");
        if let Err(e) = std::fs::write(&tmp, &bytes) {
            tracing::warn!("WorkBuddy sync: 写入临时文件失败: {e}");
            return Err(e.into());
        }
        if let Err(e) = std::fs::rename(&tmp, &path) {
            tracing::warn!("WorkBuddy sync: 原子替换 models.json 失败: {e}");
            let _ = std::fs::remove_file(&tmp);
            return Err(e.into());
        }

        tracing::info!("WorkBuddy sync: 写入 {} 个条目，保留原有 {}", kept.len() - base, base);
        Ok(kept.len() - base)
    }

    fn is_synced(&self, ps: &ProxySettings) -> bool {
        let path = crate::platform::workbuddy_paths().home.join("models.json");
        let Some(arr) = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .and_then(|v| v.as_array().cloned())
        else {
            return false;
        };
        let display_host = if ps.host == "0.0.0.0" { "127.0.0.1" } else { ps.host.as_str() };
        let url = format!("http://{display_host}:{}/v1/chat/completions", ps.port);
        // 与 sync() 写入的条目逐字段比对：存在同配置的托管条目即视为已配置。
        arr.iter().any(|e| {
            e.get("managedBy").and_then(|v| v.as_str()) == Some("WorkMami")
                && e.get("url").and_then(|v| v.as_str()) == Some(url.as_str())
                && e.get("apiKey").and_then(|v| v.as_str()) == Some(ps.api_key.as_str())
        })
    }

    fn manual_guide(&self) -> Option<ManualGuideJson> {
        Some(ManualGuideJson {
            title: "在 WorkBuddy 中手动添加 WorkMami 供应商".into(),
            subtitle: "WorkBuddy 未检测到 .workbuddy 目录，或自动同步失败时可按以下步骤手动添加".into(),
            fields: vec![
                ManualField {
                    label: "供应商名称".into(),
                    value: "WorkMami".into(),
                    copy: true,
                },
                ManualField {
                    label: "接口地址".into(),
                    value: "http://127.0.0.1:8765/v1/chat/completions".into(),
                    copy: true,
                },
                ManualField {
                    label: "API Key".into(),
                    value: "（在 WorkMami 设置 → 自定义模型里配置后复制）".into(),
                    copy: false,
                },
                ManualField {
                    label: "模型名称（示例）".into(),
                    value: "wb-balanced".into(),
                    copy: true,
                },
            ],
            footer: "接口地址中的 8765 请替换为你实际配置的自定义模型端口；模型列表以 http://127.0.0.1:<port>/v1/models 为准".into(),
        })
    }
}

