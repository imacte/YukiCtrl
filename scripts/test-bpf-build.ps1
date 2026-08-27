$env:Path = "C:\Users\zqk00\.cargo\bin;$env:Path"
$env:NDK_HOME = 'D:\Android\ndk-cache\android-ndk-r29'
$env:ANDROID_NDK_HOME = $env:NDK_HOME
$env:ANDROID_NDK_ROOT = $env:NDK_HOME

Set-Location D:\py\yumi
Write-Host "=== Building yumi-ebpf only (no main crate) ==="
Write-Host "This validates build.rs + bpf-linker integration."

# bpfel-unknown-none 是 tier-3, 不带 std, 我们的代码用 #![no_std] 应该 OK
# 但是 build.rs 通过 +Z build-std=core 会拉 std 镜像 — 必须 nightly
# 我们用 nightly 当前是 default
$sw = [Diagnostics.Stopwatch]::StartNew()

# 直接 build yumi-ebpf, 不通过主 build.rs
Push-Location D:\py\yumi\yumi-ebpf
try {
    & cargo +nightly build --target bpfel-unknown-none -Z build-std=core --release 2>&1 | Tee-Object -FilePath D:\py\yumi\scripts\test-bpf-build.log
    if ($LASTEXITCODE -eq 0) {
        Write-Host "BPF build OK" -ForegroundColor Green
    } else {
        Write-Host "BPF build failed" -ForegroundColor Red
    }
} finally {
    Pop-Location
}
$sw.Stop()
Write-Host ("Elapsed: " + $sw.Elapsed.ToString('hh\:mm\:ss'))