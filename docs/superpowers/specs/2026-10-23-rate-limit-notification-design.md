# 限流已化解 · 托盘通知设计

日期：2026-10-23
状态：设计已获用户批准，待实现

## 1. 目标与定位

- 事件驱动的纯价值汇报：**只在「捕获到频控限流并自动换号化解成功」时**弹一条标准桌面通知。
- 平时零打扰、无常驻 UI、无新配置项。
- 双端（Windows/macOS）代码复用：统一走已接入的 `tauri-plugin-notification`，同一行调用，各平台渲染原生通知样式。
- 整池耗尽、429 抛回客户端时不通知（客户端已见错误）。
- 每次化解都弹（真实频率约 1 次/小时）；全局 2 分钟节流仅作异常循环安全阀。

## 2. 文案（用户定稿，逐字执行）

- title：`已切换账号至{新账号昵称}`
- body：`账号{旧账号昵称}遇到错误：{腾讯限流报文原文}`

取值规则：

1. 昵称**原样全量，不截断**（含超长/特殊字符昵称）；昵称为空回退 uid 尾 4 位（防御分支，正常不发生）。
2. 报文原文优先级：
   - SSE 错误事件：`error.message` 原文（即「访问频率过高」所在的整句，一字不改）；
   - HTTP 429 body：若可解析为 JSON 则取 `error.message`，否则整段 body 原文；
   - 不加工、不改写、不加省略号；超长时由系统通知自行截断。
3. 双端使用同一 title/body 字符串。

## 3. 数据流

```
chat_completions failover 循环
  │
  ├─ forward_once(acc_A) -> Err(FwdErr::RateLimited { retry_at, raw })
  │     记录 pending_notice = Some((A.uid, raw))
  │     （HTTP 429 与 SSE「访问频率过高」两条拦截路径均归入此错误）
  │
  ├─ forward_once(acc_B) -> Ok(resp)
  │     若 pending_notice 有值（调用线程内完成）：
  │       AppHandle 检查 -> 2 分钟节流（先占位后发送）-> 组文案
  │       -> spawn 旁路线程 app.notification().builder()
  │          .title("已切换账号至{B.nickname}")
  │          .body("账号{A.nickname}遇到错误：{raw}")
  │          .show()   // 失败静默忽略，绝不阻塞转发
  │
  └─ 所有账号失败（429 抛回客户端）→ pending_notice 丢弃，不通知
```

时序不变式：

- 通知只在「先频控、后成功」的同一次请求内触发；
- 一次请求最多弹一条（只记首个频控）；
- 通知发送在返回响应前的旁路 spawn 中，不增加客户端延迟。

## 4. 代码落点

| 位置 | 改动 |
|---|---|
| `src-tauri/src/proxy/mod.rs` `FwdErr` | `RateLimited { retry_at: i64 }` → `RateLimited { retry_at: i64, raw: String }`；429 路径把响应体原文（按 §2.2 提取后）存入 `raw`，SSE 路径把 `error.message` 存入 `raw` |
| `src-tauri/src/proxy/mod.rs` `chat_completions` | 循环内维护 `Option<(String uid, String raw)>`，Err 时填充，成功 Ok 前若有值则发送通知（含 2 分钟节流 + 昵称回退规则） |
| `src-tauri/src/state.rs` | 注入 `AppHandle`（`OnceCell<AppHandle>`，lib.rs setup 阶段写入；`tauri::AppHandle` 为 Send+Sync+Clone） |
| `src-tauri/src/lib.rs` | setup 时 `state.app_handle.set(app.handle().clone())` |
| 前端 | 无改动 |

节流状态：`AppState` 内 `last_rate_notice_ms: Mutex<i64>`，距上次不足 120_000ms 则跳过（不排队、不补发）。

## 5. 边界与错误处理

- AppHandle 未注入（理论不可能，纯防御）→ 跳过通知。
- `.show()` 返回 Err → `tracing::warn!` 后忽略，转发流程不受影响。
- 昵称查找失败 → 用 uid 尾 4 位。
- 429 body 非 UTF-8/超大 → 用 lossy 读取，仅在 JSON 解析成功时取 message，否则整段（系统截断）。
- macOS 上通知权限未授权 → `.show()` 失败走静默分支，不重试、不打扰。
- 转发循环中换号成功但已节流（2 分钟内第二次）→ 静默跳过。

## 6. 验证

1. `cargo check` 通过；`npm run build`（tsc + vite）通过（前端无改动，回归确认）。
2. 手动触发一条通知（开发期临时调用点或临时改文案触发），确认：
   - Windows 右下角托盘区上方弹出、双端样式可接受；
   - 昵称完整不截断（用「柏霖བསམ་པའི་དོན་འག」验证）；
   - title/body 与定稿逐字一致。
3. 逻辑审查三条路径：频控→成功（弹）、整池耗尽（不弹）、2 分钟内连续两次（只弹第一次）。
4. 清理临时触发代码后重新 `cargo check`。

## 7. 明确不做

- 不做点击通知跳转行为（Windows toast 点开默认激活应用即可，不额外实现）。
- 不做「积分接力/切换」类其他事件通知（用户定位为仅报限流）。
- 不做通知开关设置项（常开能力）。
- 不做恢复时间透出（方案三被否）。
- 不做账号尾号/UID 展示（定稿仅昵称）。
