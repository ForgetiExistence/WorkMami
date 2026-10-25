#!/usr/bin/env node
/**
 * 版本号唯一源：package.json 的 "version" 字段。
 *
 * 每次编译前（通过 npm prebuild 钩子）自动把版本号刷到：
 *   - src-tauri/tauri.conf.json  （Tauri 打包用，决定产物文件名）
 *   - src-tauri/Cargo.toml        （crate version，保持一致）
 *
 * 手动同步：npm run sync-version
 */
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

// 1. 读 package.json 作为唯一源
const pkgPath = join(root, "package.json");
const pkg = JSON.parse(readFileSync(pkgPath, "utf8"));
const version = pkg.version;
if (!version) {
  console.error("[sync-version] package.json 缺少 version 字段");
  process.exit(1);
}

// 2. 写 tauri.conf.json
const tauriConfPath = join(root, "src-tauri/tauri.conf.json");
const tauriConf = JSON.parse(readFileSync(tauriConfPath, "utf8"));
if (tauriConf.version !== version) {
  tauriConf.version = version;
  writeFileSync(tauriConfPath, JSON.stringify(tauriConf, null, 2) + "\n", "utf8");
  console.log(`[sync-version] tauri.conf.json version -> ${version}`);
} else {
  console.log(`[sync-version] tauri.conf.json 已是 ${version}，跳过`);
}

// 3. 写 Cargo.toml（只改顶层 [package] 的 version，不动 [lib]）
const cargoPath = join(root, "src-tauri/Cargo.toml");
let cargo = readFileSync(cargoPath, "utf8");
// 匹配 [package] 块下的 version = "..."，只替换第一处（顶层 package）
const cargoRe = /(\[package\][\s\S]*?^version\s*=\s*)"[^"]*"/m;
const newCargo = cargo.replace(cargoRe, `$1"${version}"`);
if (newCargo !== cargo) {
  writeFileSync(cargoPath, newCargo, "utf8");
  console.log(`[sync-version] Cargo.toml version -> ${version}`);
} else {
  console.log(`[sync-version] Cargo.toml 已是 ${version}，跳过`);
}

console.log(`[sync-version] 完成，当前版本：${version}`);
