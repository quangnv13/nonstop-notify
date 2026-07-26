param(
    [Parameter(Mandatory = $true, Position = 0)]
    [ValidateSet('add', 'remove', 'self-check')]
    [string]$Action,

    [Parameter(Position = 1)]
    [string]$InstallDir
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$stateKeyPath = 'Software\dev.nonstop.notify'
$stateValueName = 'CliPathEntry'

Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;
using Microsoft.Win32.SafeHandles;

public static class NonstopNotifyRegistry
{
    private const int ErrorSuccess = 0;
    private const int ErrorFileNotFound = 2;
    private const int RegExpandSz = 2;

    [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern int RegQueryValueExW(
        SafeRegistryHandle key,
        string valueName,
        IntPtr reserved,
        out int type,
        byte[] data,
        ref int dataLength);

    [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern int RegSetValueExW(
        SafeRegistryHandle key,
        string valueName,
        int reserved,
        int type,
        byte[] data,
        int dataLength);

    public static string ReadExpandString(SafeRegistryHandle key, string valueName)
    {
        int type;
        int dataLength = 0;
        int result = RegQueryValueExW(key, valueName, IntPtr.Zero, out type, null, ref dataLength);
        if (result == ErrorFileNotFound)
        {
            return string.Empty;
        }
        if (result != ErrorSuccess)
        {
            throw new Win32Exception(result);
        }

        byte[] data = new byte[dataLength];
        result = RegQueryValueExW(key, valueName, IntPtr.Zero, out type, data, ref dataLength);
        if (result != ErrorSuccess)
        {
            throw new Win32Exception(result);
        }
        if (type != RegExpandSz)
        {
            throw new InvalidOperationException("HKCU Environment Path must be REG_EXPAND_SZ.");
        }
        return Encoding.Unicode.GetString(data, 0, dataLength).TrimEnd('\0');
    }

    public static void WriteExpandString(SafeRegistryHandle key, string valueName, string value)
    {
        byte[] data = Encoding.Unicode.GetBytes(value + "\0");
        int result = RegSetValueExW(key, valueName, 0, RegExpandSz, data, data.Length);
        if (result != ErrorSuccess)
        {
            throw new Win32Exception(result);
        }
    }
}
'@

function Normalize-PathEntry([string]$Value) {
    if ($Value.Length -gt 3) {
        return $Value.TrimEnd('\', '/')
    }
    return $Value
}

function Test-SamePathEntry([string]$Left, [string]$Right) {
    return [string]::Equals(
        (Normalize-PathEntry $Left),
        (Normalize-PathEntry $Right),
        [StringComparison]::OrdinalIgnoreCase
    )
}

function Add-PathEntry([string]$PathValue, [string]$Entry) {
    if ([string]::IsNullOrWhiteSpace($PathValue)) {
        return $Entry
    }
    if (($PathValue -split ';').Where({ Test-SamePathEntry $_ $Entry }).Count -gt 0) {
        return $PathValue
    }
    return "$PathValue;$Entry"
}

function Remove-PathEntry([string]$PathValue, [string]$Entry) {
    if ([string]::IsNullOrEmpty($PathValue)) {
        return $PathValue
    }
    return (($PathValue -split ';').Where({ -not (Test-SamePathEntry $_ $Entry) }) -join ';')
}

function Get-UserPath {
    $environmentKey = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $false)
    if ($null -eq $environmentKey) {
        return ''
    }
    try {
        return [NonstopNotifyRegistry]::ReadExpandString($environmentKey.Handle, 'Path')
    } finally {
        $environmentKey.Dispose()
    }
}

function Set-UserPath([string]$Value) {
    $environmentKey = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey('Environment')
    try {
        if ([string]::IsNullOrEmpty($Value)) {
            $environmentKey.DeleteValue('Path', $false)
        } else {
            [NonstopNotifyRegistry]::WriteExpandString($environmentKey.Handle, 'Path', $Value)
        }
    } finally {
        $environmentKey.Dispose()
    }
}

if ($Action -eq 'self-check') {
    $longPath = ((0..80).ForEach({ "C:\Tools\Segment$_" })) -join ';'
    if ($longPath.Length -le 1024) { throw 'self-check PATH must exceed 1024 characters' }
    $entry = 'C:\Users\Example\AppData\Local\Nonstop Notify'
    $added = Add-PathEntry $longPath $entry
    if (-not $added.EndsWith(";$entry", [StringComparison]::OrdinalIgnoreCase)) { throw 'add failed' }
    if ((Add-PathEntry $added ($entry + '\')) -ne $added) { throw 'duplicate detection failed' }
    if ((Remove-PathEntry $added ($entry.ToUpperInvariant())) -ne $longPath) { throw 'remove failed' }
    $trailingSeparatorPath = "$longPath;"
    if ((Remove-PathEntry (Add-PathEntry $trailingSeparatorPath $entry) $entry) -ne $trailingSeparatorPath) {
        throw 'trailing separator preservation failed'
    }
    $stateKey = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey($stateKeyPath)
    $testValueName = "PathIntegrationSelfCheck-$PID"
    try {
        [NonstopNotifyRegistry]::WriteExpandString($stateKey.Handle, $testValueName, $longPath)
        if ([NonstopNotifyRegistry]::ReadExpandString($stateKey.Handle, $testValueName) -ne $longPath) {
            throw 'long registry value round-trip failed'
        }
    } finally {
        $stateKey.DeleteValue($testValueName, $false)
        $stateKey.Dispose()
    }
    Write-Output 'PATH integration self-check ok'
    exit 0
}

if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    throw 'InstallDir is required.'
}

if ($Action -eq 'add') {
    $current = Get-UserPath
    $updated = Add-PathEntry $current $InstallDir
    if ($updated -ne $current) {
        Set-UserPath $updated
        $stateKey = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey($stateKeyPath)
        try {
            $stateKey.SetValue($stateValueName, $InstallDir, [Microsoft.Win32.RegistryValueKind]::String)
        } finally {
            $stateKey.Dispose()
        }
    }
    exit 0
}

$stateKey = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($stateKeyPath, $true)
if ($null -eq $stateKey) {
    exit 0
}

try {
    $ownedEntry = [string]$stateKey.GetValue($stateValueName, '')
    if (-not [string]::IsNullOrEmpty($ownedEntry)) {
        $current = Get-UserPath
        $updated = Remove-PathEntry $current $ownedEntry
        if ($updated -ne $current) {
            Set-UserPath $updated
        }
        $stateKey.DeleteValue($stateValueName, $false)
    }
} finally {
    $stateKey.Dispose()
}
