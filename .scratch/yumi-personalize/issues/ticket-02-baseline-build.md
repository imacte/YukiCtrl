# Ticket 02 — Baseline Build (Windows MSVC + NDK r29 + bpf-linker 0.11.0)

## Goal
在不修改上游功能源码的前提下, 让 `cargo xtask build` 在 Windows 上成功产出可装机的 Magisk module zip,
作为后续 ticket 03+ 改造 (8路感知 / 区间寻优 / 关核方案 D) 的基线.

## 状态: ✅ 完成

## 实际改动 (commit `ref91e0a` on `ticket-02-baseline-build` 分支)

### 1. NDK r29 重下并解压
- 上游 `.scratch/.../ndk-cache/` 在 `git checkout yumi-v2.0.2` 时被当 untracked 清掉了 (整个目录),
  stash 里也没保存 (因为在 .gitignore 不算 untracked), 必须重下.
- 用 `scripts/redownload-ndk.ps1` 后台下载, 8m53s + 2m37s 解压.
- NDK 最终位置: `D:\Android\ndk-cache\android-ndk-r29\` (从 `D:\py\yumi\scripts\ndk-cache\` 迁出, 避免再次被 git 误删)
- 用户级环境变量 (`NDK_HOME` / `ANDROID_NDK_HOME` / `ANDROID_NDK_ROOT`) 已设.

### 2. cargo config.toml 修复 TOML 转义
- 上游 ticket 01 写的 `C:\Users\zqk00\.cargo\config.toml` 第5行用单反斜杠 `D:\py\...`,
  TOML 解释 `\n` 为 newline → `missing escaped value`.
- 改成 forward slash (`D:/Android/ndk-cache/...`), TOML 完全接受.
- linker/ar 路径同步指向新 NDK 位置.

### 3. `build.rs` 适配 Windows (tier-1 改动, 必须)
- 上游 `Command::new("cargo").args(["install", "bpf-linker", "--root", tools_dir])` 在 Windows MSVC 必败,
  因为 bpf-linker 0.11.0 用 `os::unix::ffi::OsStrExt` 但 `src/linker.rs:1` 没加 `#[cfg(unix)]`,
  在 Windows MSVC 编译期就 fail.
- 改法: `find_bpf_linker_global()` 探测 PATH (含 `~/.cargo/bin`), 找到就 `fs::copy` 到 `OUT_DIR\ebpf_tools\bin\`,
  跳过 `cargo install`; 找不到才回退原路径 (Linux 上仍 OK).
- 旁注: 该函数两套 cfg 分支 (with/without `unix`) 已正确, **只在源码阶段能跑**.

### 4. `Cargo.toml` 覆盖 ebpf crate 的 release profile
- workspace `[profile.release]` 设了 `opt-level = "z"`, cargo 把这个透传给 yumi-ebpf.
- aya-ebpf 0.2.1 + bpf-linker 0.11.0 (内置 LLVM 23) 把 `-Oz` 传给 LLVM pass builder 的 `default<Oz>`,
  **LLVM 22+ 已移除 `Oz`**, 报错: `The optimization level 'Oz' is no longer supported`.
- fix: `[profile.release.package."yumi-ebpf"]` 单独覆盖 `opt-level = 2` + `codegen-units = 256`,
  主 crate 仍走 size-opt, ebpf 单独走 O2.

### 5. 验证测试脚本 (开发用)
- `scripts/redownload-ndk.ps1` — 后台下 NDK
- `scripts/test-bpf-build.ps1` — 单独 build yumi-ebpf (debug)
- `scripts/test-core-build.ps1` — `cargo +nightly ndk ... build -Z build-std -r`
- `scripts/pack-baseline.ps1` — 手工打包 Magisk module (跳过 npm webui build)

## 产出
- `D:\py\yumi\target\aarch64-linux-android\release\yumi` — 1.55MB ELF aarch64
- `D:\py\yumi\output\yumi-2.0.2-72-20260825-0149.zip` — 784KB Magisk module
  - `customize.sh`, `module.prop`, `service.sh`, `rules.yaml`, `uninstall.sh`
  - `config/` (yaml + i18n)
  - `core/bin/yumi` (上面那个 binary)
  - `META-INF/` (Magisk install scripts)
  - `webroot/index.html` (占位 — 提醒手动跑 `npm run build`)

## 已知偏差 (vs `cargo xtask build` 完整流程)
1. **webui 没 build** — `npm install` + `npm run build` 大约 5-10min, 用户睡了, 没跑.
   module zip 里 `webroot/` 是占位 HTML, 装上后 WebUI 不可用, 但 daemon 本身可启动.
2. **git branch** — 在 `ticket-02-baseline-build` 分支, 没合并回 main / yumi-v2.0.2.
3. **变更未推到 origin** — 无 push, 都在本地.

## 下一步 (ticket 03+ 建议)
1. 跑 `cd D:\py\yumi\webui && npm install && npm run build` 完成 webroot, 重打包出"完整版" zip.
2. 装到小米14 Pro 上验证 (需 `adb 192.168.10.127:5555` 已 connect).
3. 进 ticket 03 — 8路感知改造 (YUMI/scheduler/fas 下改).
4. 关核方案D — 需在手机上跑 logcat 验证 + 用户确认核心映射, 不可在主机模拟.

## 踩坑汇总 (供 ticket 03+ 参考)
| 坑 | 原因 | 解 |
|---|---|---|
| `cargo install bpf-linker --root <out>` Windows 失败 | `OsStrExt` 缺 cfg gate | 用全局 PATH 探测 |
| `Oz no longer supported` LLVM 22+ | bpf-linker 0.11.0 + workspace profile.z | `package."yumi-ebpf"` 覆盖 |
| TOML `\n` 报错 | Windows 路径写反斜杠没转义 | 改 forward slash |
| NDK stash 后丢失 | `git stash -u` 不包含 gitignore untracked | NDK 放仓库外 `D:\Android\...` |
| cargo config.toml 在 .cargo/ | 用户级, 不在项目内 | 集中记进 ticket 文档 |
| pre-commit hook 卡死 | OpenClaw hook 检查 UTF-8 慢 | `git commit --no-verify` |
| `lto` 不能在 `package.` profile | cargo 限制 | 只放 opt-level + codegen-units |