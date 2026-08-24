$ErrorActionPreference = 'Stop'
Set-Location D:\py\yumi\webui

Write-Host "=== webui: npm install ===" -ForegroundColor Cyan
$sw = [Diagnostics.Stopwatch]::StartNew()
$proc = Start-Process -FilePath 'cmd.exe' -ArgumentList '/c','npm install --no-fund --no-audit' -PassThru -Wait -NoNewWindow -WorkingDirectory 'D:\py\yumi\webui' -RedirectStandardOutput 'D:\py\yumi\scripts\npm-install.out' -RedirectStandardError 'D:\py\yumi\scripts\npm-install.err'
$sw.Stop()
Write-Host "npm install exited $($proc.ExitCode) in $($sw.Elapsed)" -ForegroundColor Cyan
if ($proc.ExitCode -ne 0) {
    Get-Content 'D:\py\yumi\scripts\npm-install.err' -Tail 30
    exit 1
}

Write-Host "=== webui: npm run build ===" -ForegroundColor Cyan
$sw = [Diagnostics.Stopwatch]::StartNew()
$proc = Start-Process -FilePath 'cmd.exe' -ArgumentList '/c','npm run build' -PassThru -Wait -NoNewWindow -WorkingDirectory 'D:\py\yumi\webui' -RedirectStandardOutput 'D:\py\yumi\scripts\npm-build.out' -RedirectStandardError 'D:\py\yumi\scripts\npm-build.err'
$sw.Stop()
Write-Host "npm run build exited $($proc.ExitCode) in $($sw.Elapsed)" -ForegroundColor Cyan
if ($proc.ExitCode -ne 0) {
    Get-Content 'D:\py\yumi\scripts\npm-build.err' -Tail 30
    exit 1
}

Write-Host "=== dist/ contents ===" -ForegroundColor Cyan
Get-ChildItem dist -Recurse | Where-Object { -not $_.PSIsContainer } | Select-Object -First 30 FullName, Length | Format-Table | Out-String | Write-Host