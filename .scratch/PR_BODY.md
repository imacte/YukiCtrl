# ticket-02: Windows NDK r29 baseline build + webui + verify scripts

## 简介

yumi v2.0.2 在 Windows + NDK r29 + LLVM 22 上的端到端 baseline 构建链路,以及 webui 完整打包 + on-device zip 校验。

## 改了什么

### 构建链路 (NDK r29 / LLVM 22 兼容)
- `Cargo.toml`: 为 `yumi-ebpf` 设置独立 release profile (`opt-level=3`), 规避 LLVM 22+ 不再支持 `-Oz` 导致 BPF 编译失败
- `build.rs`: Windows 路径探测, 自动复制 `bpf-linker.exe` 到 `target/`, 跳过失败的 `cargo install`
- `scripts/redownload-ndk.ps1`: NDK 重下脚本 (NDK 放仓库外 `D:\Android\ndk-cache\android-ndk-r29\`, 避免被 git 清理)

### WebUI + 打包
- `scripts/webui-build.ps1`: 跑 `npm install && npm run build`
- `scripts/pack-baseline.ps1`: 改 webroot 真实复制 `webui/dist/*` 内容 (原版是占位 HTML)
- `webui/package-lock.json`: 同步 npm install

### 验证工具链
- `scripts/test-bpf-build.ps1`: BPF 单独测试
- `scripts/test-core-build.ps1`: Core 单独测试
- `scripts/verify-on-device.ps1`: adb push + unzip -t + SHA1 对比
- `scripts/check-elf.ps1`: host 端 ELF magic/machine 验证
- `scripts/verify-full-build.ps1`: BPF + core + zip 三阶段端到端验证

### 文档
- `.scratch/yumi-personalize/issues/ticket-02-baseline-build.md`: ticket-02 修复记录
- `.scratch/yumi-personalize/issues/ticket-03-percpu-perception.md`: 8路感知 + 区间寻优 spec (待 grill-me)
- `.scratch/yumi-personalize/issues/ticket-04-disable-core.md`: 关核方案 D spec (待 grill-me)

## 验证

- ✅ BPF 编译通过: `cargo +nightly build -Z build-std -p yumi-ebpf --target bpfel-unknown-none --release`
- ✅ Core 编译通过: `cargo +nightly ndk --platform 26 -t arm64-v8a build -Z build-std -r`, 1515 KB aarch64 ELF
- ✅ WebUI 构建通过: `npm install` + `npm run build`
- ✅ Zip 打包: `yumi-2.0.2-75-...zip` (1008 KB)
- ✅ Zip integrity: `unzip -t` 13 files all OK
- ✅ ELF header 验证: magic `\x7fELF`, class=64, endianness=little, machine=0xb7 (EM_AARCH64), type=ET_DYN
- ✅ On-device: 推送 zip 到小米 14 Pro (Android 14 SDK 34), SHA1 与 host 完全一致

## 风险 / 注意

1. NDK r29 路径硬编码在 `scripts/redownload-ndk.ps1`, CI 需要相应路径
2. WebUI dist 当前被 `.gitignore` `output/` 排除 (但 `webui/dist/` 没在 gitignore), 需要时可 commit dist
3. **未在手机上实跑 module** — 用户的设备是 KernelSU Next 3.3.0 而非 Magisk, 验证停在 zip 完整性 + ELF 校验层面. 装机需要 yuki 在有 Magisk 的设备上做

## 关联

- Rebase 到 `origin/main` (`5c54e0f Update update.json`)
- 3 commits: `39bf71f` + `f75271b` + `b6264d3`

## 下一步 (待 grill-me)

- ticket-03: 8路 per-cpu 感知 + 用户区间寻优 (spec 已写, 4 个决策点待确认)
- ticket-04: 关核方案 D (spec 已写, 5 个决策点待确认)