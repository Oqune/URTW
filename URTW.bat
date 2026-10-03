@echo off
setlocal
chcp 65001 >nul
"%~dp0URTW.exe" %*
if errorlevel 1 pause
