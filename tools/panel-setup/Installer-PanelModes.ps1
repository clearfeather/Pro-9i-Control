#Requires -Version 5.1
[CmdletBinding()]
param([ValidateSet('Probe','Install','Apply')][string]$Action='Probe')
$ErrorActionPreference='Stop'

# Exit codes shared with the installer: 0=offer setup, 10=already active,
# 20=unavailable or an explained setup failure, 1223=UAC cancelled,
# 3010=installed/pending restart, 1=launcher failure.
function Get-PanelInstallerStatus($Info) {
    if($Info.supported-ne $true){throw 'This laptop does not support the optional panel modes.'}
    if($Info.includes48Hz-eq $true) {
        if($Info.restartRequired-eq $true){return 3010}
        return 10
    }
    return 0
}

function Get-PanelPowerShell {
    $directory=if([Environment]::Is64BitOperatingSystem-and ![Environment]::Is64BitProcess){'Sysnative'}else{'System32'}
    return Join-Path ([Environment]::GetFolderPath('Windows')) ($directory+'\WindowsPowerShell\v1.0\powershell.exe')
}

function Invoke-PanelSetupCommand([switch]$Install) {
    $script=Join-Path $PSScriptRoot 'Setup-PanelModes.ps1'
    $arguments='-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "'+$script+'"'
    if($Install){$arguments+=' -Install'}
    # File paths are process arguments, never PowerShell source or shell code.
    $start=New-Object Diagnostics.ProcessStartInfo
    $start.FileName=Get-PanelPowerShell
    $start.Arguments=$arguments
    $start.UseShellExecute=$false
    $start.CreateNoWindow=$true
    $start.RedirectStandardOutput=$true
    $start.RedirectStandardError=$true
    $process=New-Object Diagnostics.Process
    $process.StartInfo=$start
    try {
        if(!$process.Start()){throw 'Could not start the panel setup helper.'}
        $stdout=$process.StandardOutput.ReadToEndAsync()
        $stderr=$process.StandardError.ReadToEndAsync()
        $process.WaitForExit()
        $output=$stdout.GetAwaiter().GetResult()
        $errorText=$stderr.GetAwaiter().GetResult()
        if($process.ExitCode-ne 0){
            if(!$errorText){$errorText=$output}
            throw $errorText.Trim()
        }
        return $output
    } finally {$process.Dispose()}
}

function Test-PanelAdministrator {
    $principal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Test-PanelElevationCancelled([Exception]$Exception) {
    while($null-ne $Exception){
        if($Exception-is [ComponentModel.Win32Exception]-and $Exception.NativeErrorCode-eq 1223){return $true}
        $Exception=$Exception.InnerException
    }
    return $false
}

function Show-PanelSetupFailure([string]$Message) {
    Add-Type -AssemblyName System.Windows.Forms
    [Windows.Forms.MessageBox]::Show("The app is installed, but the optional panel modes were not added.`r`n`r`n"+$Message+"`r`n`r`nYou can run setup again after preparing the laptop.",'48Hz and 120Hz panel setup','OK','Warning')|Out-Null
}

function Invoke-PanelInstaller([string]$RequestedAction) {
    try {
        # Recheck in each process, including after elevation. An existing matching
        # setup needs neither a registry write nor an administrator prompt.
        $info=Invoke-PanelSetupCommand|ConvertFrom-Json
        $status=Get-PanelInstallerStatus $info
        if($RequestedAction-eq 'Probe'-or $status-ne 0){return $status}
        if($RequestedAction-eq 'Install'-and !(Test-PanelAdministrator)) {
            try {
                $arguments='-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "'+(Join-Path $PSScriptRoot 'Installer-PanelModes.ps1')+'" -Action Apply'
                $child=Start-Process -FilePath (Get-PanelPowerShell) -ArgumentList $arguments -Verb RunAs -WindowStyle Hidden -Wait -PassThru
                try{return $child.ExitCode}finally{$child.Dispose()}
            } catch {
                if(Test-PanelElevationCancelled $_.Exception){return 1223}
                throw
            }
        }
        if(!(Test-PanelAdministrator)){throw 'Administrator access is required for panel setup.'}
        try {
            Invoke-PanelSetupCommand -Install|Out-Null
            return 3010
        } catch {
            Show-PanelSetupFailure $_.Exception.Message
            return 20
        }
    } catch {
        # Probe failures are read-only and simply omit the optional offer.
        # If a supported panel disappears after consent/elevation, explain it.
        if($RequestedAction-eq 'Apply') {
            Show-PanelSetupFailure $_.Exception.Message
            return 20
        }
        [Console]::WriteLine($_.Exception.Message)
        if($RequestedAction-eq 'Probe'){return 20}
        return 1
    }
}

# Dot-sourcing exposes functions to the hardware-free launcher tests only.
if($MyInvocation.InvocationName-ne '.') {
    $code=Invoke-PanelInstaller $Action
    switch($code) {
        10 {[Console]::WriteLine('48Hz and 120Hz modes are already installed.')}
        3010 {[Console]::WriteLine('Panel modes are set up. Restart Windows to make them available.')}
        1223 {[Console]::WriteLine('Administrator request cancelled. No panel changes were made.')}
        20 {[Console]::WriteLine('Optional panel setup is unavailable or could not complete.')}
    }
    exit $code
}
