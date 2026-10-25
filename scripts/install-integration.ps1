param([string]$Plan, [string]$Version)
$ErrorActionPreference = 'Stop'
$options = Get-Content -LiteralPath $Plan -Raw -Encoding UTF8 | ConvertFrom-Json
$options.target = [IO.Path]::GetFullPath($options.target)
$exe = Join-Path $options.target 'x-clipboard.exe'
$shell = New-Object -ComObject WScript.Shell
foreach ($entry in @(
    @{ enabled = $options.desktop; folder = [Environment]::GetFolderPath('Desktop') },
    @{ enabled = $options.start_menu; folder = [Environment]::GetFolderPath('Programs') }
)) {
    if ($entry.enabled) {
        $shortcut = $shell.CreateShortcut((Join-Path $entry.folder 'X Clipboard.lnk'))
        $shortcut.TargetPath = $exe
        $shortcut.WorkingDirectory = $options.target
        $shortcut.IconLocation = "$exe,0"
        $shortcut.Description = 'X Clipboard - 本机加密剪贴板'
        $shortcut.Save()
    }
}
$key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\XClipboard'
New-Item -Path $key -Force | Out-Null
foreach ($pair in @{
    DisplayName = 'X Clipboard'; DisplayVersion = $Version; Publisher = 'X Clipboard';
    InstallLocation = $options.target; DisplayIcon = "$exe,0";
    UninstallString = '"' + (Join-Path $options.target 'uninstall.exe') + '" --uninstall';
    URLInfoAbout = 'https://github.com/x-iCNNN/x-clipboard'
}.GetEnumerator()) {
    New-ItemProperty -Path $key -Name $pair.Key -Value $pair.Value -PropertyType String -Force | Out-Null
}
New-ItemProperty -Path $key -Name NoModify -Value 1 -PropertyType DWord -Force | Out-Null
New-ItemProperty -Path $key -Name NoRepair -Value 1 -PropertyType DWord -Force | Out-Null
