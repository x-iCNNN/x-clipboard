# Runs the actual Windows PowerShell helper against disposable application files.
$ErrorActionPreference = 'Stop'
$root = Join-Path ([IO.Path]::GetTempPath()) "x-clipboard-update-test-$([guid]::NewGuid())"
$helper = Join-Path $PSScriptRoot 'install-update.ps1'
$powershell = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
New-Item -ItemType Directory -Path $root | Out-Null
function Assert($condition, $message) { if (-not $condition) { throw $message } }
function Fixture($name, $files) {
    $directory = Join-Path $root $name
    $stage = Join-Path $directory 'stage'
    $target = Join-Path $directory 'app'
    New-Item -ItemType Directory -Path "$stage/payload", $target -Force | Out-Null
    foreach ($file in @('x-clipboard.exe', 'pdfium.dll')) {
        Set-Content -LiteralPath (Join-Path $target $file) -Value "old-$file"
        Set-Content -LiteralPath (Join-Path "$stage/payload" $file) -Value "new-$file"
    }
    Set-Content -LiteralPath "$target/user-data.txt" -Value 'preserve'
    $plan = [ordered]@{ version = 'v0.2.0'; stage = $stage; target = $target;
        executable = "$target/x-clipboard.exe"; files = $files; pid = 2147483647; restart = $false }
    $planPath = Join-Path $stage 'plan.json'
    $plan | ConvertTo-Json | Set-Content -LiteralPath $planPath -Encoding UTF8
    return @{ stage = $stage; target = $target; planPath = $planPath; plan = $plan }
}
$success = Fixture 'success' @('x-clipboard.exe', 'pdfium.dll')
& $powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $helper -Plan $success.planPath
Assert ($LASTEXITCODE -eq 0) 'Installer failed'
Assert ((Get-Content "$($success.target)/x-clipboard.exe") -eq 'new-x-clipboard.exe') 'EXE was not installed'
Assert ((Get-Content "$($success.target)/pdfium.dll") -eq 'new-pdfium.dll') 'Runtime was not installed'
Assert ((Get-Content "$($success.target)/user-data.txt") -eq 'preserve') 'User data changed'
Assert ((Get-Content "$($success.stage)/backup/x-clipboard.exe") -eq 'old-x-clipboard.exe') 'Backup missing'

$rollback = Fixture 'rollback' @('x-clipboard.exe', 'pdfium.dll')
$lock = [IO.File]::Open("$($rollback.target)/pdfium.dll", 'Open', 'Read', 'Read')
try {
    & $powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $helper -Plan $rollback.planPath
    Assert ($LASTEXITCODE -ne 0) 'Locked file must fail installation'
    Assert ((Get-Content "$($rollback.target)/x-clipboard.exe") -eq 'old-x-clipboard.exe') 'EXE rollback failed'
    Assert ((Get-Content "$($rollback.target)/pdfium.dll") -eq 'old-pdfium.dll') 'Locked runtime changed'
    Assert ((Get-Content "$($rollback.stage)/result.txt" -Raw) -match 'Update failed') 'Failure log missing'
} finally { $lock.Dispose() }

$unsafe = Fixture 'unsafe' @('../outside.txt')
& $powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $helper -Plan $unsafe.planPath
Assert ($LASTEXITCODE -ne 0) 'Unsafe target path must fail'
Assert (-not (Test-Path (Join-Path (Split-Path $unsafe.target -Parent) 'outside.txt'))) 'Unsafe file written'

$waiting = Fixture 'waiting' @('x-clipboard.exe', 'pdfium.dll')
$parent = Start-Process -FilePath $powershell -ArgumentList '-NoProfile -Command Start-Sleep -Seconds 5' -WindowStyle Hidden -PassThru
$waiting.plan.pid = $parent.Id
$waiting.plan | ConvertTo-Json | Set-Content -LiteralPath $waiting.planPath -Encoding UTF8
$installer = Start-Process -FilePath $powershell -ArgumentList @('-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', "`"$helper`"", '-Plan', "`"$($waiting.planPath)`"") -WindowStyle Hidden -PassThru
for ($attempt = 0; $attempt -lt 60 -and -not (Test-Path "$($waiting.stage)/ready"); $attempt++) { Start-Sleep -Milliseconds 50 }
Assert (Test-Path "$($waiting.stage)/ready") 'Helper handshake failed'
Assert ((Get-Content "$($waiting.target)/x-clipboard.exe") -eq 'old-x-clipboard.exe') 'Installer replaced a running application'
Assert ($installer.WaitForExit(15000)) 'Installer did not finish'
Assert ($installer.ExitCode -eq 0) 'Waiting installer failed'
Assert ((Get-Content "$($waiting.target)/x-clipboard.exe") -eq 'new-x-clipboard.exe') 'Post-exit install failed'
Write-Output "PASS: install, preserved user data, backup, locked-file rollback, path rejection, and parent-exit handshake. Fixtures: $root"
exit 0
