@echo off
echo Read README.txt first. Windows will request administrator access if needed.
set "PanelPowerShell=%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe"
if exist "%SystemRoot%\Sysnative\WindowsPowerShell\v1.0\powershell.exe" set "PanelPowerShell=%SystemRoot%\Sysnative\WindowsPowerShell\v1.0\powershell.exe"
"%PanelPowerShell%" -NoProfile -ExecutionPolicy Bypass -File "%~dp0Installer-PanelModes.ps1" -Action Install
pause
