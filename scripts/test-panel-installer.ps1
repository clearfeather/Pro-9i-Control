# Hardware-free tests: all panel reads, elevation, and writes are mocked.
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot '..\tools\panel-setup\Installer-PanelModes.ps1')
function Assert($Condition,[string]$Message){if(!$Condition){throw "FAIL: $Message"}}
function Reset-Scenario {
    $script:scenario=@{info=@{supported=$true;includes48Hz=$false;restartRequired=$false};admin=$false;installCalls=0;elevationCalls=0;failureMessages=0;cancel=$false;installFails=$false;probeFails=$false;disposed=$false}
}
function Invoke-PanelSetupCommand([switch]$Install) {
    if($Install){$script:scenario.installCalls++;if($script:scenario.installFails){throw 'Simulated setup prerequisite failure.'};return 'Installed'}
    if($script:scenario.probeFails){throw 'Unsupported panel or factory layout.'}
    return ($script:scenario.info|ConvertTo-Json)
}
function Test-PanelAdministrator {return $script:scenario.admin}
function Show-PanelSetupFailure([string]$Message){$script:scenario.failureMessages++;$script:scenario.lastFailure=$Message}
function Start-Process {
    param($FilePath,$ArgumentList,$Verb,$WindowStyle,[switch]$Wait,[switch]$PassThru)
    $script:scenario.elevationCalls++
    Assert ($Verb-eq 'RunAs'-and $WindowStyle-eq 'Hidden'-and $Wait-and $PassThru) 'elevation is hidden, awaited, and explicit'
    Assert ($ArgumentList.EndsWith('" -Action Apply')-and $ArgumentList.Contains('-File "')) 'only the fixed quoted helper is elevated'
    if($script:scenario.cancel){throw (New-Object ComponentModel.Win32Exception(1223))}
    $child=[pscustomobject]@{ExitCode=3010}
    $child|Add-Member -MemberType ScriptMethod -Name Dispose -Value {$script:scenario.disposed=$true}
    return $child
}

Reset-Scenario
Assert ((Invoke-PanelInstaller 'Probe')-eq 0) 'supported factory/legacy setup is offered'
Assert ($scenario.installCalls-eq 0-and $scenario.elevationCalls-eq 0) 'probe never installs or elevates'
Reset-Scenario;$scenario.info.includes48Hz=$true
Assert ((Invoke-PanelInstaller 'Install')-eq 10) 'matching active modes skip setup'
Assert ($scenario.elevationCalls-eq 0-and $scenario.installCalls-eq 0) 'matching modes do not request UAC or write'
Reset-Scenario;$scenario.info.includes48Hz=$true;$scenario.info.restartRequired=$true
Assert ((Invoke-PanelInstaller 'Probe')-eq 3010) 'pending modes request a restart'
Reset-Scenario;$scenario.probeFails=$true
Assert ((Invoke-PanelInstaller 'Probe')-eq 20) 'unsupported hardware omits the offer'
Assert ($scenario.elevationCalls-eq 0-and $scenario.installCalls-eq 0-and $scenario.failureMessages-eq 0) 'unsupported read-only check has no side effects'
Reset-Scenario
Assert ((Invoke-PanelInstaller 'Install')-eq 3010) 'nonadmin install waits for elevated result'
Assert ($scenario.elevationCalls-eq 1-and $scenario.installCalls-eq 0-and $scenario.disposed) 'only the child may perform a nonadmin install'
Reset-Scenario;$scenario.cancel=$true
Assert ((Invoke-PanelInstaller 'Install')-eq 1223) 'UAC cancellation is preserved'
Assert ($scenario.installCalls-eq 0) 'cancellation never writes'
Reset-Scenario;$scenario.admin=$true
Assert ((Invoke-PanelInstaller 'Apply')-eq 3010) 'elevated supported setup succeeds'
Assert ($scenario.installCalls-eq 1-and $scenario.elevationCalls-eq 0) 'elevated setup applies once without another UAC prompt'
Reset-Scenario;$scenario.admin=$true;$scenario.installFails=$true
Assert ((Invoke-PanelInstaller 'Apply')-eq 20) 'failed prerequisites leave app installation successful'
Assert ($scenario.failureMessages-eq 1-and $scenario.lastFailure-like '*prerequisite*') 'specific setup failure is shown once'
Reset-Scenario;$scenario.probeFails=$true
Assert ((Invoke-PanelInstaller 'Apply')-eq 20) 'topology/panel is rechecked after elevation'
Assert ($scenario.installCalls-eq 0-and $scenario.failureMessages-eq 1) 'failed elevated recheck never writes'
Reset-Scenario
Assert ((Invoke-PanelInstaller 'Apply')-eq 20) 'direct apply without admin is refused'
Assert ($scenario.installCalls-eq 0) 'nonadmin apply never writes'
'PASS: installer offer, existing/pending modes, unsupported panel, elevation, cancellation, fresh checks, and failure handling. No display settings changed.'
