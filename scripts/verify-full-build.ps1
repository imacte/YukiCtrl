$ErrorActionPreference = 'Continue'
Set-Location D:\py\yumi

$env:Path = "C:\Users\zqk00\.cargo\bin;$env:Path"

Write-Host '=== verify BPF build (-Z build-std) ==='
& cmd /c 'cargo +nightly build -Z build-std --release -p yumi-ebpf --target bpfel-unknown-none 2>&1' | Select-Object -Last 5
$bpfOut = 'target\bpfel-unknown-none\release\yumi-ebpf'
if (Test-Path $bpfOut) {
    $len = (Get-Item $bpfOut).Length
    Write-Host "BPF OK: $bpfOut ($([math]::Round($len/1KB)) KB)" -ForegroundColor Green
} else {
    Write-Host "BPF MISSING" -ForegroundColor Red
}

Write-Host '=== verify core build (nightly ndk -t arm64-v8a --platform 26) ==='
$env:RUSTFLAGS = '-C default-linker-libraries'
$env:NDK_HOME = 'D:\Android\ndk-cache\android-ndk-r29'
$env:ANDROID_NDK_HOME = $env:NDK_HOME
& cmd /c 'cargo +nightly ndk --platform 26 -t arm64-v8a build -Z build-std -r 2>&1' | Select-Object -Last 5
$core = 'target\aarch64-linux-android\release\yumi'
if (Test-Path $core) {
    $len = (Get-Item $core).Length
    $sha1 = (Get-FileHash -Path $core -Algorithm SHA1).Hash.ToLower()
    Write-Host "core OK: $core ($([math]::Round($len/1KB)) KB) sha1=$sha1" -ForegroundColor Green
} else {
    Write-Host "core MISSING" -ForegroundColor Red
}

Write-Host '=== verify zip ==='
& cmd /c 'powershell -NoProfile -ExecutionPolicy Bypass -File D:\py\yumi\scripts\pack-baseline.ps1' | Select-Object -Last 5
$zips = Get-ChildItem 'D:\py\yumi\output\*.zip' | Sort-Object LastWriteTime -Descending | Select-Object -First 2
foreach ($z in $zips) {
    $len = $z.Length
    $sha1 = (Get-FileHash -Path $z.FullName -Algorithm SHA1).Hash.ToLower()
    Write-Host "zip: $($z.Name) ($([math]::Round($len/1KB)) KB) sha1=$sha1"
}

Write-Host '=== summary ==='
Write-Host "branch: $((& git rev-parse --abbrev-ref HEAD).Trim())"
Write-Host "head:   $((& git rev-parse --short HEAD).Trim())"
& git --no-pager log --oneline -3