@echo off
echo Read README.txt first. Run this launcher as administrator.
"%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -ExecutionPolicy Bypass -File "%~dp0Setup-120Hz.ps1" -Install
pause
