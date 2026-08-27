$env:Path = "C:\Users\zqk00\.cargo\bin;$env:Path"
$env:NDK_HOME = 'D:\Android\ndk-cache\android-ndk-r29'
$env:ANDROID_NDK_HOME = $env:NDK_HOME
$env:ANDROID_NDK_ROOT = $env:NDK_HOME

Set-Location D:\py\yumi
Write-Host "=== Building yumi core via cargo ndk ==="

# xtask/build_core 用 -C default-linker-libraries, 也保留它
$env:RUSTFLAGS = '-C default-linker-libraries'

$sw = [Diagnostics.Stopwatch]::StartNew()
& cargo +nightly ndk --platform 26 -t arm64-v8a build -Z build-std -r 2>&1 | Tee-Object -FilePath D:\py\yumi\scripts\test-core-build.log
$LAST = $LASTEXITCODE
$sw.Stop()
Write-Host ("Elapsed: " + $sw.Elapsed.ToString('hh\:mm\:ss'))
if ($LAST -eq 0) {
    Write-Host "Core build OK" -ForegroundColor Green
    Get-Item target\aarch64-linux-android\release\yumi -ErrorAction SilentlyContinue | Select-Object Name,Length,LastWriteTime
} else {
    Write-Host "Core build failed (exit $LAST)" -ForegroundColor Red
    Write-Host "Last 50 lines of log:"
    Get-Content D:\py\yumi\scripts\test-core-build.log -Tail 50
}