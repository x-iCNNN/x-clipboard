param([Parameter(Mandatory = $true)][string]$Plan)
$ErrorActionPreference = 'Stop'
$planData = Get-Content -LiteralPath $Plan -Raw -Encoding UTF8 | ConvertFrom-Json
$stage = [IO.Path]::GetFullPath($planData.stage)
$target = [IO.Path]::GetFullPath($planData.target)
$payload = Join-Path $stage 'payload'
$backup = Join-Path $stage 'backup'
$log = Join-Path $stage 'result.txt'
$changed = @()
$success = $false
try {
    # Resolve every path before doing any replacement. Never recursively delete the app directory.
    foreach ($relative in $planData.files) {
        $destination = [IO.Path]::GetFullPath((Join-Path $target $relative))
        if (-not $destination.StartsWith($target.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) {
            throw 'Update path is outside the application directory'
        }
        if (-not (Test-Path -LiteralPath (Join-Path $payload $relative) -PathType Leaf)) {
            throw "Missing staged file: $relative"
        }
        # Refuse junctions/symlinks which could redirect writes outside the app directory.
        $ancestor = $destination
        while ($ancestor) {
            if (Test-Path -LiteralPath $ancestor) {
                if ((Get-Item -LiteralPath $ancestor -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) {
                    throw "Update destination contains a reparse point: $ancestor"
                }
            }
            $ancestor = [IO.Path]::GetDirectoryName($ancestor)
        }
    }
    New-Item -ItemType Directory -Path $backup -Force | Out-Null
    Set-Content -LiteralPath (Join-Path $stage 'ready') -Value 'ready' -Encoding UTF8
    $parent = Get-Process -Id $planData.pid -ErrorAction SilentlyContinue
    if ($parent -and -not $parent.WaitForExit(120000)) { throw 'Application did not exit within 120 seconds' }
    foreach ($relative in $planData.files) {
        $destination = Join-Path $target $relative
        $saved = Join-Path $backup $relative
        $existed = Test-Path -LiteralPath $destination
        if ($existed) {
            New-Item -ItemType Directory -Path (Split-Path -Parent $saved) -Force | Out-Null
            Copy-Item -LiteralPath $destination -Destination $saved
        }
        $changed += [pscustomobject]@{ destination = $destination; saved = $saved; existed = $existed }
        New-Item -ItemType Directory -Path (Split-Path -Parent $destination) -Force | Out-Null
        Copy-Item -LiteralPath (Join-Path $payload $relative) -Destination $destination -Force
    }
    $success = $true
    Set-Content -LiteralPath $log -Value "Installed $($planData.version)" -Encoding UTF8
    $key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\XClipboard'
    try {
        if ((Test-Path $key) -and [IO.Path]::GetFullPath((Get-ItemProperty $key).InstallLocation) -eq $target) {
            Set-ItemProperty -Path $key -Name DisplayVersion -Value $planData.version.TrimStart('v')
        }
    } catch { Add-Content -LiteralPath $log -Value "Version registration warning: $($_.Exception.Message)" }
} catch {
    $failure = $_.Exception.Message
    [array]::Reverse($changed)
    foreach ($item in $changed) {
        try {
            if ($item.existed) { Copy-Item -LiteralPath $item.saved -Destination $item.destination -Force }
            elseif (Test-Path -LiteralPath $item.destination) { Remove-Item -LiteralPath $item.destination -Force }
        } catch { $failure += "`nRollback failed: $($_.Exception.Message). Backup: $backup" }
    }
    Set-Content -LiteralPath $log -Value "Update failed: $failure" -Encoding UTF8
}
if ($planData.restart -and (Test-Path -LiteralPath $planData.executable)) {
    try {
        Start-Process -FilePath $planData.executable -WorkingDirectory $target -WindowStyle Hidden
    } catch { Add-Content -LiteralPath $log -Value "Restart failed: $($_.Exception.Message)" }
}
if (-not $success) { exit 1 }
