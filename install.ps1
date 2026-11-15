# ==============================================================================
# Open AG Patcher (Rust Edition) - Universal Windows Installer
# Installs 'open-ag-patcher.exe' and registers 'agyptch' command in PATH.
# ==============================================================================

[CmdletBinding()]
param(
    [Parameter(Position = 0, ValueFromRemainingArguments = $true)]
    [string[]]$RemainingArgs
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

# --- Terminal Styling (Material Dark Colors & VT100 Emulation) ---
try {
    [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
} catch {}

$Esc = [char]27
$ColorReset   = "$Esc[0m"
$ColorBold    = "$Esc[1m"
$ColorDim     = "$Esc[2m"
$ColorBlue    = "$Esc[38;2;66;133;244m"     # Google Blue
$ColorGreen   = "$Esc[38;2;52;168;83m"     # Google Green
$ColorYellow  = "$Esc[38;2;251;188;5m"    # Google Yellow
$ColorRed     = "$Esc[38;2;234;67;53m"     # Google Red
$ColorGray    = "$Esc[38;2;154;160;166m"   # Material Gray
$ColorSurface = "$Esc[38;2;248;249;250m"  # Off-white

function Write-StepAnimated {
    param(
        [string]$Label,
        [scriptblock]$Action
    )

    $spinChars = @('⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏')
    for ($k = 0; $k -lt 4; $k++) {
        $c = $spinChars[$k]
        Write-Host -NoNewline "`r  $ColorBlue$c$ColorReset  $($Label.PadRight(48))"
        Start-Sleep -Milliseconds 45
    }

    try {
        & $Action
        Write-Host "`r  $ColorGreen[+]$ColorReset $ColorBold$($Label.PadRight(48))$ColorReset $ColorGray[Готово]$ColorReset"
    } catch {
        Write-Host "`r  $ColorRed[-]$ColorReset $ColorBold$($Label.PadRight(48))$ColorReset $ColorRed[Ошибка]$ColorReset"
        Write-Host "$ColorDim$_$ColorReset"
        throw $_
    }
}

function Broadcast-EnvironmentChange {
    # Only broadcast on Windows systems
    $isWin = ($env:OS -eq "Windows_NT") -or ($PSVersionTable.PSVersion.Major -le 5) -or ($IsWindows -eq $true)
    if (-not $isWin) { return }

    try {
        if (-not ("Win32Native.Win32EnvironmentBroadcaster" -as [type])) {
            Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

namespace Win32Native {
    public static class Win32EnvironmentBroadcaster {
        [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Auto)]
        public static extern IntPtr SendMessageTimeout(
            IntPtr hWnd,
            uint Msg,
            UIntPtr wParam,
            string lParam,
            uint fuFlags,
            uint uTimeout,
            out UIntPtr lpdwResult
        );

        public static void NotifyEnvironmentChange() {
            try {
                IntPtr HWND_BROADCAST = new IntPtr(0xffff);
                uint WM_SETTINGCHANGE = 0x001A;
                uint SMTO_ABORTIFHUNG = 0x0002;
                UIntPtr result;
                SendMessageTimeout(
                    HWND_BROADCAST,
                    WM_SETTINGCHANGE,
                    UIntPtr.Zero,
                    "Environment",
                    SMTO_ABORTIFHUNG,
                    5000,
                    out result
                );
            } catch {}
        }
    }
}
"@ -ErrorAction Stop
        }
        [Win32Native.Win32EnvironmentBroadcaster]::NotifyEnvironmentChange()
    } catch {
        # Fallback / ignore if P/Invoke compilation is blocked
    }
}

# --- Header Banner ---
Write-Host ""
Write-Host "$ColorBlue┌────────────────────────────────────────────────────────────┐$ColorReset"
Write-Host "$ColorBlue│$ColorReset   $ColorBold$ColorSurface Open AG Patcher (Rust Edition)$ColorReset                           $ColorBlue│$ColorReset"
Write-Host "$ColorBlue│$ColorReset   $ColorDim Универсальный установщик бинарника и команды 'agyptch'$ColorReset   $ColorBlue│$ColorReset"
Write-Host "$ColorBlue└────────────────────────────────────────────────────────────┘$ColorReset"
Write-Host ""

# --- Step 1: Detect Architecture ---
$is64Bit = [Environment]::Is64BitOperatingSystem
$rawArch = $env:PROCESSOR_ARCHITECTURE
$arch = "x86_64"
if ($rawArch -eq "ARM64" -or $env:PROCESSOR_ARCHITEW6432 -eq "ARM64") {
    $arch = "arm64"
} elseif (-not $is64Bit) {
    $arch = "x86"
}
Write-Host "  $ColorGreen[+]$ColorReset Система определена: $ColorBold Windows$ColorReset ($arch)"

# --- Step 2: Target Directory Setup ---
$installDir = Join-Path $env:LOCALAPPDATA "Programs\open-ag-patcher"
if (-not (Test-Path $installDir)) {
    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
}

$targetExe = Join-Path $installDir "agyptch.exe"
$targetCmd = Join-Path $installDir "agyptch.cmd"
$targetExeAlias = Join-Path $installDir "open-ag-patcher.exe"

# Script directory (only if executed from disk, not piped via iex)
$scriptDir = $PSScriptRoot
if (-not $scriptDir -and $MyInvocation.MyCommand.Path) {
    $scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path 2>$null
}

# --- Step 3: Install Executable ---
Write-StepAnimated "Установка исполняемого файла в $installDir" {
    # 1. Check local precompiled binary (if running from an existing local folder script)
    if ($scriptDir) {
        $localAgy = Join-Path $scriptDir "agyptch.exe"
        $localExe = Join-Path $scriptDir "open-ag-patcher.exe"
        $rustAgy = Join-Path $scriptDir "rust\target\release\agyptch.exe"
        $rustTargetExe = Join-Path $scriptDir "rust\target\release\open-ag-patcher.exe"

        if (Test-Path $localAgy) {
            Copy-Item -Path $localAgy -Destination $targetExe -Force
            return
        } elseif (Test-Path $localExe) {
            Copy-Item -Path $localExe -Destination $targetExe -Force
            return
        } elseif (Test-Path $rustAgy) {
            Copy-Item -Path $rustAgy -Destination $targetExe -Force
            return
        } elseif (Test-Path $rustTargetExe) {
            Copy-Item -Path $rustTargetExe -Destination $targetExe -Force
            return
        }
    }

    # 2. If cargo is present in PATH or ~/.cargo/bin, build release binary
    $cargoCmd = Get-Command "cargo" -ErrorAction SilentlyContinue
    if (-not $cargoCmd -and $env:USERPROFILE) {
        $userCargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
        if (Test-Path $userCargo) {
            $cargoCmd = $userCargo
        }
    }
    if ($cargoCmd) {
        $rustDir = Join-Path $scriptDir "rust"
        if (Test-Path $rustDir) {
            Push-Location $rustDir
            if ($cargoCmd -is [System.Management.Automation.CommandInfo]) {
                & $cargoCmd.Definition build --release --quiet
            } else {
                & "$cargoCmd" build --release --quiet
            }
            Pop-Location
            if (Test-Path $rustAgy) {
                Copy-Item -Path $rustAgy -Destination $targetExe -Force
                return
            } elseif (Test-Path $rustTargetExe) {
                Copy-Item -Path $rustTargetExe -Destination $targetExe -Force
                return
            }
        }
    }

    # 3. Fallback: Download prebuilt binary from GitHub Releases
    $downloadUrl = "https://github.com/Milordick/rust-antigravity-patcher/releases/latest/download/agyptch-windows-$arch.exe"
    $checksumsUrl = "https://github.com/Milordick/rust-antigravity-patcher/releases/latest/download/checksums.txt"
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    $tempExe = "$targetExe.download"
    $usedUrl = $downloadUrl

    try {
        Invoke-WebRequest -Uri $downloadUrl -OutFile $tempExe -UseBasicParsing -TimeoutSec 30
    } catch {
        # Try proxy mirror or fallback to x86_64
        $mirrorUrl = "https://mirror.ghproxy.com/$downloadUrl"
        try {
            Invoke-WebRequest -Uri $mirrorUrl -OutFile $tempExe -UseBasicParsing -TimeoutSec 30
        } catch {
            $usedUrl = "https://github.com/Milordick/rust-antigravity-patcher/releases/latest/download/agyptch-windows-x86_64.exe"
            Invoke-WebRequest -Uri $usedUrl -OutFile $tempExe -UseBasicParsing -TimeoutSec 30
        }
    }

    # Verify SHA-256 checksum if checksums.txt is available
    $tempChecksums = Join-Path $env:TEMP "agy_checksums_$pid.txt"
    try {
        Invoke-WebRequest -Uri $checksumsUrl -OutFile $tempChecksums -UseBasicParsing -ErrorAction SilentlyContinue
        if (Test-Path $tempChecksums) {
            $expectedFileName = [System.IO.Path]::GetFileName($usedUrl)
            $actualHash = (Get-FileHash -Path $tempExe -Algorithm SHA256).Hash.ToLower()
            $lines = Get-Content $tempChecksums
            foreach ($line in $lines) {
                if ($line -match "([a-fA-F0-9]{64})\s+\*?(.+)") {
                    $hash = $matches[1].ToLower()
                    $file = $matches[2].Trim()
                    if ($file -eq $expectedFileName) {
                        if ($hash -ne $actualHash) {
                            Remove-Item -Path $tempExe -Force -ErrorAction SilentlyContinue
                            Remove-Item -Path $tempChecksums -Force -ErrorAction SilentlyContinue
                            throw "Не совпала контрольная сумма SHA-256 для $expectedFileName"
                        }
                        break
                    }
                }
            }
        }
    } catch {
        if ($_.Exception.Message -like "*Не совпала контрольная сумма*") {
            throw $_
        }
    } finally {
        if (Test-Path $tempChecksums) {
            Remove-Item -Path $tempChecksums -Force -ErrorAction SilentlyContinue
        }
    }

    Move-Item -Path $tempExe -Destination $targetExe -Force
}

# --- Step 4: Create 'agyptch' Command Wrappers ---
Write-StepAnimated "Создание команды 'agyptch' ($targetCmd)" {
    # 1. Create agyptch.cmd wrapper for CMD & PowerShell
    $cmdContent = "@echo off`r`n`"%~dp0agyptch.exe`" %*`r`n"
    [System.IO.File]::WriteAllText($targetCmd, $cmdContent, [System.Text.Encoding]::ASCII)

    # 2. Also hard-link or copy to open-ag-patcher.exe alias for direct execution
    try {
        Copy-Item -Path $targetExe -Destination $targetExeAlias -Force
    } catch {
        # ignore if file is currently open
    }
}

# --- Step 5: Configure User PATH Environment ---
Write-StepAnimated "Регистрация $installDir в системном окружении PATH" {
    $normalizedInstall = $installDir.Trim().Trim('"').TrimEnd('\', '/')

    # 1. Access user environment directly from registry to avoid REG_EXPAND_SZ corruption
    $regKey = $null
    $rawUserPath = ""
    $valueKind = [Microsoft.Win32.RegistryValueKind]::ExpandString

    try {
        $regKey = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)
        if ($null -eq $regKey) {
            $regKey = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey('Environment')
        }

        # Read unexpanded raw string (preserves %USERPROFILE%, %LOCALAPPDATA%, etc.)
        $rawVal = $regKey.GetValue("Path", "", [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
        if ($null -ne $rawVal) {
            $rawUserPath = [string]$rawVal
        }

        # Check existing value kind if present, ensuring ExpandString is preserved
        try {
            $existingKind = $regKey.GetValueKind("Path")
            if ($existingKind -eq [Microsoft.Win32.RegistryValueKind]::String -or $existingKind -eq [Microsoft.Win32.RegistryValueKind]::ExpandString) {
                $valueKind = [Microsoft.Win32.RegistryValueKind]::ExpandString
            } else {
                $valueKind = $existingKind
            }
        } catch {
            $valueKind = [Microsoft.Win32.RegistryValueKind]::ExpandString
        }

        # 2. Check if already present with case-insensitive and trailing backslash normalization
        $paths = $rawUserPath.Split(';', [System.StringSplitOptions]::RemoveEmptyEntries)
        $exists = $false
        foreach ($p in $paths) {
            $cleanP = $p.Trim().Trim('"').TrimEnd('\', '/')
            if ([string]::Equals($cleanP, $normalizedInstall, [System.StringComparison]::OrdinalIgnoreCase)) {
                $exists = $true
                break
            }
            # Also compare against expanded environment variables in existing entries
            try {
                $expandedP = [Environment]::ExpandEnvironmentVariables($cleanP).TrimEnd('\', '/')
                if ([string]::Equals($expandedP, $normalizedInstall, [System.StringComparison]::OrdinalIgnoreCase)) {
                    $exists = $true
                    break
                }
            } catch {}
        }

        # 3. Add to User PATH if not present
        if (-not $exists) {
            $cleanUserPath = $rawUserPath.Trim().TrimEnd(';')
            $newUserPath = if ([string]::IsNullOrEmpty($cleanUserPath)) {
                $installDir
            } else {
                "$cleanUserPath;$installDir"
            }
            $regKey.SetValue("Path", $newUserPath, $valueKind)
        }
    } finally {
        if ($null -ne $regKey) {
            $regKey.Close()
        }
    }

    # 4. Broadcast WM_SETTINGCHANGE notification to all top-level windows
    Broadcast-EnvironmentChange

    # 5. Update current session PATH so agyptch works immediately
    $currentPaths = $env:Path.Split(';', [System.StringSplitOptions]::RemoveEmptyEntries)
    $inCurrent = $false
    foreach ($p in $currentPaths) {
        $cleanP = $p.Trim().Trim('"').TrimEnd('\', '/')
        if ([string]::Equals($cleanP, $normalizedInstall, [System.StringComparison]::OrdinalIgnoreCase)) {
            $inCurrent = $true
            break
        }
    }
    if (-not $inCurrent) {
        $env:Path = "$installDir;$env:Path"
    }
}

# --- Step 6: Verify Installation ---
Write-StepAnimated "Проверка целостности и готовности утилиты" {
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $targetExe
    $psi.Arguments = "--version"
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.CreateNoWindow = $true

    $p = [System.Diagnostics.Process]::Start($psi)
    $p.WaitForExit(5000)
    if ($p.ExitCode -ne 0) {
        throw "Self-test failed with code $($p.ExitCode)"
    }
}

# --- Final Summary Card ---
Write-Host ""
Write-Host "$ColorGreen╭────────────────────────────────────────────────────────────╮$ColorReset"
Write-Host "$ColorGreen│$ColorReset   $ColorBold Установка успешно завершена!                             $ColorReset$ColorGreen│$ColorReset"
Write-Host "$ColorGreen├────────────────────────────────────────────────────────────┤$ColorReset"
Write-Host "$ColorGreen│$ColorReset   Версия:    $ColorSurface Open AG Patcher v1.4.3 (Rust Edition)$ColorReset"
Write-Host "$ColorGreen│$ColorReset   Команда:   $ColorBold$ColorBlue agyptch$ColorReset $ColorDim(или open-ag-patcher)$ColorReset"
Write-Host "$ColorGreen│$ColorReset   Путь:      $ColorGray $installDir\agyptch.cmd$ColorReset"
Write-Host "$ColorGreen├────────────────────────────────────────────────────────────┤$ColorReset"
Write-Host "$ColorGreen│$ColorReset   $ColorBold Быстрый старт:$ColorReset                                           $ColorGreen│$ColorReset"
Write-Host "$ColorGreen│$ColorReset     $ColorBlue agyptch$ColorReset              Запуск графического интерфейса (TUI)$ColorGreen│$ColorReset"
Write-Host "$ColorGreen│$ColorReset     $ColorBlue agyptch --check$ColorReset      Быстрая проверка статуса компонентов$ColorGreen│$ColorReset"
Write-Host "$ColorGreen│$ColorReset     $ColorBlue agyptch --patch-all$ColorReset  Пропатчить всё одной командой       $ColorGreen│$ColorReset"
Write-Host "$ColorGreen╰────────────────────────────────────────────────────────────╯$ColorReset"
Write-Host ""
Write-Host "$ColorGray Подсказка: окружение PATH обновлено в реестре и оповещение разослано в систему (WM_SETTINGCHANGE).$ColorReset"
Write-Host "$ColorGray Новые окна PowerShell и CMD подхватят команду 'agyptch' автоматически.$ColorReset"
Write-Host ""
