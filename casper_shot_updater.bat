@echo off
title Casper Shot Updater & color 0A
cd /d "%~dp0"

taskkill /IM "casper_shot.exe" /F >nul 2>&1
timeout /t 2 /nobreak >nul

echo Downloading latest version...
curl -L -o "%TEMP%\CASPERShot-update.zip" "https://github.com/Nyaldee/CASPER-Shot/releases/latest/download/CASPER.Shot.Windows.zip" || (echo Download failed. & pause & exit /b 1)

echo Installing...
tar -xf "%TEMP%\CASPERShot-update.zip" -C .. --exclude="CASPER Shot/casper_shot_updater.bat" || (echo Extraction failed. & pause & exit /b 1)

del /q "%TEMP%\CASPERShot-update.zip"

if exist "casper_shot.exe" (
    start "" "casper_shot.exe"
) else (
    echo.
    echo Move this file into the "CASPER Shot" folder.
    pause
)
