//! 模型展示名映射。
//!
//! WorkBuddy 的自定义模型条目会按「name = id」渲染，只要两者一致就只显示一个清爽的名字，
//! 倍率则走 credits 字段显示在右侧。这里把官方展示名（如 `Deepseek-V4-Pro`）用作展示用 id，
//! 自定义模型再用同一套映射把它还原成上游真实 id。

use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// 品牌词的大小写映射，用于把真实 id 转成可读名字。
const MODEL_BRANDS: &[(&str, &str)] = &[
    ("deepseek", "DeepSeek"),
    ("glm", "GLM"),
    ("kimi", "Kimi"),
    ("minimax", "MiniMax"),
    ("mimo", "MiMo"),
    ("hunyuan", "Hunyuan"),
    ("hy", "Hy"),
    ("step", "Step"),
    ("doubao", "Doubao"),
    ("qwen", "Qwen"),
    ("gpt", "GPT"),
    ("claude", "Claude"),
    ("gemini", "Gemini"),
    ("space", "Space"),
    ("openai", "OpenAI"),
    ("llama", "Llama"),
];

/// 由真实 id 推导可读名字：`deepseek-v4-pro` → `DeepSeek-V4-Pro`。
pub fn pretty_model_id(id: &str) -> String {
    id.split('-')
        .map(|seg| {
            let alpha_len = seg
                .find(|c: char| !c.is_ascii_alphabetic())
                .unwrap_or(seg.len());
            let (alpha, rest) = seg.split_at(alpha_len);
            if alpha.is_empty() {
                if let Some(stem) = seg.strip_suffix('v') {
                    return format!("{stem}V");
                }
                return seg.to_string();
            }
            if let Some((_, brand)) = MODEL_BRANDS
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(alpha))
            {
                return format!("{brand}{rest}");
            }
            let mut chars = alpha.chars();
            match chars.next() {
                Some(first) => format!("{}{}{}", first.to_ascii_uppercase(), chars.as_str(), rest),
                None => seg.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join("-")
}

/// 展示用 id 只保留字母数字与 `. _ - +`，空白折叠成 `-`，避免带空格的名字被客户端当成路径处理。
fn sanitize_display(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_whitespace() { '-' } else { c })
        .filter(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-' | '+'))
        .collect()
}

/// 生成「真实 id → 展示用 id」映射。
/// 优先使用官方展示名，重名时回退到可读名，保证展示 id 全局唯一。
pub fn real_to_display(models: &[Value]) -> HashMap<String, String> {
    let mut used: HashSet<String> = HashSet::new();
    let mut map = HashMap::new();
    for m in models {
        let Some(real) = m.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        let name = m.get("name").and_then(|v| v.as_str()).unwrap_or("").trim();
        let mut display = if name.is_empty() {
            pretty_model_id(real)
        } else {
            sanitize_display(name)
        };
        if used.contains(&display) {
            display = pretty_model_id(real);
        }
        if used.contains(&display) {
            display = format!("{display}-{real}");
        }
        used.insert(display.clone());
        map.insert(real.to_string(), display);
    }
    map
}
