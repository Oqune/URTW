@echo off
setlocal
chcp 65001 >nul
"%~dp0URT.exe" %*
if errorlevel 1 pause
