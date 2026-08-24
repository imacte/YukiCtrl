$ErrorActionPreference = 'Stop'
$ErrorActionPreference = 'Continue'

# Find latest zip in output/
$zip = Get-ChildItem 'D:\py\yumi\output\*.zip' | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $zip) { Write-Host "no zip in output/"; exit 1 }
Write-Host "Verifying: $($zip.FullName) ($([math]::Round($zip.Length/1024)) KB)"

# Inspect contents on host first
Write-Host "--- host-side contents ---"
Add-Type -AssemblyName System.IO.Compression.FileSystem
$z = [System.IO.Compression.ZipFile]::OpenRead($zip.FullName)
$z.Entries | ForEach-Object {
    $size = $_.Length
    "{0,8} {1}" -f $size, $_.FullName
}
$z.Dispose()

# Push to device, verify integrity
Write-Host "--- adb push ---"
& adb push $zip.FullName '/sdcard/Download/yumi-baseline.zip'

Write-Host "--- adb shell unzip listing ---"
& adb shell 'cd /sdcard/Download && unzip -l yumi-baseline.zip' | Select-Object -Skip 2

Write-Host "--- adb shell extract yumi binary (use 'unzip -j' to flatten paths) ---"
# -j junk paths (Android zip entry names use '\' which Android unzip can't recreate)
& adb shell 'rm -rf /sdcard/Download/verify && mkdir -p /sdcard/Download/verify && cd /sdcard/Download && unzip -o -j yumi-baseline.zip core/bin/yumi -d verify/'
& adb shell 'ls -la /sdcard/Download/verify/core/bin/yumi 2>/dev/null; ls -la /sdcard/Download/verify/yumi 2>/dev/null'
& adb shell 'file /sdcard/Download/verify/yumi 2>/dev/null || file /sdcard/Download/verify/core/bin/yumi 2>/dev/null'

# Compare SHA1 with host
$hostHash = (Get-FileHash -Path $zip.FullName -Algorithm SHA1).Hash.ToLower()
$deviceBin = '/sdcard/Download/verify/yumi'
$ext = (& adb shell "test -f $deviceBin && echo y || echo n").Trim()
if ($ext -eq 'n') { $deviceBin = '/sdcard/Download/verify/core/bin/yumi' }
$deviceHash = (& adb shell "sha1sum $deviceBin").Trim().ToLower().Split(' ')[0]
Write-Host "host  zip SHA1:   $hostHash"
Write-Host "device bin SHA1: $deviceHash"
if ($deviceHash -eq $hostHash) { Write-Host "MATCH (within zip)" -ForegroundColor Green }
elseif (-not $deviceHash) { Write-Host "(device hash unavailable, but file extracted OK)" -ForegroundColor Yellow }
else { Write-Host "MISMATCH" -ForegroundColor Red }

# Also dump module.prop and customize.sh via unzip -p (no path issues)
& adb shell 'cd /sdcard/Download && unzip -p yumi-baseline.zip module.prop'
& adb shell 'cd /sdcard/Download && unzip -p yumi-baseline.zip customize.sh | head -5'

# Cleanup
& adb shell 'rm -rf /sdcard/Download/verify /sdcard/Download/yumi-baseline.zip'
Write-Host "--- DONE ---"