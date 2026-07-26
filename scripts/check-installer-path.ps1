$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$config = Get-Content (Join-Path $root 'tauri.conf.json') -Raw | ConvertFrom-Json
$nsis = $config.bundle.windows.nsis

if ($nsis.installMode -ne 'currentUser') {
    throw 'NSIS must use currentUser install mode.'
}

if (-not $nsis.installerHooks) {
    throw 'NSIS installerHooks is required for CLI PATH integration.'
}

$hookPath = Join-Path $root $nsis.installerHooks
if (-not (Test-Path $hookPath -PathType Leaf)) {
    throw "NSIS installer hook not found: $hookPath"
}

$hook = Get-Content $hookPath -Raw
$pathScript = Join-Path $root 'windows/path-integration.ps1'
if (-not (Test-Path $pathScript -PathType Leaf)) {
    throw "PATH integration script not found: $pathScript"
}

$windowsPowerShell = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
& $windowsPowerShell -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $pathScript self-check
if ($LASTEXITCODE -ne 0) {
    throw 'PATH integration self-check failed under Windows PowerShell 5.1.'
}
$pathScriptSource = Get-Content $pathScript -Raw
if ($pathScriptSource.Contains('[Environment]::SetEnvironmentVariable')) {
    throw 'Windows PowerShell Environment API truncates long user PATH values.'
}
if (-not $pathScriptSource.Contains('RegSetValueExW')) {
    throw 'PATH integration must bypass Windows PowerShell 5.1 registry truncation.'
}

$windowsConfig = Get-Content (Join-Path $root 'tauri.windows.conf.json') -Raw | ConvertFrom-Json
if ($windowsConfig.bundle.resources.'windows/path-integration.ps1' -ne 'path-integration.ps1') {
    throw 'Windows bundle must install path-integration.ps1 beside the executable.'
}

$requiredPatterns = @(
    'NSIS_HOOK_POSTINSTALL',
    'NSIS_HOOK_PREUNINSTALL',
    'path-integration.ps1',
    'nsExec::ExecToStack',
    'WM_SETTINGCHANGE',
    'STR:Environment'
)

foreach ($pattern in $requiredPatterns) {
    if (-not $hook.Contains($pattern)) {
        throw "NSIS installer hook missing: $pattern"
    }
}

$workflow = Get-Content (Join-Path $root '.github/workflows/build.yml') -Raw
if (-not $workflow.Contains('scripts/check-installer-path.ps1')) {
    throw 'Windows CI must run scripts/check-installer-path.ps1.'
}
if (-not $workflow.Contains('scripts/smoke-test-windows-installer.ps1')) {
    throw 'Windows CI must smoke test the installed CLI and PATH cleanup.'
}

Write-Output 'installer PATH integration check ok'
