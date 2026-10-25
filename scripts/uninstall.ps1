param([string]$Target, [int]$ParentPid)
$ErrorActionPreference = 'Stop'
$log = Join-Path ([IO.Path]::GetTempPath()) 'x-clipboard-uninstall-result.txt'
try {
    $parent = Get-Process -Id $ParentPid -ErrorAction SilentlyContinue
    if ($parent -and -not $parent.WaitForExit(120000)) { throw 'Uninstaller did not exit' }
    $root = [IO.Path]::GetFullPath($Target).TrimEnd('\')
    $files = Get-Content -LiteralPath (Join-Path $root 'installation.json') -Raw -Encoding UTF8 | ConvertFrom-Json
    $files += @('installation.json', '.installation.lock')
    # Validate the complete deletion list first; preserve user data and unrelated files.
    foreach ($relative in $files) {
        $allowed = $relative -in @('x-clipboard.exe','pdfium.dll','uninstall.exe','installation.json','.installation.lock','README.txt','THIRD-PARTY.md','SHA256SUMS.txt','build-info.json') -or $relative.StartsWith('licenses/')
        $path = [IO.Path]::GetFullPath((Join-Path $root $relative))
        if (-not $allowed -or -not $path.StartsWith($root + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe uninstall manifest' }
        $ancestor = $path
        while ($ancestor) {
            if ((Test-Path -LiteralPath $ancestor) -and ((Get-Item -LiteralPath $ancestor -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Uninstall path contains a reparse point' }
            $ancestor = [IO.Path]::GetDirectoryName($ancestor)
        }
    }
    foreach ($relative in $files) {
        $path = Join-Path $root $relative
        if (Test-Path -LiteralPath $path -PathType Leaf) { Remove-Item -LiteralPath $path -Force }
    }
    $shell = New-Object -ComObject WScript.Shell
    foreach ($folder in @([Environment]::GetFolderPath('Desktop'), [Environment]::GetFolderPath('Programs'))) {
        $path = Join-Path $folder 'X Clipboard.lnk'
        if ((Test-Path -LiteralPath $path) -and $shell.CreateShortcut($path).TargetPath -eq (Join-Path $root 'x-clipboard.exe')) {
            Remove-Item -LiteralPath $path
        }
    }
    $key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\XClipboard'
    if ((Test-Path $key) -and [IO.Path]::GetFullPath((Get-ItemProperty $key).InstallLocation).TrimEnd('\') -eq $root) { Remove-Item -Path $key }
    # Empty application directories may remain. Never recursively delete an installation folder.
    Set-Content -LiteralPath $log -Value 'Uninstalled. Clipboard history and keys were preserved.' -Encoding UTF8
    if ($PSCommandPath.StartsWith([IO.Path]::GetTempPath(), [StringComparison]::OrdinalIgnoreCase)) { Remove-Item -LiteralPath $PSCommandPath }
} catch {
    Set-Content -LiteralPath $log -Value $_.Exception.Message -Encoding UTF8
    Add-Type -AssemblyName PresentationFramework
    [System.Windows.MessageBox]::Show("卸载未完成：$($_.Exception.Message)`n请先从托盘退出 X Clipboard。", 'X Clipboard') | Out-Null
    exit 1
}
