#requires -Version 7.2
param(
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][string]$Output,
    [string]$Label = 'background',
    [switch]$ShowWindow,
    [string]$FixtureDirectory
)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
$runRoot = Join-Path $workspace ('test-data\memory-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $runRoot | Out-Null
if ($ShowWindow) {
    $seeder = Join-Path $workspace 'target\debug\examples\memory_fixture.exe'
    if (-not (Test-Path -LiteralPath $seeder)) { throw 'Build the memory_fixture example first (no-default-features)' }
    & $seeder $runRoot
    if ($LASTEXITCODE -ne 0) { throw 'Could not seed paused empty database' }
}
if (-not $ShowWindow) {
    if (-not $FixtureDirectory) { throw 'Background measurement requires an onboarded synthetic FixtureDirectory' }
    $marker = Get-Content -LiteralPath (Join-Path $FixtureDirectory 'UI-REVIEW-FIXTURE.json') -Raw | ConvertFrom-Json
    if (-not $marker.synthetic) { throw 'Only synthetic test fixtures are allowed' }
    $wal = Join-Path $FixtureDirectory 'clipboard.db-wal'
    if ((Test-Path -LiteralPath $wal) -and (Get-Item -LiteralPath $wal).Length -ne 0) { throw 'Checkpoint fixture database before copying' }
    foreach ($file in @('clipboard.db', 'clipboard-key.dat')) {
        Copy-Item -LiteralPath (Join-Path $FixtureDirectory $file) -Destination $runRoot
    }
}
$previous = $env:X_CLIPBOARD_DATA_DIR
$process = $null
try {
    $env:X_CLIPBOARD_DATA_DIR = $runRoot
    $options = @{
        FilePath = (Resolve-Path -LiteralPath $Executable).Path
        WorkingDirectory = Split-Path -Parent (Resolve-Path -LiteralPath $Executable).Path
        WindowStyle = 'Hidden'
        PassThru = $true
        RedirectStandardError = Join-Path $runRoot 'stderr.log'
        RedirectStandardOutput = Join-Path $runRoot 'stdout.log'
    }
    if (-not $ShowWindow) { $options.ArgumentList = '--background' }
    $process = Start-Process @options
    Start-Sleep -Seconds 5
    $samples = @(1..10 | ForEach-Object {
        $process.Refresh()
        if ($process.HasExited) { throw "Application exited early: $($process.ExitCode)" }
        [ordered]@{ working_set_bytes = $process.WorkingSet64; private_bytes = $process.PrivateMemorySize64 }
        Start-Sleep -Milliseconds 500
    })
    if (-not (Test-Path -LiteralPath (Join-Path $runRoot 'clipboard.db'))) { throw 'Encrypted database was not created' }
    if ($ShowWindow -and $process.MainWindowHandle -eq 0) { throw 'Application window was not created' }
    if (-not $ShowWindow -and $process.MainWindowHandle -ne 0) { throw 'Background measurement unexpectedly opened a window' }
    if ((Get-Item -LiteralPath (Join-Path $runRoot 'stderr.log')).Length -ne 0) { throw "Startup stderr is not empty; inspect $runRoot" }
    $working = @($samples.working_set_bytes | Sort-Object)
    $private = @($samples.private_bytes | Sort-Object)
    $result = [ordered]@{
        label = $Label
        executable = (Resolve-Path -LiteralPath $Executable).Path
        data_directory = $runRoot
        sha256 = (Get-FileHash -LiteralPath $Executable -Algorithm SHA256).Hash.ToLowerInvariant()
        scenario = "isolated encrypted database; capture paused; empty welcome or synthetic background fixture; welcome_window=$ShowWindow; 5s warmup, 10 samples at 500ms"
        working_set_median_bytes = ($working[4] + $working[5]) / 2
        private_median_bytes = ($private[4] + $private[5]) / 2
        samples = $samples
    }
    $result | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $Output -Encoding utf8
    [pscustomobject]$result | Select-Object label,working_set_median_bytes,private_median_bytes | Format-List
} finally {
    if ($process -and -not $process.HasExited) { Stop-Process -Id $process.Id; $process.WaitForExit() }
    $env:X_CLIPBOARD_DATA_DIR = $previous
}
