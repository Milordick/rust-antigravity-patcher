# ==============================================================================
# Open AG Patcher (Rust Edition) - Universal Windows Installer
# Installs 'open-ag-patcher.exe' and registers 'agyptch' command in PATH.
# ==============================================================================

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

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
        Write-Host "`r  $ColorGreen✓$ColorReset  $ColorBold$($Label.PadRight(48))$ColorReset $ColorGray[Готово]$ColorReset"
    } catch {
        Write-Host "`r  $ColorRed✗$ColorReset  $ColorBold$($Label.PadRight(48))$ColorReset $ColorRed[Ошибка]$ColorReset"
        Write-Host "$ColorDim$_$ColorReset"
        throw $_
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
Write-Host "  $ColorGreen✓$ColorReset  Система определена: $ColorBold Windows$ColorReset ($arch)"

# --- Step 2: Target Directory Setup ---
$installDir = Join-Path $env:LOCALAPPDATA "Programs\open-ag-patcher"
if (-not (Test-Path $installDir)) {
    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
}

$targetExe = Join-Path $installDir "agyptch.exe"
$targetCmd = Join-Path $installDir "agyptch.cmd"
$targetExeAlias = Join-Path $installDir "open-ag-patcher.exe"

# Script directory
$scriptDir = $PSScriptRoot
if (-not $scriptDir) {
    $scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path 2>$null
}
if (-not $scriptDir) {
    $scriptDir = (Get-Location).Path
}

# --- Step 3: Install Executable ---
Write-StepAnimated "Установка исполняемого файла в $installDir" {
    # 1. Check local precompiled binary
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

    # 2. If cargo is present in PATH, build release binary
    $cargoCmd = Get-Command "cargo" -ErrorAction SilentlyContinue
    if ($cargoCmd) {
        $rustDir = Join-Path $scriptDir "rust"
        if (Test-Path $rustDir) {
            Push-Location $rustDir
            & cargo build --release --quiet
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
        Invoke-WebRequest -Uri $downloadUrl -OutFile $tempExe -UseBasicParsing
    } catch {
        # Fallback to x86_64 (Windows on ARM runs x64 via built-in Prism emulator)
        $usedUrl = "https://github.com/Milordick/rust-antigravity-patcher/releases/latest/download/agyptch-windows-x86_64.exe"
        Invoke-WebRequest -Uri $usedUrl -OutFile $tempExe -UseBasicParsing
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
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if (-not $userPath) { $userPath = "" }

    $paths = $userPath.Split(';', [System.StringSplitOptions]::RemoveEmptyEntries)
    $normalizedInstall = $installDir.TrimEnd('\')

    $exists = $false
    foreach ($p in $paths) {
        if ($p.TrimEnd('\') -eq $normalizedInstall) {
            $exists = $true
            break
        }
    }

    if (-not $exists) {
        $newUserPath = if ($userPath) { "$userPath;$installDir" } else { $installDir }
        [Environment]::SetEnvironmentVariable("Path", $newUserPath, "User")
    }

    # Update current session PATH so agyptch works immediately
    $env:Path = "$installDir;$env:Path"
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
Write-Host "$ColorGreen│$ColorReset   Версия:    $ColorSurface Open AG Patcher v1.4.0 (Rust Edition)$ColorReset"
Write-Host "$ColorGreen│$ColorReset   Команда:   $ColorBold$ColorBlue agyptch$ColorReset $ColorDim(или open-ag-patcher)$ColorReset"
Write-Host "$ColorGreen│$ColorReset   Путь:      $ColorGray $installDir\agyptch.cmd$ColorReset"
Write-Host "$ColorGreen├────────────────────────────────────────────────────────────┤$ColorReset"
Write-Host "$ColorGreen│$ColorReset   $ColorBold Быстрый старт:$ColorReset                                           $ColorGreen│$ColorReset"
Write-Host "$ColorGreen│$ColorReset     $ColorBlue agyptch$ColorReset              Запуск графического интерфейса (TUI)$ColorGreen│$ColorReset"
Write-Host "$ColorGreen│$ColorReset     $ColorBlue agyptch --check$ColorReset      Быстрая проверка статуса компонентов$ColorGreen│$ColorReset"
Write-Host "$ColorGreen│$ColorReset     $ColorBlue agyptch --patch-all$ColorReset  Пропатчить всё одной командой       $ColorGreen│$ColorReset"
Write-Host "$ColorGreen╰────────────────────────────────────────────────────────────╯$ColorReset"
Write-Host ""
Write-Host "$ColorGray Подсказка: откройте новое окно PowerShell / CMD для автоматического применения PATH.$ColorReset"
Write-Host ""
