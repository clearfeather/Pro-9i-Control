[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Executable,
    [string]$Label = 'boost',
    [ValidateSet('live', 'idle', 'battery', 'saver', 'power-transition')][string]$Scenario = 'live'
)

$ErrorActionPreference = 'Stop'
$projectFolder = Split-Path -Parent $PSScriptRoot
$outputFolder = Join-Path $projectFolder 'src-tauri\target\hdr-perf'
New-Item -ItemType Directory -Path $outputFolder -Force | Out-Null
$executablePath = (Resolve-Path -LiteralPath $Executable).Path
$safeLabel = $Label -replace '[^a-zA-Z0-9_-]', '_'
$stdout = Join-Path $outputFolder "$safeLabel.stdout.log"
$stderr = Join-Path $outputFolder "$safeLabel.stderr.log"
$dwmId = (Get-Process dwm | Select-Object -First 1).Id

# Measure the same bounded native workload for each executable. The helper owns
# HDR/refresh recovery and its watchdog; this script never terminates it.
$helper = Start-Process -FilePath $executablePath -ArgumentList $Scenario -WindowStyle Hidden -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
$startup = [System.Diagnostics.Stopwatch]::StartNew()
do {
    $startupLog = Get-Content -LiteralPath $stdout -Raw -ErrorAction SilentlyContinue
    if ($startupLog -match 'Live HDR pipeline initialized') { break }
    if ($helper.HasExited -or $startup.Elapsed.TotalSeconds -gt 15) { throw "The helper did not initialize: $startupLog" }
    Start-Sleep -Milliseconds 50
} while ($true)
# Expand the counter wildcard after the helper creates its GPU context, so its
# engine instances are included along with DWM's already-existing instances.
$samples = @(Get-Counter '\GPU Engine(*)\Utilization Percentage' -SampleInterval 1 -MaxSamples 15 -ErrorAction SilentlyContinue | ForEach-Object {
    $reading = $_
    $log = Get-Content -LiteralPath $stdout -Raw -ErrorAction SilentlyContinue
    $active = $log -match 'Live HDR pipeline initialized' -and $log -notmatch 'Live HDR session ended|After:'
    # A process leaving the GPU removes its instances while sampling. Discard
    # those invalid counters instead of treating the renderer's exit as zero load.
    $valid = @($reading.CounterSamples | Where-Object { $_.Status -in @(0, 1) -and [double]::IsFinite($_.CookedValue) -and $_.CookedValue -ge 0 })
    $app = @($valid | Where-Object { $_.InstanceName -like "pid_$($helper.Id)_*" })
    $dwm = @($valid | Where-Object { $_.InstanceName -like "pid_${dwmId}_*" })
    $engines = @($valid | Group-Object { $_.InstanceName -replace '^pid_\d+_', '' } | ForEach-Object { ($_.Group | Measure-Object CookedValue -Sum).Sum })
    [pscustomobject]@{
        Time = $reading.Timestamp.ToString('o')
        Active = $active
        Boost = [double](($app | Measure-Object CookedValue -Maximum).Maximum)
        Dwm = [double](($dwm | Measure-Object CookedValue -Maximum).Maximum)
        BusiestEngine = [double](($engines | Measure-Object -Maximum).Maximum)
        TopEngines = @($valid | Sort-Object CookedValue -Descending | Select-Object -First 5 InstanceName,CookedValue)
    }
})
if (-not $helper.WaitForExit(20000)) { throw 'The bounded helper did not finish; its recovery record is retained.' }
$helper.Refresh()
$log = Get-Content -LiteralPath $stdout -Raw
$errors = Get-Content -LiteralPath $stderr -Raw
if ($helper.ExitCode -ne 0 -or $log -notmatch 'PASS: bounded benchmark') { throw "Native check failed: $log $errors" }
$activeSamples = @($samples | Where-Object Active)
if ($activeSamples.Count -lt 3) { throw 'Too few active GPU samples for comparison.' }
$result = [pscustomobject]@{
    Label = $Label
    BoostProcessId = $helper.Id
    Scenario = $Scenario
    ActiveSamples = $activeSamples.Count
    MeanBoost = [math]::Round(($activeSamples | Measure-Object Boost -Average).Average, 2)
    MeanDwm = [math]::Round(($activeSamples | Measure-Object Dwm -Average).Average, 2)
    MeanBusiestEngine = [math]::Round(($activeSamples | Measure-Object BusiestEngine -Average).Average, 2)
    Samples = $samples
}
$result | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $outputFolder "$safeLabel.json") -Encoding utf8
$result | Select-Object Label,Scenario,ActiveSamples,MeanBoost,MeanDwm,MeanBusiestEngine | Format-List
Write-Output $log
