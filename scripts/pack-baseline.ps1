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
[System.IO.Compression.ZipFile]::CreateFromDirectory($temp, $zipPath, [System.IO.Compression.CompressionLevel]::Optimal, $false)

Write-Host ("Done: " + (Resolve-Path $zipPath).Path) -ForegroundColor Green
Write-Host ("Size: " + (Get-Item $zipPath).Length + " bytes")