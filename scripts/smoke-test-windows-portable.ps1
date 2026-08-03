param(
    [Parameter(Mandatory = $true)]
    [string]$BinaryPath
)

$ErrorActionPreference = 'Stop'

$binary = (Resolve-Path $BinaryPath).Path
if ([System.IO.Path]::GetExtension($binary) -ine '.exe') {
    throw "Expected a Windows executable: $binary"
}

$originalLocation = Get-Location
try {
    Set-Location ([System.IO.Path]::GetTempPath())
    & $binary --self-check
    if ($LASTEXITCODE -ne 0) {
        throw "Portable CLI self-check exit code $LASTEXITCODE."
    }
} finally {
    Set-Location $originalLocation
}

Write-Output 'Windows portable CLI smoke test ok'
