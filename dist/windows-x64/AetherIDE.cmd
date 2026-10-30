@echo off
title AetherIDE - Autonomous AI Engineering Platform
echo Starting Aether Native Daemon...
start /B "" "%~dp0bin\aether-daemon.exe"
timeout /t 1 /nobreak >nul
echo Launching AetherIDE Desktop Workbench...
start "" code "%~dp0.." --extensionDevelopmentPath="%~dp0extensions\aether-core"
exit
