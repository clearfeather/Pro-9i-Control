# No display changes. Registry mutations are confined to a random HKCU test key.
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot '..\tools\120hz\Edid120.ps1')
function Assert($Condition,[string]$Message){if(!$Condition){throw "FAIL: $Message"}}
function Reject([scriptblock]$Action,[string]$Message){$rejected=$false;try{& $Action|Out-Null}catch{$rejected=$true};Assert $rejected $Message}
$factory=[IO.File]::ReadAllBytes((Join-Path $PSScriptRoot 'fixtures\cso1418-anonymized.bin'))
$original=Get-Factory120Edid $factory
$candidate=New-120HzEdid $original
Assert ($candidate.Length-eq 512) 'four-block candidate'
Assert ((Get-ByteHash (Get-Factory120Edid $candidate))-eq (Get-ByteHash $factory)) 'exact factory round trip'
Assert ((Get-ByteHash ([byte[]]$candidate[128..383]))-eq (Get-ByteHash ([byte[]]$factory[128..383]))) 'factory HDR and 165Hz blocks preserved'
$changed=@(for($i=0;$i-lt 384;$i++){if($factory[$i]-ne $candidate[$i]){$i}})
Assert (($changed-join ',')-eq '126,127') 'only factory extension count/checksum changed'
$pixelClock=($candidate[392]+256*$candidate[393]+65536*$candidate[394]+1)*10000
Assert ($pixelClock-eq 805930000) 'reviewed 120Hz pixel clock'
Assert (($candidate[395]-band 128)-eq 0) '120Hz is nonpreferred'
$differentSerial=[byte[]]$factory.Clone();$differentSerial[12]=42;$differentSerial[16]=33;Set-BlockChecksum $differentSerial 0
Assert ((New-120HzEdid (Get-Factory120Edid $differentSerial))[12]-eq 42) 'per-unit serial retained'
$bad=[byte[]]$factory.Clone();$bad[8]=1;Set-BlockChecksum $bad 0
Reject {Get-Factory120Edid $bad} 'wrong panel refused'
$bad=[byte[]]$factory.Clone();$bad[140]=$bad[140]-bxor 1;Set-BlockChecksum $bad 128
Reject {Get-Factory120Edid $bad} 'different HDR layout refused'
$bad=[byte[]]$factory.Clone();$bad[55]=$bad[55]-bxor 1;Set-BlockChecksum $bad 0
Reject {Get-Factory120Edid $bad} 'different factory timing refused'
$bad=[byte[]]$factory.Clone();$bad[1]=0
Reject {Get-Factory120Edid $bad} 'invalid checksum refused'
$bad=[byte[]]$candidate.Clone();$bad[392]=$bad[392]-bxor 1;Set-BlockChecksum $bad 384
Reject {Get-Factory120Edid $bad} 'foreign fourth block refused'

$testPath='Software\Pro9iControl.Tests\120Hz\'+[Guid]::NewGuid().ToString('N')
$testRoot=[Microsoft.Win32.Registry]::CurrentUser.CreateSubKey($testPath,$true)
try {
    Install-120Override $testRoot $candidate|Out-Null
    $key=$testRoot.OpenSubKey('EDID_OVERRIDE')
    try{Assert-Owned120Override $key $candidate}finally{$key.Dispose()}
    Assert ((Install-120Override $testRoot $candidate)-like 'Already installed*') 'install idempotence'
    Remove-120Override $testRoot $candidate|Out-Null
    Assert ($null-eq $testRoot.OpenSubKey('EDID_OVERRIDE')) 'owned override removed'
    Assert ((Remove-120Override $testRoot $candidate)-like 'Already absent*') 'removal idempotence'
    Reject {Install-120Override $testRoot $factory} 'partial candidate refused'
    Install-120Override $testRoot $candidate|Out-Null
    $key=$testRoot.OpenSubKey('EDID_OVERRIDE',$true)
    try{$key.SetValue('foreign','keep me')}finally{$key.Dispose()}
    Reject {Install-120Override $testRoot $candidate} 'foreign data blocks installation'
    Reject {Remove-120Override $testRoot $candidate} 'foreign data blocks removal'
    $key=$testRoot.OpenSubKey('EDID_OVERRIDE',$true)
    try{Assert ($key.GetValue('foreign')-eq 'keep me') 'foreign data preserved';$key.DeleteValue('foreign')}finally{$key.Dispose()}
    Remove-120Override $testRoot $candidate|Out-Null
    $key=$testRoot.CreateSubKey('EDID_OVERRIDE',$true)
    try{$key.SetValue('0',[byte[]]$candidate[0..127],[Microsoft.Win32.RegistryValueKind]::Binary)}finally{$key.Dispose()}
    Reject {Remove-120Override $testRoot $candidate} 'incomplete data refused by ordinary removal'
    Remove-120Override $testRoot $candidate -AllowPartial|Out-Null
    Assert ($null-eq $testRoot.OpenSubKey('EDID_OVERRIDE')) 'failed-install partial bytes can be recovered'
} finally {
    $testRoot.Dispose()
    [Microsoft.Win32.Registry]::CurrentUser.DeleteSubKeyTree($testPath,$false)
}
Add-Type -Path (Join-Path $PSScriptRoot '..\tools\120hz\DisplayReadOnly.cs')
Add-Type -Path (Join-Path $PSScriptRoot '..\tools\120hz\IntelDescriptorReadOnly.cs')
'PASS: EDID identity, factory preservation, 120Hz timing, serial retention, corruption/foreign-data refusal, HKCU install/removal/recovery, native helper compilation. No display settings changed.'
