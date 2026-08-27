$ErrorActionPreference = 'Stop'
$env:Path = "C:\Users\zqk00\.cargo\bin;$env:Path"

$root = 'D:\py\yumi'

$core = Join-Path $root 'target\aarch64-linux-android\release\yumi'
if (-not (Test-Path $core)) {
    Write-Host "yumi binary not found at $core" -ForegroundColor Red
    exit 1
}

# Build temp output dir like xtask does
$temp = Join-Path $root 'output\.temp'
if (Test-Path $temp) { Remove-Item $temp -Recurse -Force }
New-Item -ItemType Directory -Path $temp -Force | Out-Null

# Copy module dir
$moduleSrc = Join-Path $root 'module'
Copy-Item -Path (Join-Path $moduleSrc '*') -Destination $temp -Recurse -Force
if (Test-Path (Join-Path $temp '.gitignore')) { Remove-Item (Join-Path $temp '.gitignore') -Force }

# Copy yumi binary
$binPath = Join-Path $temp 'core\bin'
New-Item -ItemType Directory -Path $binPath -Force | Out-Null
Copy-Item -Path $core -Destination (Join-Path $binPath 'yumi') -Force

# webroot dir: copy contents of webui/dist/ (built via npm run build earlier)
$dist = Join-Path $root 'webui\dist'
$webroot = Join-Path $temp 'webroot'
New-Item -ItemType Directory -Path $webroot -Force | Out-Null
if (Test-Path $dist) {
    foreach ($item in (Get-ChildItem $dist -Force)) {
        $dest = Join-Path $webroot $item.Name
        if ($item.PSIsContainer) {
            Copy-Item -Path $item.FullName -Destination $dest -Recurse -Force
        } else {
            Copy-Item -Path $item.FullName -Destination $dest -Force
        }
    }
    Write-Host "Copied webui/dist into webroot/" -ForegroundColor Green
} else {
    Write-Host "WARN: webui/dist not found, webroot/ will be empty" -ForegroundColor Yellow
}

# Pack zip like xtask
$outputDir = Join-Path $root 'output'
New-Item -ItemType Directory -Path $outputDir -Force | Out-Null

# Same name format as xtask: yumi-{ver}-{git_count}-{date}
$cargoToml = Join-Path $root 'Cargo.toml'
$pkgVer = 'unknown'
foreach ($line in (Get-Content $cargoToml)) {
    if ($line -match '^\s*version\s*=\s*"([^"]+)"') { $pkgVer = $Matches[1]; break }
}
Push-Location $root
$gitCount = (& git rev-list --count HEAD).Trim()
Pop-Location
$dateStr = Get-Date -Format 'yyyyMMdd-HHmm'

$zipName = "yumi-$pkgVer-$gitCount-$dateStr.zip"
$zipPath = Join-Path $outputDir $zipName

Write-Host "Packaging $zipPath ..."
Add-Type -AssemblyName System.IO.Compression.FileSystem

# CreateFromDirectory emits entries with platform-native separators.
# On Windows that means '\\' which Android's zip tools do not rewrite to '/',
# causing /data/adb/modules_update/yumi/config\config.yaml style paths and
# 'No such file or directory' on chown/install. Post-process every entry name
# to use forward slashes; this is the same convention Magisk/KSU modules use.
$tmpZip = $zipPath + '.tmp'
[System.IO.Compression.ZipFile]::CreateFromDirectory($temp, $tmpZip, [System.IO.Compression.CompressionLevel]::Optimal, $false)

$rewrite = $true
if ($PSVersionTable.PSVersion.Major -ge 6) { $rewrite = $true }
$src = [System.IO.Compression.ZipFile]::OpenRead($tmpZip)
$dst = [System.IO.Compression.ZipFile]::Open($zipPath, [System.IO.Compression.ZipArchiveMode]::Create)
foreach ($e in $src.Entries) {
    $newName = $e.FullName -replace '\\', '/'
    # Detect shell scripts whose source ended up with CRLF on disk and
    # normalize to LF so Android /system/bin/sh does not choke on '\r'.
    $isShell = $newName -match '\.(sh|prop|yaml|ftl)$' -and $e.Length -gt 0
    $buf = New-Object byte[] $e.Length
    if ($isShell) {
        $s = $e.Open(); $s.Read($buf, 0, $buf.Length) | Out-Null; $s.Close()
        $text = [System.Text.Encoding]::UTF8.GetString($buf)
        if ($text.Contains("`r`n")) {
            $text = $text -replace "`r`n", "`n"
            $buf = [System.Text.Encoding]::UTF8.GetBytes($text)
        }
    } else {
        $s = $e.Open(); $s.Read($buf, 0, $buf.Length) | Out-Null; $s.Close()
    }
    $ne = $dst.CreateEntry($newName, [System.IO.Compression.CompressionLevel]::Optimal)
    $ns = $ne.Open(); $ns.Write($buf, 0, $buf.Length); $ns.Close()
}
$src.Dispose(); $dst.Dispose()
Remove-Item $tmpZip -Force

Write-Host ("Done: " + (Resolve-Path $zipPath).Path) -ForegroundColor Green
Write-Host ("Size: " + (Get-Item $zipPath).Length + " bytes")