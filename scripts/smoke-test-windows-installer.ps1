param(
    [Parameter(Mandatory = $true)]
    [string]$InstallerPath
)

$ErrorActionPreference = 'Stop'

function Get-RawUserPath {
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $false)
    if ($null -eq $key) {
        return ''
    }
    try {
        return [string]$key.GetValue(
            'Path',
            '',
            [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames
        )
    } finally {
        $key.Dispose()
    }
}

function Get-UninstallCommand {
    return Get-ChildItem 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall' |
        Get-ItemProperty |
        Where-Object DisplayName -eq 'Nonstop Notify' |
        Select-Object -First 1 -ExpandProperty UninstallString
}

$installer = (Resolve-Path $InstallerPath).Path
$beforePath = Get-RawUserPath
$installed = $false

try {
    $installProcess = Start-Process -FilePath $installer -ArgumentList '/S' -Wait -PassThru
    if ($installProcess.ExitCode -ne 0) {
        throw "Installer exit code $($installProcess.ExitCode)."
    }
    $installed = $true

    $installDir = (Get-ItemProperty 'HKCU:\Software\dev.nonstop.notify').CliPathEntry
    $installedPath = Get-RawUserPath
    $matches = @($installedPath -split ';' | Where-Object {
        $_.TrimEnd('\') -ieq $installDir.TrimEnd('\')
    })
    if ($matches.Count -ne 1) {
        throw "Expected one PATH entry for $installDir, found $($matches.Count)."
    }

    $env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' +
        [Environment]::ExpandEnvironmentVariables($installedPath)
    $command = Get-Command nonstop-notify -CommandType Application -ErrorAction Stop
    & $command.Source --self-check
    if ($LASTEXITCODE -ne 0) {
        throw "nonstop-notify --self-check exit code $LASTEXITCODE."
    }

    $uninstaller = Get-UninstallCommand
    if (-not $uninstaller) {
        throw 'Uninstall command not found.'
    }
    $uninstallProcess = Start-Process -FilePath $uninstaller.Trim('"') -ArgumentList '/S' -Wait -PassThru
    if ($uninstallProcess.ExitCode -ne 0) {
        throw "Uninstaller exit code $($uninstallProcess.ExitCode)."
    }
    $installed = $false

    if ((Get-RawUserPath) -ne $beforePath) {
        throw 'Uninstaller did not restore original user PATH exactly.'
    }
    $state = Get-ItemProperty 'HKCU:\Software\dev.nonstop.notify' -ErrorAction SilentlyContinue
    if ($state.CliPathEntry) {
        throw 'Uninstaller left CliPathEntry marker.'
    }
} finally {
    if ($installed) {
        $uninstaller = Get-UninstallCommand
        if ($uninstaller) {
            Start-Process -FilePath $uninstaller.Trim('"') -ArgumentList '/S' -Wait
        }
    }
}

Write-Output 'Windows installer CLI PATH smoke test ok'
