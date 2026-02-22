use std::process::Command;
use std::env;
use std::path::Path;

fn main() {
    let bpf_src = "src/bpf/fps_probe.c";
    let bpf_header = "src/bpf/bpf_abi.h";

    let out_dir = env::var("OUT_DIR").unwrap();
    let bpf_obj = Path::new(&out_dir).join("fps_probe.o");

    println!("cargo:rerun-if-changed={}", bpf_src);
    println!("cargo:rerun-if-changed={}", bpf_header);

    let clang_path = "/usr/bin/clang";

    let status = Command::new(clang_path)
    .args([
        "-target", "bpfel-unknown-none", // 明确指定为纯净 BPF 目标
        "-O2",
        "-c",
        // 保持以下纯净参数
        "-fno-addrsig",
        "-fno-ident",
        bpf_src,
        "-o",
        bpf_obj.to_str().unwrap(),
    ])
    .status()
    .expect("Failed to execute system clang");

    if !status.success() {
        panic!("eBPF compilation failed!");
    }

    println!("cargo:rustc-env=BPF_OBJ_PATH={}", bpf_obj.display());
}