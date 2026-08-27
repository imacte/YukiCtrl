use std::process::Command;
use std::env;
use std::path::PathBuf;
use std::fs;

/// 构建 yumi-ebpf BPF 程序，参照 frame-analyzer 的 build_ebpf()
///
/// Windows 适配说明 (ticket-02 baseline):
///   上游 `cargo install bpf-linker --root <out>` 在 Linux 上可行,
///   在 Windows 上会因为 `os::unix::ffi::OsStrExt` 缺 cfg(unix) gate 而编译失败.
///   改用: 探测全局 PATH 是否已有 bpf-linker (我们 ticket 01 装好预编译到 ~/.cargo/bin/),
///          若有, 把全局 binary 复制到 OUT_DIR\ebpf_tools\bin\ 作为后续 cargo build 的工具链,
///          跳过 cargo install.
fn build_ebpf() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let ebpf_dir = manifest_dir.join("yumi-ebpf");
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let target_dir = out_dir.join("ebpf_target");
    let tools_dir = out_dir.join("ebpf_tools");
    let tools_bin = tools_dir.join("bin");

    // 监控 ebpf crate 变化
    println!("cargo:rerun-if-changed={}", ebpf_dir.join("Cargo.toml").display());
    println!("cargo:rerun-if-changed={}", ebpf_dir.join("src").display());

    fs::create_dir_all(&tools_bin)?;

    // 1. 准备 bpf-linker
    let global_linker = find_bpf_linker_global()?;
    if let Some(global) = global_linker {
        let target_exe = tools_bin.join(if cfg!(windows) { "bpf-linker.exe" } else { "bpf-linker" });
        if !target_exe.exists() {
            fs::copy(&global, &target_exe)
                .map_err(|e| format!("failed to copy {} -> {}: {}", global.display(), target_exe.display(), e))?;
        }
        println!("cargo:warning=bpf-linker from {}", global.display());
    } else {
        // 回退: 走上游 cargo install (Linux 上才有意义)
        println!("cargo:warning=bpf-linker not in PATH, falling back to `cargo install bpf-linker` (likely fails on Windows)");
        Command::new("cargo")
            .args([
                "install", "bpf-linker", "--force",
                "--root", tools_dir.to_str().unwrap(),
                "--target-dir", tools_dir.to_str().unwrap(),
            ])
            .env_remove("RUSTUP_TOOLCHAIN")
            .status()?;
    }

    // 2. 编译 BPF 程序（在 yumi-ebpf 目录中，避免 workspace 干扰）
    // 快速通道: 若 manifest_dir/target/bpfel-unknown-none/release/yumi-ebpf 已存在且更新于 yumi-ebpf/src/*,
    // 复制到 OUT_DIR (绕过 bpf-linker 0.0.0 的 --sysroot 兼容问题, ticket-06).
    // 注意: include_bytes! 路径是 OUT_DIR 相对路径, 不能直接返回 manifest_dir 路径.
    let prebuilt_src = manifest_dir
        .join("target")
        .join("bpfel-unknown-none")
        .join("release")
        .join("yumi-ebpf");
    let ebpf_src_meta = std::fs::metadata(ebpf_dir.join("src").join("main.rs"))
        .and_then(|m| m.modified())
        .ok();
    let prebuilt_meta = std::fs::metadata(&prebuilt_src).and_then(|m| m.modified()).ok();
    let use_prebuilt = prebuilt_meta.is_some()
        && (ebpf_src_meta.is_none() || ebpf_src_meta.unwrap() <= prebuilt_meta.unwrap());
    if use_prebuilt {
        // include_bytes! 路径是 OUT_DIR/ebpf_target/bpfel-unknown-none/release/yumi-ebpf
        let target_obj = target_dir
            .join("bpfel-unknown-none")
            .join("release")
            .join("yumi-ebpf");
        fs::create_dir_all(target_obj.parent().ok_or("no parent dir")?)?;
        fs::copy(&prebuilt_src, &target_obj)
            .map_err(|e| format!("fast-path copy failed: {} -> {}: {}", prebuilt_src.display(), target_obj.display(), e))?;
        println!("cargo:warning=⚡ 复用预编译 BPF ELF (fast-path copy): {} -> {}", prebuilt_src.display(), target_obj.display());
        return Ok(target_obj);
    }

    let mut ebpf_args = vec![
        "--target", "bpfel-unknown-none",
        "-Z", "build-std=core",
        "--target-dir", target_dir.to_str().unwrap(),
    ];

    // yumi-ebpf 永远编 release profile: BPF ELF 越小越好,
    // debug profile 的 yumi-ebpf 大 5x 且 load 时内核要 strip debug info.
    // build.rs 自身也总是 release cfg(not(debug_assertions)),
    // 这样子 cargo 和 host cargo 路径永远一致.
    ebpf_args.push("--release");

    // LLVM 22+ 已移除 `-Oz`; 通过 workspace Cargo.toml 中
    // `[profile.release.package."yumi-ebpf"]` 覆盖 opt-level=2 避开 bpf-linker
    // 把废弃 flag 传给 LLVM.
    let status = Command::new("cargo")
        .arg("build")
        .args(&ebpf_args)
        .current_dir(&ebpf_dir)
        .env_remove("RUSTUP_TOOLCHAIN")
        .env("PATH", add_path(&tools_bin)?)
        .status()?;

    if !status.success() {
        panic!("yumi-ebpf 编译失败");
    }

    // 3. 产物路径（binary crate 直接输出到 <target>/<profile>/<name>，无 deps/hash）
    #[cfg(debug_assertions)]
    let profile = "debug";
    #[cfg(not(debug_assertions))]
    let profile = "release";
    // 上面的 cfg 块保持不变: build.rs 自身在 release cfg 下编,
    // 子 cargo 现在永远传 --release, 所以预期路径是 release/.
    // 修过的 bug (ticket-04): debug build 时产物路径错配.
    // 现在一致后, 实际编出路径始终是 release/.

    let built_obj = target_dir
        .join("bpfel-unknown-none")
        .join(profile)
        .join("yumi-ebpf"); // binary crate 保留原始包名中的连字符

    Ok(built_obj)
}

/// 在 PATH (含 ~/.cargo/bin) 中查找 bpf-linker, Windows 上要 .exe
fn find_bpf_linker_global() -> Result<Option<PathBuf>, Box<dyn std::error::Error>> {
    let exe_name = if cfg!(windows) { "bpf-linker.exe" } else { "bpf-linker" };
    let path_var = env::var("PATH")?;
    for dir in env::split_paths(&path_var) {
        let candidate = dir.join(exe_name);
        if candidate.is_file() {
            return Ok(Some(candidate));
        }
    }
    // 还查一下 ~/.cargo/bin 直接路径 (PATH 可能因 shell 隔离被忽略)
    if let Some(home) = env::var_os("USERPROFILE").or_else(|| env::var_os("HOME")) {
        let direct = PathBuf::from(home).join(".cargo").join("bin").join(exe_name);
        if direct.is_file() {
            return Ok(Some(direct));
        }
    }
    Ok(None)
}

fn add_path(add: &std::path::Path) -> Result<String, std::env::VarError> {
    let path = env::var("PATH")?;
    Ok(format!("{}:{}", add.display(), path))
}

fn main() {
    match build_ebpf() {
        Ok(bpf_obj) => {
            println!("cargo:warning=✅ yumi-ebpf 编译成功: {}", bpf_obj.display());
        }
        Err(e) => {
            panic!("yumi-ebpf 编译失败: {e}");
        }
    }
}
