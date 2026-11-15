@echo off
setlocal

echo Запуск установщика Open AG Patcher...
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0install.ps1" %*
set "EXIT_CODE=%ERRORLEVEL%"

if %EXIT_CODE% NEQ 0 (
    echo.
    echo [Ошибка] Установка прервана с кодом %EXIT_CODE%.
    if "%CI%"=="" if "%GITHUB_ACTIONS%"=="" pause
    exit /b %EXIT_CODE%
)

endlocal
exit /b 0

