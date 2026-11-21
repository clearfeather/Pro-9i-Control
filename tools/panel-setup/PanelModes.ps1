# Pure EDID validation/generation and narrowly scoped registry helpers.
# Importing this file does not read or write hardware or the registry.
Set-StrictMode -Version Latest

function Get-ByteHash([byte[]]$Bytes) {
    $sha=[Security.Cryptography.SHA256]::Create()
    try { return ([BitConverter]::ToString($sha.ComputeHash($Bytes))).Replace('-','').ToLowerInvariant() }
    finally { $sha.Dispose() }
}

function Set-BlockChecksum([byte[]]$Bytes,[int]$Start) {
    $Bytes[$Start+127]=0
    $sum=0;for($i=$Start;$i-lt $Start+127;$i++){$sum+=$Bytes[$i]}
    $Bytes[$Start+127]=[byte]((256-($sum%256))%256)
}

function Assert-EdidChecksums([byte[]]$Bytes) {
    if($Bytes.Length-notin @(384,512)){throw 'Expected the complete three-block factory EDID or our four-block override.'}
    for($offset=0;$offset-lt $Bytes.Length;$offset+=128){
        $sum=0;for($i=$offset;$i-lt $offset+128;$i++){$sum+=$Bytes[$i]}
        if($sum%256-ne 0){throw "Invalid EDID checksum in block $($offset/128)."}
    }
    if($Bytes[126]+1-ne $Bytes.Length/128){throw 'EDID extension count does not match its length.'}
}

function Get-FactoryPanelEdid([byte[]]$Bytes) {
    Assert-EdidChecksums $Bytes
    if([BitConverter]::ToString($Bytes[0..11])-ne '00-FF-FF-FF-FF-FF-FF-00-0E-6F-18-14') {
        throw 'This setup supports only the reviewed CSO1418 panel.'
    }
    $factory=[byte[]]$Bytes[0..383]
    $factory[126]=2;Set-BlockChecksum $factory 0
    # Ignore per-unit serial/date fields when matching the reviewed factory
    # layout. Preserve all of these bytes in the generated override.
    $canonical=[byte[]]$factory.Clone()
    foreach($offset in 12..17){$canonical[$offset]=0}
    foreach($offset in @(54,72,90,108)) {
        if($canonical[$offset]-eq 0-and $canonical[$offset+1]-eq 0-and $canonical[$offset+3]-eq 255) {
            foreach($textOffset in ($offset+5)..($offset+17)){$canonical[$textOffset]=0}
        }
    }
    $canonical[127]=0
    if((Get-ByteHash $canonical)-ne '033d42a3cdee3bd6b8559149de526e30d2d7688b4fcd3fdadc111b99637e370c') {
        throw 'Factory timings or HDR metadata differ from the reviewed panel. No override can be installed.'
    }
    if($Bytes.Length-eq 512) {
        $candidate=New-PanelModesEdid $factory
        $legacy=New-PanelModesEdid $factory -Legacy120Only
        if((Get-ByteHash $candidate)-ne (Get-ByteHash $Bytes)-and (Get-ByteHash $legacy)-ne (Get-ByteHash $Bytes)){throw 'The existing fourth EDID block is not one of our reviewed panel mode sets.'}
    }
    return ,$factory
}

function New-PanelModesEdid([byte[]]$Factory,[switch]$Legacy120Only) {
    if($Factory.Length-ne 384){throw 'Generate a candidate from a validated complete factory EDID.'}
    $result=New-Object byte[] 512
    [Array]::Copy($Factory,$result,384)
    $added=New-Object byte[] 128
    [byte[]]$header=0x70,0x12,121,0,0,3,1,20
    [Array]::Copy($header,$added,8)
    # Copy factory 165Hz geometry; change only the pixel clock to 805.930MHz
    # and clear preferred. Factory 60/165 and all HDR bytes stay intact.
    [Array]::Copy($Factory,264,$added,8,20)
    $encodedClock=80593-1
    $added[8]=[byte]($encodedClock-band 255)
    $added[9]=[byte](($encodedClock-shr 8)-band 255)
    $added[10]=[byte](($encodedClock-shr 16)-band 255)
    $added[11]=[byte]($added[11]-band 127)
    if(!$Legacy120Only) {
        # Add 48Hz after the unchanged 120Hz descriptor. Round upward at
        # the panel's declared 48Hz lower limit: 322.380MHz / 48.001Hz.
        $added[7]=40
        [Array]::Copy($Factory,264,$added,28,20)
        $clock48=32238-1
        $added[28]=[byte]($clock48-band 255)
        $added[29]=[byte](($clock48-shr 8)-band 255)
        $added[30]=[byte](($clock48-shr 16)-band 255)
        $added[31]=[byte]($added[31]-band 127)
    }
    $sum=0;for($i=1;$i-lt 126;$i++){$sum+=$added[$i]}
    $added[126]=[byte]((256-($sum%256))%256)
    Set-BlockChecksum $added 0
    [Array]::Copy($added,0,$result,384,128)
    $result[126]=3;Set-BlockChecksum $result 0
    Assert-EdidChecksums $result
    return ,$result
}

function Read-OwnedPanelOverride([Microsoft.Win32.RegistryKey]$Key) {
    if($Key.SubKeyCount-ne 0-or (($Key.GetValueNames()|Sort-Object)-join ',')-ne '0,1,2,3'){throw 'Existing override has foreign or incomplete data.'}
    foreach($block in 0..3){if($Key.GetValueKind([string]$block)-ne [Microsoft.Win32.RegistryValueKind]::Binary-or ([byte[]]$Key.GetValue([string]$block)).Length-ne 128){throw 'Existing override has an unexpected block type or length.'}}
    [byte[]]$image=@(foreach($block in 0..3){$Key.GetValue([string]$block)})
    $validated=Get-FactoryPanelEdid $image
    return ,$image
}

function Assert-OwnedPanelOverride([Microsoft.Win32.RegistryKey]$Key,[byte[]]$Image,[switch]$AllowPartial) {
    $names=@($Key.GetValueNames()|Sort-Object)
    if($Key.SubKeyCount-ne 0-or (!$AllowPartial-and ($names-join ',')-ne '0,1,2,3')){throw 'Existing override has foreign or incomplete data; refusing to change it.'}
    foreach($name in $names) {
        if($name-notin @('0','1','2','3')){throw 'Existing override has foreign values.'}
        $offset=128*[int]$name
        if($Key.GetValueKind($name)-ne [Microsoft.Win32.RegistryValueKind]::Binary-or
            (Get-ByteHash ([byte[]]$Key.GetValue($name)))-ne (Get-ByteHash ([byte[]]$Image[$offset..($offset+127)]))) {
            throw 'Existing override bytes differ; refusing to change them.'
        }
    }
}

function Install-PanelOverride([Microsoft.Win32.RegistryKey]$Parent,[byte[]]$Image) {
    if($Image.Length-ne 512){throw 'Only a complete four-block candidate can be installed.'}
    $validated=Get-FactoryPanelEdid $Image
    $existing=$Parent.OpenSubKey('EDID_OVERRIDE')
    if($null-ne $existing){
        try{$previous=Read-OwnedPanelOverride $existing}finally{$existing.Dispose()}
        if((Get-ByteHash $previous)-eq (Get-ByteHash $Image)){return 'Already installed; no registry changes.'}
        $factory=Get-FactoryPanelEdid $previous
        $legacy=New-PanelModesEdid $factory -Legacy120Only
        $upgraded=New-PanelModesEdid $factory
        if((Get-ByteHash $previous)-ne (Get-ByteHash $legacy)-or (Get-ByteHash $Image)-ne (Get-ByteHash $upgraded)){throw 'Only the reviewed 120Hz-to-48/120Hz upgrade is allowed.'}
        $key=$Parent.OpenSubKey('EDID_OVERRIDE',$true)
        try {
            Assert-OwnedPanelOverride $key $previous
            $key.SetValue('3',[byte[]]$Image[384..511],[Microsoft.Win32.RegistryValueKind]::Binary)
            $key.Flush();Assert-OwnedPanelOverride $key $Image
        } catch {
            $writeError=$_
            if((Get-ByteHash ([byte[]]$key.GetValue('3')))-eq (Get-ByteHash ([byte[]]$Image[384..511]))){$key.SetValue('3',[byte[]]$previous[384..511],[Microsoft.Win32.RegistryValueKind]::Binary);$key.Flush()}
            throw $writeError
        } finally {$key.Dispose()}
        return 'Added 48Hz to the owned 120Hz override; factory and 120Hz bytes preserved. Restart Windows.'
    }
    $key=$Parent.CreateSubKey('EDID_OVERRIDE',$true)
    try {
        foreach($block in 0..3){$offset=$block*128;$key.SetValue([string]$block,[byte[]]$Image[$offset..($offset+127)],[Microsoft.Win32.RegistryValueKind]::Binary)}
        $key.Flush();Assert-OwnedPanelOverride $key $Image
    } catch {
        $writeError=$_
        $key.Dispose();$key=$null
        # Remove only bytes belonging to this failed write; never foreign data.
        Remove-PanelOverride $Parent $Image -AllowPartial|Out-Null
        throw $writeError
    } finally {if($null-ne $key){$key.Dispose()}}
    return 'Installed and verified four contiguous EDID blocks. Restart Windows before selecting 48Hz or 120Hz.'
}

function Remove-PanelOverride([Microsoft.Win32.RegistryKey]$Parent,[byte[]]$Image,[switch]$AllowPartial) {
    if($Image.Length-ne 512){throw 'Only a complete four-block candidate can establish override ownership.'}
    $validated=Get-FactoryPanelEdid $Image
    $key=$Parent.OpenSubKey('EDID_OVERRIDE')
    if($null-eq $key){return 'Already absent; no registry changes.'}
    try{Assert-OwnedPanelOverride $key $Image -AllowPartial:$AllowPartial}finally{$key.Dispose()}
    $Parent.DeleteSubKey('EDID_OVERRIDE',$false);$Parent.Flush()
    $remaining=$Parent.OpenSubKey('EDID_OVERRIDE')
    if($null-ne $remaining){$remaining.Dispose();throw 'Targeted override removal did not verify.'}
    return 'Removed only the matching panel mode override. Restart Windows to reload factory modes.'
}
