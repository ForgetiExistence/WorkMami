# 反向代理页 UX 优化 · 设计规范

- 日期：2026-10-22
- 范围：仅 `src/pages/Proxy.tsx` + 后端配套改动（自动配置时机、表单状态、实时保存）
- 非目标：反向代理内核、账号系统、数据看板重构；其他 IDE 自动配置策略本体（只搭后端扩展点）

---

## 1. 交互范式切换

| 维度 | 现状 | 目标 |
|---|---|---|
| 保存触发 | 用户点"保存设置"按钮，整体写入 | **实时保存**：字段 change 后 debounce 300ms 写盘，switch toggle 立即启停反向代理 |
| 脏态 | 无 | 每个 Form.Item 旁显示 **● 红点**（表示值与后端落盘版不一致），保存中变"写入中…"，保存成功点消失 |
| 成功反馈 | message.success 全局飘字 | 顶部一条细进度条（从左到右）+ 几秒后淡出，不打断操作 |
| 入口形态 | 左卡 WorkMami 设置 + 右卡 WorkBuddy 向导 | **单卡**：IDE 自动配置区 + 设置区，没有左右分栏 |
| 手工配置 | 右卡主流程 | 每张 IDE 卡自己的「？手动配置」入口，WorkBuddy 独占，其他 IDE 暂时留空 |
| 高级字段 | 默认展开的 Collapse | **全部平铺**，不区分基础/高级 |
| 状态条 | 顶部大条 | **砍掉**——switch + 服务 Tag 已足够表达"运行中/已开启/待保存" |

---

## 2. 页面线框（最终形态）

```
┌─────────────────────────────────────────────────────────────────┐
│ 💾 已自动保存    （细进度条，写入时出现）                        │
├─────────────────────────────────────────────────────────────────┤
│ ① 一键自动配置到 IDE                                                │
│                                                                 │
│  ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐                   │
│  │WorkBuddy│ │ Trae    │ │Cursor  │ │Claude… │                   │
│  │ ✓ 已自动配置│ │ ✓ 已自动配置│ │ ✓ 已自动配置│ │⚠ 检测到│                   │
│  │…models │ │user…   │ │custom… │ │        │                   │
│  │  ？手动配置│ │        │ │        │ │        │                   │
│  └────────┘ └────────┘ └────────┘ └────────┘                   │
│                                                                 │
│  [ 一键自动配置全部已检测到的 IDE ]   [ 🔗 测试连接 ]                 │
├─────────────────────────────────────────────────────────────────┤
│ ② 反向代理设置                                                      │
│                                                                 │
│  [Switch 已开启]  ● 服务运行中                                   │
│                                                                 │
│  默认模型        │  监听地址    │  端口                         │
│  wb-balanced ●   │  127.0.0.1 ● │  8765 ●                      │
│  [新] x0.69 ⟳    │             │                              │
│                                                                 │
│  API Key                    👁 复制 重生成                       │
│  SK-•••••••••••••••••••• ●                                     │
│  （所有 IDE 接入通用这一个 Key；重生成后需等保存生效）             │
│                                                                 │
│  积分调度策略      │  上下文窗口    │  默认思考强度               │
│  临近到期优先 ●    │  128000 ●      │  medium ▾ ●               │
│                                                                 │
│  会话粘性窗口(分钟) │ Failover 最大换号数                        │
│  30 ●             │  3 ●                                        │
└─────────────────────────────────────────────────────────────────┘

IDE 卡"？手动配置"展开后（就地跨 4 列展开，不占主屏）：
┌─────────────────────────────────────────────────────────────────┐
│ WorkBuddy 手动配置向导（自动配置失败时备用）                       │
│                                                                 │
│  在 WorkBuddy → 设置 → 模型 → 添加模型，逐项照抄：                 │
│                                                                 │
│  供应商              │  接口地址                                 │
│  自定义              │  http://127.0.0.1:8765/v1 · 复制         │
│                                                                 │
│  API Key             │  模型名称                                │
│  SK-•••••••••• · 复制│  wb-balanced ▾                           │
│                                                                 │
│  高级配置（按所选模型自动勾好，照抄即可）：                        │
│  ☑ 工具调用  ☑ 图片输入  ☑ 思考模式                              │
│  ☐ 仅思考   ☐ 允许关闭   ☐ 自定义协议                             │
│                                                                 │
│  在 WorkBuddy 弹窗点"测试连接" → 保存即可。                        │
└─────────────────────────────────────────────────────────────────┘
```

---

## 3. 实时保存与脏数据标志

### 3.1 前端状态机

```
form.setFieldsValue(initial)   ← 页面加载时，读后端 settings
          │
          ▼
   Form 字段 change ── 脏标志 "●" 出现在该 Form.Item 右上角
          │
          ▼ debounce(300ms)
   patchSettings({ field: newValue })
          │
          ▼ 请求进行中
   顶部细进度条出现，form 局部禁用（或 Form.Item 显示"写入中…"）
          │
          ▼ 成功
   脏标志消失；底部进度条 1s 后淡出；自动配置 hook 触发
          │
          ▼ 失败
   脏标志保留变红色；顶栏飘一条轻量 error；不丢本地输入
```

### 3.2 与 switch 的特殊处理

Switch `enabled=true` 后反向代理应立即启动，不等 debounce——单独走 `proxy::start` 命令。但仍要写 settings 文件（异步，允许稍后完成）。Switch `enabled=false` 立即调用 `proxy::stop`，写盘可稍后。

### 3.3 刷新页面恢复

页面进入时先读后端 `settings.proxy` → `form.setFieldsValue()` → 所有脏标志置空 → 细进度条从左到右一次后淡出（"已自动配置到落盘版"的确认性动画）。

### 3.4 API Key 重生成

重生成是一个原子操作（生成新随机串 + 写 settings.proxy.apiKey + trigger proxy reload），**单独命令**，不走普通 patch 流程：

```rust
#[tauri::command]
pub async fn regenerate_api_key(state: ...) -> R<String>
```

成功后前端用返回值 setFieldValue + 清脏标志。

---

## 4. 后端扩展点：IDE 自动配置架构

### 4.1 trait 定义（后端）

```rust
// src/sync_targets/mod.rs（新模块）
pub trait IdeSyncTarget {
    fn id(&self) -> &'static str;
    fn label(&self) -> &'static str;
    /// 返回本机上该 IDE 已安装且路径可写时，才能自动配置
    fn detect(&self) -> Option<PathBuf>;
    /// 把 WorkMami 模型条目写入该 IDE 配置。返回写入条数。
    fn sync(&self, proxy_settings: &ProxySettings, models: &[serde_json::Value]) -> Result<usize>;
    /// 可选：该 IDE 的手动配置说明 JSON（供前端"？手动配置"卡就地展示）
    fn manual_guide(&self) -> Option<ManualGuideJson> { None }
}
```

### 4.2 WorkBuddy 是首个实现

把现在 `proxy::sync_models_json` 里的实现搬进：

```rust
struct WorkBuddyTarget;
impl IdeSyncTarget for WorkBuddyTarget {
    fn detect(&self) -> Option<PathBuf> {
        let path = crate::platform::workbuddy_paths().home.join("models.json");
        // 只要 ~/.workbuddy 目录存在就算安装
        path.parent().filter(|p| p.exists())
    }
    fn sync(&self, ps: &ProxySettings, models: &[Value]) -> Result<usize> {
        // 现有 sync_models_json 逻辑，只是入参改成显式传入
    }
    fn manual_guide(&self) -> Option<ManualGuideJson> { Some(WB_GUIDE.clone()) }
}
```

### 4.3 自动配置时机（统一 hook）

把现有代码里散落的三处 `proxy::sync_models_json(&state)` 调用，全部替换成统一入口 `sync_targets::sync_all(state)`：

| 触发点 | 现有代码位置 | 变更 |
|---|---|---|
| 反向代理启动 | `proxy::start` | 换调用 |
| 模型列表刷新 | `commands::list_models` | 换调用 |
| 保存 settings 后 | `commands::save_settings` | 新增 |
| **新增：实时 patch 成功后** | — | 新增 |
| **新增：页面加载时** | `store.init` → `proxy_status` | 新增 |

### 4.4 自动配置防污染（已有）

只替换 `managedBy: "WorkMami"` 标记条目，保留用户手工加的模型。写文件走临时文件 + 原子替换，避免 IDE 读到半截 JSON。

### 4.5 后端新命令（供前端 IDE 卡"一键自动配置全部"按钮调用）

```rust
#[tauri::command]
pub async fn sync_all_ides(state: State<'_, Arc<AppState>>) -> R<Vec<SyncResult>>
// 返回 [{id, label, detected, synced, written, error?}]
```

前端按此渲染 IDE 卡列表——**探测不到的 target 不渲染**，所以不会出现"未安装"那一行灰字。

---

## 5. 手工配置向导的后端结构

每张 IDE 卡的「？手动配置」卡内容来自后端 trait 的 `manual_guide()`，是一份结构化 JSON，前端直接渲染，不硬编码：

```rust
struct ManualGuideJson {
    /// Add model 弹窗里的字段值，按顺序渲染
    fields: Vec<ManualField>,
    /// 底部说明文案
    footer: String,
}
struct ManualField {
    label: String,
    value: String,   // 或 "from:api_key" / "from:base_url" 等占位
    copy: bool,      // 是否带复制按钮
}
```

未来新增 IDE 只写一个 target + 一份 guide，前端 UI 自动多出一张卡的就地说明。

---

## 6. 测试连接按钮

新增独立命令，不依赖反向代理已启动：

```rust
#[tauri::command]
pub async fn proxy_test_connection(state: ...) -> R<TestResult>
```

实现：向反向代理自身发 `GET http://{host}:{port}/v1/models`，判断是否成功返回 200 + models 数组。失败时带明确错误字符串（端口未监听 / 连接拒绝 / 鉴权失败 / 上游 5xx）。前端把结果以轻量 Alert 形式在按钮下方就地展开，不飘 message。

---

## 7. 改动清单

### 前端（src/）

| 文件 | 改动 |
|---|---|
| `pages/Proxy.tsx` | 整体重写为线框形态：砍掉状态条、砍掉右卡、砍掉保存按钮、砍掉高级折叠；新增脏数据标志逻辑；新增顶部细进度条 hook；新增 IDE 自动配置网格 + 就地"？手动配置"；实时保存 debounce + patchSettings；新增测试连接按钮 |
| `types.ts` | 新增 `SyncResult`、`ManualGuideJson`、`TestResult`、`IdeStatus`；`ProxySettings` 保持不变 |
| `api.ts` | 新增 `patchSettings` / `syncAllIdes` / `proxyTestConnection` / `regenerateApiKey` |
| `store.ts` | `saveSettings` 内部改成后端 patch 调用 + 本地 dirty diff；新增 `syncAllIdes` / `testProxy` |

### 后端（src-tauri/src/）

| 文件 | 改动 |
|---|---|
| 新增 `sync_targets/mod.rs` | trait + 注册中心（`vec![WorkBuddyTarget, ...]`）+ `sync_all` 入口 |
| `sync_targets/workbuddy.rs` | 现有 `proxy::sync_models_json` 迁移 + 结构化 guide 常量 |
| `proxy/mod.rs` | 删掉 `sync_models_json`，换成调 `sync_targets::sync_all`；保留 start/stop 核心逻辑 |
| `commands.rs` | 新增 `sync_all_ides` / `proxy_test_connection` / `regenerate_api_key`；`save_settings` 改成局部 patch 语义 |
| `platform/mod.rs` | 暂不变；未来加 Cursor/Trae 路径时再扩 |

---

## 8. 自检

- [x] **占位符**：无 "TODO/TBD"
- [x] **内部一致**：实时保存 debounce(300ms) 与 switch 立即启停的特殊处理互相独立；IDE 自动配置 hook 统一走 `sync_all`，不再散落调用
- [x] **范围**：聚焦反向代理页面 + 后端自动配置架构搭骨架，其他 IDE 自动配置暂只注册 hook 不实现
- [x] **歧义**：patch 粒度明确（单字段 change → patch）；Key 重生成单独命令，不走 patch；"脏数据标志" = Form.Item 右上角红点，保存中变"写入中…"

---

## 9. 下一步

spec 已提交到 `docs/superpowers/specs/2026-10-22-reverse-proxy-page-ux-design.md`。**请你过一眼这份文档，如果 OK，我调用 writing-plans 把它拆成具体的实现任务。** 这是 brainstorming skill checklist 的最后一步。
