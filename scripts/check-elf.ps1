$ErrorActionPreference = 'Stop'

$zipPath = Get-ChildItem 'D:\py\yumi\output\*.zip' | Sort-Object LastWriteTime -Descending | Select-Object -First 1 -ExpandProperty FullName
Write-Host "Reading: $zipPath"

Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip = [System.IO.Compression.ZipFile]::OpenRead($zipPath)
foreach ($e in $zip.Entries) {
    $s = $e.Open()
    $buf = New-Object byte[] 64
    $read = $s.Read($buf, 0, 64)
    $s.Close()
    if ($read -lt 4) { continue }
    $magic = [BitConverter]::ToString($buf, 0, 4)
    Write-Host ("{0,-50} {1,10}  magic={2}" -f $e.FullName, $e.Length, $magic)
}
$zip.Dispose()

# Specifically ELF for core/bin/yumi
$zip = [System.IO.Compression.ZipFile]::OpenRead($zipPath)
$yumi = $zip.GetEntry('core\bin\yumi')
if (-not $yumi) { Write-Host "core/bin/yumi NOT FOUND"; $zip.Dispose(); exit 1 }
$s = $yumi.Open()
$buf = New-Object byte[] 64
$null = $s.Read($buf, 0, 64)
$s.Close()
$zip.Dispose()
Write-Host ""
Write-Host "=== core/bin/yumi ELF header ==="
Write-Host ("magic           : {0}" -f [System.Text.Encoding]::ASCII.GetString($buf, 0, 4))
Write-Host ("class           : {0} (1=32bit, 2=64bit)" -f $buf[4])
Write-Host ("endianness      : {0} (1=little, 2=big)" -f $buf[5])
Write-Host ("type            : {0} (2=ET_EXEC, 3=ET_DYN)" -f [BitConverter]::ToUInt16($buf, 16))
Write-Host ("machine (e_machine): 0x{0:x} (0xb7 = EM_AARCH64)" -f [BitConverter]::ToUInt16($buf, 18))
Write-Host ("size            : {0} bytes" -f $yumi.Length)