@echo off
setlocal
echo Запуск установщика Open AG Patcher...
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0install.ps1"
if %ERRORLEVEL% NEQ 0 (
    echo.
    echo [Ошибка] Установка прервана.
    pause
    exit /b %ERRORLEVEL%
)
endlocal
