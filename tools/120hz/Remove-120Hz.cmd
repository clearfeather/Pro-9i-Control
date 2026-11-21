@echo off
echo This removes only the matching panel override. Run as administrator.
"%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -ExecutionPolicy Bypass -File "%~dp0Setup-120Hz.ps1" -Remove
pause
