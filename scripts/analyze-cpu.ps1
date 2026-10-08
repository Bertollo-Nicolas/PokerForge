[CmdletBinding()]
param([Parameter(Mandatory)][string]$CaptureDirectory,[int]$ProcessId=0)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
$dir=(Resolve-Path -LiteralPath $CaptureDirectory).Path
$xperf='C:/Program Files (x86)/Windows Kits/10/Windows Performance Toolkit/xperf.exe'
$meta=Get-Content -Raw -LiteralPath (Join-Path $dir 'metadata.json') | ConvertFrom-Json
if((Get-FileHash -LiteralPath (Join-Path $dir 'pokerforge.exe')).Hash -ne $meta.ExecutableSHA256){throw 'Captured executable hash mismatch'}
$out=Join-Path $dir 'analysis'; $null=New-Item -ItemType Directory -Force -Path $out
$cache=Join-Path $out 'symcache'; $null=New-Item -ItemType Directory -Force -Path $cache
$oldSymbols=$env:_NT_SYMBOL_PATH; $oldCache=$env:_NT_SYMCACHE_PATH
try {
    $env:_NT_SYMBOL_PATH=$dir; $env:_NT_SYMCACHE_PATH=$cache
    & $xperf -i (Join-Path $dir 'cpu.etl') -symbols -o (Join-Path $out 'raw-functions.txt') -a profile -detail
    if($LASTEXITCODE -ne 0){throw 'xperf failed'}
} finally {$env:_NT_SYMBOL_PATH=$oldSymbols; $env:_NT_SYMCACHE_PATH=$oldCache}
$rows=@(Get-Content -LiteralPath (Join-Path $out 'raw-functions.txt') | ForEach-Object {
    if($_ -match '^\s*pokerforge\.exe \((\d+)\),\s*(\d+),\s*[\d.]+,\s*(.*)$') {
        if($ProcessId -eq 0 -or [int]$matches[1] -eq $ProcessId) {[pscustomobject]@{PID=[int]$matches[1];Weight=[double]$matches[2];Function=$matches[3]}}
    }
})
if($rows.Count -eq 0){throw 'No pokerforge samples found'}
if(@($rows.PID | Select-Object -Unique).Count -gt 1){throw 'Multiple pokerforge processes; specify -ProcessId'}
$total=($rows | Measure-Object Weight -Sum).Sum
$report=@($rows | Sort-Object Weight -Descending | ForEach-Object {[pscustomobject]@{PID=$_.PID;SampleWeight=$_.Weight;Percent=[Math]::Round(100*$_.Weight/$total,3);Function=$_.Function}})
$report | Export-Csv -LiteralPath (Join-Path $out 'pokerforge-functions.csv') -NoTypeInformation
$report | Select-Object -First 15 | Format-Table -Wrap
Write-Host "Total sample weight: $total. Shares normalized to pokerforge only; leaf attribution, not inclusive time. Unresolved modules remain Unknown. Results: $out"
