# WorkMami 项目记忆

## 构建与发布约定
- **日常开发编译**：`npm run tauri build -- --debug --no-bundle`，产物 `src-tauri/target/debug/workmami.exe`（约 34MB，debug）。
- **发布替换约定（重要）**：每次编译 release 后，必须把 `src-tauri/target/release/workmami.exe` 复制到 `C:\Program Files\WorkMami\WorkMami.exe` 覆盖现有 exe（替换前先备份旧 exe 为 `WorkMami.exe.bak`）。
- **release 编译命令（推荐）**：`npm run build && cd src-tauri && cargo build --release`（先生成前端 dist，再编译 Rust release；彻底跳过 Tauri 打包，退出码干净、无需联网）。注意：`npm run tauri build` 在 release 下会进入 bundle 阶段联网下载 WiX（本机代理 502 会失败、`BUILD_EXIT=1`），但 exe 实际已生成，不影响替换。
- 环境：本机 `node` 在 `C:\Program Files\nodejs`、`cargo` 在 `%USERPROFILE%\.cargo\bin`，均不在默认 PATH；PowerShell 工具禁止调用 `cmd.exe`，执行前需手动把 Node / Cargo / MSVC 链接器目录（`...\VC\Tools\MSVC\14.44.35207\bin\Hostx64\x64`）加入 `$env:PATH`，`rustc` 会自动定位 VS2022 BuildTools（stable-x86_64-pc-windows-msvc 1.99.1）。
- 日志：编译日志重定向为 UTF-16，用 `Get-Content -Encoding Unicode` 读取；命名 `build.log`（debug）/ `build-release.log`（release），可随手删。
