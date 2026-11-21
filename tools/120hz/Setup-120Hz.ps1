#Requires -Version 5.1
[CmdletBinding(DefaultParameterSetName='Check')]
param(
    [Parameter(ParameterSetName='Install',Mandatory=$true)][switch]$Install,
    [Parameter(ParameterSetName='Remove',Mandatory=$true)][switch]$Remove
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'Edid120.ps1')

function Assert-Administrator {
    $principal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
    if(!$principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Open 64-bit Windows PowerShell as administrator, then run the documented command.'}
}

function Get-PanelRegistryPath([string]$DevicePath) {
    if($DevicePath-notmatch '^\\\\\?\\DISPLAY#CSO1418#([A-Za-z0-9_&]+)#\{[0-9a-fA-F-]+\}$'){throw 'Unexpected panel device identity.'}
    return 'SYSTEM\CurrentControlSet\Enum\DISPLAY\CSO1418\'+$Matches[1]+'\Device Parameters'
}

function Save-LocalBackup([string]$RegistryPath,[byte[]]$Factory,[byte[]]$Candidate,[string]$Action) {
    $folder=Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) ('Pro 9i Control\120Hz\'+[Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $folder -Force|Out-Null
    [IO.File]::WriteAllBytes((Join-Path $folder 'factory-edid.bin'),$Factory)
    [IO.File]::WriteAllBytes((Join-Path $folder 'candidate-edid.bin'),$Candidate)
    [ordered]@{createdAtUtc=[DateTime]::UtcNow.ToString('o');action=$Action;registryParent=$RegistryPath;factorySha256=(Get-ByteHash $Factory);candidateSha256=(Get-ByteHash $Candidate)}|
        ConvertTo-Json|Set-Content -LiteralPath (Join-Path $folder 'backup.json') -Encoding UTF8
    return $folder
}

if($Remove) {
    Assert-Administrator
    # Registry-only recovery also works in Safe Mode without Intel/WMI APIs.
    $root=[Microsoft.Win32.Registry]::LocalMachine.OpenSubKey('SYSTEM\CurrentControlSet\Enum\DISPLAY\CSO1418')
    if($null-eq $root){throw 'No CSO1418 panel registry entry found.'}
    $owned=@()
    try {
        foreach($instance in $root.GetSubKeyNames()) {
            if($instance-notmatch '^[A-Za-z0-9_&]+$'){continue}
            $parent=$root.OpenSubKey($instance+'\Device Parameters')
            if($null-eq $parent){continue}
            try {
                $key=$parent.OpenSubKey('EDID_OVERRIDE')
                if($null-eq $key){continue}
                try {
                    if((($key.GetValueNames()|Sort-Object)-join ',')-ne '0,1,2,3'){continue}
                    [byte[]]$image=@(foreach($number in 0..3){$key.GetValue([string]$number)})
                    $factory=Get-Factory120Edid $image
                    Assert-Owned120Override $key $image
                    $owned+=,[pscustomobject]@{instance=$instance;image=$image;factory=$factory}
                } catch {Write-Warning 'An unrelated override was left unchanged.'} finally {$key.Dispose()}
            } finally {$parent.Dispose()}
        }
    } finally {$root.Dispose()}
    if($owned.Count-ne 1){throw 'Expected exactly one matching owned 120Hz override; no registry data changed.'}
    $registryPath='SYSTEM\CurrentControlSet\Enum\DISPLAY\CSO1418\'+$owned[0].instance+'\Device Parameters'
    $backup=Save-LocalBackup $registryPath $owned[0].factory $owned[0].image 'remove'
    $parent=[Microsoft.Win32.Registry]::LocalMachine.OpenSubKey($registryPath,$true)
    try {Remove-120Override $parent $owned[0].image}finally{$parent.Dispose()}
    Write-Output "Local backup: $backup"
    exit
}

if($env:OS-ne 'Windows_NT'-or ![Environment]::Is64BitProcess){throw 'Use 64-bit Windows PowerShell on Windows.'}
if($Install){Assert-Administrator}
$computer=Get-CimInstance Win32_ComputerSystem
if($computer.Manufacturer-notlike '*LENOVO*'-or $computer.Model-notlike '83BV*'){throw '120Hz setup is limited to the reviewed Lenovo 83BV / CSO1418 configuration.'}
Add-Type -Path (Join-Path $PSScriptRoot 'DisplayReadOnly.cs')
$snapshot=[Pro9i120Display]::ReadActive()
$panels=@($snapshot.active|Where-Object {$_.devicePath-like '*DISPLAY#CSO1418#*'-and $_.path.target.technology-eq 2147483648})
if($panels.Count-ne 1){throw 'Expected one active built-in CSO1418 panel.'}
$panel=$panels[0]
$shared=@($snapshot.active|Where-Object {$_.path.source.adapter.low-eq $panel.path.source.adapter.low-and $_.path.source.adapter.high-eq $panel.path.source.adapter.high-and $_.path.source.id-eq $panel.path.source.id})
if($shared.Count-ne 1-or $panel.signalWidth-ne 3072-or $panel.signalHeight-ne 1920){throw 'Use native 3072x1920 and Extend displays before checking this setup.'}
$registryPath=Get-PanelRegistryPath $panel.devicePath
Add-Type -Path (Join-Path $PSScriptRoot 'IntelDescriptorReadOnly.cs')
$effective=[Pro9i120Intel]::Read([uint32]$panel.path.target.id)
$factory=Get-Factory120Edid $effective
$candidate=New-120HzEdid $factory
$parent=[Microsoft.Win32.Registry]::LocalMachine.OpenSubKey($registryPath,[bool]$Install)
if($null-eq $parent){throw 'The selected physical panel registry entry is unavailable.'}
try {
    $key=$parent.OpenSubKey('EDID_OVERRIDE')
    $alreadyInstalled=$null-ne $key
    if($alreadyInstalled){try{Assert-Owned120Override $key $candidate}finally{$key.Dispose()}}
    $display=@([Pro9i120Display]::ReadModes()|Where-Object {$_.device.name-eq $panel.sourceName})
    if($display.Count-ne 1){throw 'Cannot read the selected display modes.'}
    $rates=@($display[0].compatible|Where-Object {$_.width-eq 3072-and $_.height-eq 1920}|Select-Object -ExpandProperty hz -Unique|Sort-Object)
    if(!$Install){
        [pscustomobject]@{action='Read-only check';supported=$true;panel='CSO1418 / MNE507HZ2-1';overrideInstalled=$alreadyInstalled;nativeRefreshRates=$rates;restartRequired=($alreadyInstalled-and 120-notin $rates)}|ConvertTo-Json
    } elseif($alreadyInstalled){
        Write-Output 'The matching 120Hz override is already installed. No registry data changed.'
    } else {
        if(![Pro9i120Display]::OnAC()){throw 'Connect AC power before installation.'}
        if(@($snapshot.active|Where-Object {$_.path.target.technology-ne 2147483648}).Count-eq 0){throw 'Connect an external recovery display in Extend mode before installation.'}
        if(($panel.advancedColorFlags-band 2)-ne 0-or ($rates|Where-Object {$_-in @(60,165)}).Count-ne 2){throw 'Begin at a factory mode with HDR off and factory60/165 both available.'}
        $hz=$panel.path.target.refresh.numerator/$panel.path.target.refresh.denominator
        if([math]::Min([math]::Abs($hz-60),[math]::Abs($hz-165))-gt 0.1){throw 'Select a factory60Hz or165Hz mode before installation.'}
        $backup=Save-LocalBackup $registryPath $factory $candidate 'install'
        # Detect hotplug/replacement immediately before the sole hardware write.
        $now=@([Pro9i120Display]::ReadActive().active|Where-Object devicePath -eq $panel.devicePath)
        if($now.Count-ne 1-or $now[0].sourceName-ne $panel.sourceName){throw 'Display topology changed. No override installed.'}
        Install-120Override $parent $candidate
        Write-Output "Local backup: $backup"
    }
} finally {$parent.Dispose()}
