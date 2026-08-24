$ErrorActionPreference = 'Stop'

# 在 git worktree 之外的位置重下,避免 checkout 把它当 untracked 清掉
$base = 'D:\Android\ndk-cache'
$zip = "$base\android-ndk-r29-windows.zip"
$expectedSha = 'AB3BB30FBB9E6903666D60C55D11E78B04E07472'
$url = 'https://dl.google.com/android/repository/android-ndk-r29-windows.zip'

New-Item -ItemType Directory -Path $base -Force | Out-Null

if (Test-Path $zip) {
    $existing = (Get-FileHash $zip -Algorithm SHA1).Hash.ToUpper()
    Write-Host ("Existing zip SHA1: " + $existing)
    if ($existing -eq $expectedSha) {
        Write-Host "Zip already matches expected SHA1, skip download" -ForegroundColor Green
    } else {
        Write-Host "SHA1 mismatch, will redownload" -ForegroundColor Yellow
        Remove-Item $zip -Force
    }
}

if (-not (Test-Path $zip)) {
    Write-Host ("Downloading NDK r29 (~795MB) from Google CDN to " + $zip)
    Write-Host "This will take ~10-20 minutes depending on network."
    Write-Host "Using curl.exe with resume support."
    $sw = [Diagnostics.Stopwatch]::StartNew()
    & curl.exe --ssl-no-revoke -L -C - --retry 3 --connect-timeout 60 -o "$zip.tmp" $url
    if ($LASTEXITCODE -ne 0) {
        Write-Host ("curl failed exit " + $LASTEXITCODE) -ForegroundColor Red
        if (Test-Path "$zip.tmp") { Remove-Item "$zip.tmp" -Force }
        exit 1
    }
    Move-Item "$zip.tmp" $zip -Force
    $sw.Stop()
    Write-Host ("Download took " + $sw.Elapsed.ToString('hh\:mm\:ss'))
}

# Verify SHA1
$actualSha = (Get-FileHash $zip -Algorithm SHA1).Hash.ToUpper()
Write-Host ("Actual SHA1:   " + $actualSha)
Write-Host ("Expected SHA1: " + $expectedSha)
if ($actualSha -ne $expectedSha) {
    Write-Host "FAIL: SHA1 mismatch" -ForegroundColor Red
    exit 1
}
Write-Host "SHA1 OK" -ForegroundColor Green

# Extract
$extracted = "$base\android-ndk-r29"
if (Test-Path $extracted) {
    Write-Host ("Already extracted: " + $extracted) -ForegroundColor Green
} else {
    Write-Host ("Extracting to " + $extracted)
    $sw = [Diagnostics.Stopwatch]::StartNew()
    Expand-Archive -Path $zip -DestinationPath $base -Force
    $sw.Stop()
    Write-Host ("Extract took " + $sw.Elapsed.ToString('hh\:mm\:ss'))
}

# Validate
$clang = Join-Path $extracted 'toolchains\llvm\prebuilt\windows-x86_64\bin\clang.exe'
if (Test-Path $clang) {
    Write-Host ("NDK ready: " + $clang) -ForegroundColor Green
    & $clang --version | Select-Object -First 1
} else {
    Write-Host ("FAIL: clang.exe not found at " + $clang) -ForegroundColor Red
    exit 1
}

# Set user env vars (no admin)
[Environment]::SetEnvironmentVariable('NDK_HOME', $extracted, 'User')
[Environment]::SetEnvironmentVariable('ANDROID_NDK_HOME', $extracted, 'User')
[Environment]::SetEnvironmentVariable('ANDROID_NDK_ROOT', $extracted, 'User')
Write-Host "Env vars set: NDK_HOME=$extracted"