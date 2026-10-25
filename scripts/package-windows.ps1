#requires -Version 7.2
param(
    [string]$Upx = 'D:\devTools\upx\upx.exe',
    [switch]$SkipBuild,
    [switch]$NoUpx
)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
Push-Location $workspace
try {
    if (-not $SkipBuild) {
        & cargo build --locked --release --bin x-clipboard
        if ($LASTEXITCODE -ne 0) { throw 'Release build failed' }
    }
    if (-not $NoUpx -and -not (Test-Path -LiteralPath $Upx)) { throw "UPX not found: $Upx" }
    $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
    $variant = if ($NoUpx) { 'release' } else { 'upx' }
    $stage = Join-Path $workspace "dist\x-clipboard-0.1.0-windows-x64-$variant-$stamp"
    New-Item -ItemType Directory -Path $stage | Out-Null
    $source = Join-Path $workspace 'target\release\x-clipboard.exe'
    Copy-Item -LiteralPath $source -Destination $stage
    $packed = Join-Path $stage 'x-clipboard.exe'
    $originalHash = (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash
    $originalSize = (Get-Item -LiteralPath $source).Length
    if (-not $NoUpx) {
        & $Upx --best --lzma $packed
        if ($LASTEXITCODE -ne 0) { throw 'UPX compression failed' }
        & $Upx -t $packed
        if ($LASTEXITCODE -ne 0) { throw 'UPX integrity check failed' }
    }
    if ((Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash -ne $originalHash) {
        throw 'Original Release EXE was modified'
    }
    $runtime = Join-Path $workspace 'dist\windows-x64-preview'
    Copy-Item -LiteralPath (Join-Path $runtime 'pdfium.dll') -Destination $stage
    Copy-Item -LiteralPath (Join-Path $runtime 'licenses') -Destination $stage -Recurse
    foreach ($document in @('THIRD-PARTY.md', 'WINDOW-FIXES.md', 'MEMORY-AND-PACKAGING.md')) {
        Copy-Item -LiteralPath (Join-Path $workspace "docs\$document") -Destination $stage
    }
    @'
X Clipboard Windows x64 portable Release

Run x-clipboard.exe. Keep pdfium.dll beside the executable for PDF previews.
Settings and encrypted history use the normal per-user application data directory.
See build-info.json for EXE compression. PDFium is distributed unchanged.
No user history, database keys, or test fixtures are included.
This is a local build, not an installer or remotely published release.
'@ | Set-Content -LiteralPath (Join-Path $stage 'README.txt') -Encoding utf8
    [ordered]@{
        built_at = (Get-Date).ToString('o')
        profile = 'release; opt-level=s; thin LTO; codegen-units=1; stripped'
        compression = $(if ($NoUpx) { 'none' } else { 'UPX --best --lzma; upx -t passed' })
        original_exe_bytes = $originalSize
        packed_exe_bytes = (Get-Item -LiteralPath $packed).Length
        original_exe_sha256 = $originalHash.ToLowerInvariant()
        packed_exe_sha256 = (Get-FileHash -LiteralPath $packed -Algorithm SHA256).Hash.ToLowerInvariant()
    } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $stage 'build-info.json') -Encoding utf8
    Get-ChildItem -LiteralPath $stage -File -Recurse | Sort-Object FullName | ForEach-Object {
        $relative = [IO.Path]::GetRelativePath($stage, $_.FullName).Replace('\', '/')
        '{0}  {1}' -f (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant(), $relative
    } | Set-Content -LiteralPath (Join-Path $stage 'SHA256SUMS.txt') -Encoding utf8
    $archive = "$stage.zip"
    Compress-Archive -LiteralPath $stage -DestinationPath $archive -CompressionLevel Optimal
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [IO.Compression.ZipFile]::OpenRead($archive)
    try {
        $entry = @($zip.Entries | Where-Object { $_.Name -eq 'x-clipboard.exe' })
        if ($entry.Count -ne 1) { throw 'Archive must contain exactly one application EXE' }
        $stream = $entry[0].Open()
        $hasher = [Security.Cryptography.SHA256]::Create()
        try { $zipHash = [Convert]::ToHexString($hasher.ComputeHash($stream)) }
        finally { $stream.Dispose(); $hasher.Dispose() }
        if ($zipHash -ne (Get-FileHash -LiteralPath $packed -Algorithm SHA256).Hash) { throw 'Archive EXE hash mismatch' }
    } finally { $zip.Dispose() }
    Write-Output "PACKAGE=$stage"
    Write-Output "ARCHIVE=$archive"
    Write-Output "RELEASE_EXE_BYTES=$originalSize"
    Write-Output "UPX_EXE_BYTES=$((Get-Item -LiteralPath $packed).Length)"
    Write-Output "ZIP_BYTES=$((Get-Item -LiteralPath $archive).Length)"
} finally { Pop-Location }
