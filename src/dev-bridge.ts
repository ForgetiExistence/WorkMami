//! Dev bridge polyfill：当 Tauri runtime 不存在时（例如 Trae 内置浏览器），
//! 把 invoke 替换为走本地 axum bridge server（端口 PORT）。
//! 只在 !window.__TAURI__ 时生效，Tauri 窗口完全不受影响。

export const BRIDGE_PORT = 1421;

export function hasTauri(): boolean {
  // Tauri 2 始终注入 __TAURI_INTERNALS__（核心 IPC 标记），
  // __TAURI__ 只有 withGlobalTauri: true 才注入。前者更可靠。
  const g = globalThis as any;
  return !!(g.__TAURI_INTERNALS__ || g.__TAURI__);
}

/// 返回一个可直接调用的 invoke 实现。
/// 由 api.ts 在模块顶层用它来决定最终走哪条路径。
export async function makeInvoke(): Promise<(cmd: string, args?: Record<string, unknown>) => any> {
  if (hasTauri()) {
    // 动态 import，只在 Tauri runtime 存在时才真正加载
    const m = await import("@tauri-apps/api/core");
    return (cmd, args) => m.invoke(cmd as any, args as any);
  }

  // bridge 实现
  return async function bridgeInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
    const res = await fetch(`http://127.0.0.1:${BRIDGE_PORT}/__invoke`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "x-wm-bridge": "workmami-dev",
      },
      body: JSON.stringify({ command: cmd, args: args ?? {} }),
    });
    const data = await res.json().catch(() => ({} as any));
    if (!res.ok) {
      const msg = (data && (data.error || data.message)) || `bridge HTTP ${res.status}`;
      throw new Error(msg);
    }
    return data as T;
  };
}
