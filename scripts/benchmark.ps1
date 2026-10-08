[CmdletBinding()]
param(
    [string]$Corpus = 'data',
    [ValidateRange(1,100)][int]$Runs = 5,
    [ValidateRange(1,100000)][int]$Repeat = 10,
    [int[]]$Workers = @(1,2,4,8),
    [ValidateRange(1,2147483647)][int]$BlockBytes = 262144,
    [string]$OutputDirectory = '',
    [switch]$SkipBuild
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$project = Split-Path $PSScriptRoot -Parent
Push-Location $project
try {
    if (@($Workers | Where-Object {$_ -le 0}).Count -gt 0 -or $Workers.Count -eq 0) {throw 'Workers must be positive'}
    $Workers = @($Workers | Sort-Object -Unique)
    $corpusPath = (Resolve-Path -LiteralPath $Corpus).Path
    if (-not $OutputDirectory) { $OutputDirectory = Join-Path $project ('target/benchmarks/' + (Get-Date -Format 'yyyyMMdd-HHmmss-fff')) }
    $null = New-Item -ItemType Directory -Path $OutputDirectory -Force
    $outputPath = (Resolve-Path -LiteralPath $OutputDirectory).Path
    function Get-Manifest {
        $root = Get-Item -LiteralPath $corpusPath
        $items = if ($root.PSIsContainer) {Get-ChildItem -LiteralPath $corpusPath -File -Recurse | Where-Object {$_.Extension -ieq '.txt'}} else {@($root)}
        @($items | Sort-Object FullName | ForEach-Object {
            [pscustomobject]@{Path=[IO.Path]::GetRelativePath($project,$_.FullName);Bytes=$_.Length;SHA256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash}
        })
    }
    function Digest([string]$text) { [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($text))) }
    $manifest = @(Get-Manifest)
    if ($manifest.Count -eq 0) {throw 'No TXT files'}
    $manifestJson = ConvertTo-Json -InputObject $manifest -Depth 5 -Compress
    $corpusHash = Digest $manifestJson
    $manifestJson | Set-Content -LiteralPath (Join-Path $outputPath 'corpus.json') -Encoding utf8
    if (-not $SkipBuild) { & cargo build --release --locked; if ($LASTEXITCODE -ne 0) {throw 'Build failed'} }
    $exe = Join-Path $project 'target/release/pokerforge.exe'
    if (-not (Test-Path -LiteralPath $exe)) {throw 'Missing release executable'}
    $sourceFiles = @(Get-ChildItem src,tests,examples,scripts -File -Recurse) + @(Get-Item Cargo.toml,Cargo.lock)
    $sourceManifest = @($sourceFiles | Sort-Object FullName | ForEach-Object {[pscustomobject]@{Path=[IO.Path]::GetRelativePath($project,$_.FullName);SHA256=(Get-FileHash -LiteralPath $_.FullName).Hash}})
    $head = & git rev-parse --verify HEAD 2>$null
    if ($LASTEXITCODE -ne 0) {$head='uncommitted repository'}
    $metadata = [ordered]@{
        StartedUTC=[DateTime]::UtcNow.ToString('o');OS=[Environment]::OSVersion.ToString();CPU=$env:PROCESSOR_IDENTIFIER;LogicalProcessors=[Environment]::ProcessorCount
        Rust=(& rustc -V | Out-String).Trim();GitHead=$head;GitStatus=(& git status --short | Out-String)
        ExecutableSHA256=(Get-FileHash -LiteralPath $exe).Hash;SourceSHA256=(Digest (ConvertTo-Json -InputObject $sourceManifest -Compress));Sources=$sourceManifest
        CorpusSHA256=$corpusHash;Files=$manifest.Count;Bytes=($manifest | Measure-Object Bytes -Sum).Sum
        Runs=$Runs;Repeat=$Repeat;Workers=$Workers;BlockBytes=$BlockBytes;SkipBuild=[bool]$SkipBuild
        Notes='Warm OS cache; one warm-up per configuration. No cache flush. Setup/reading excluded only with --memory. CLI timing includes workers and block partitioning. Source hash and executable hash both recorded; SkipBuild may use stale code.'
    }
    $metadata | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $outputPath 'metadata.json') -Encoding utf8
    $largest = $manifest | Sort-Object Bytes -Descending | Select-Object -First 1
    $singlePath = Join-Path $project $largest.Path
    $configs = @()
    foreach ($memory in @($true,$false)) { foreach ($n in $Workers) {
        $configs += [pscustomobject]@{Dataset='corpus';Path=$corpusPath;Memory=$memory;Threads=$n;Blocks=$false}
        $configs += [pscustomobject]@{Dataset='largest-file';Path=$singlePath;Memory=$memory;Threads=$n;Blocks=($n -gt 1)}
    }}
    function Invoke-Measurement($config,[int]$iterations) {
        $cli = @($config.Path,'--threads',"$($config.Threads)",'--repeat',"$iterations")
        if ($config.Memory) {$cli+='--memory'}
        if ($config.Blocks) {$cli+=@('--block-bytes',"$BlockBytes")}
        $lines = & $exe @cli
        if ($LASTEXITCODE -ne 0) {throw "Benchmark rejected input: $($config.Dataset)"}
        $fields=@{}
        foreach ($line in $lines) {if ($line -match '^([^:]+): (.+)$') {$fields[$matches[1]]=$matches[2]}}
        if ($fields['Parse errors'] -ne '0') {throw 'Parse errors'}
        return $fields
    }
    foreach ($config in $configs) {$null=Invoke-Measurement $config 1}
    $rows=[Collections.Generic.List[object]]::new(); $expected=@{}
    for ($run=1; $run -le $Runs; $run++) {
        # Rotate configurations rather than always warming the same mode first.
        for ($j=0; $j -lt $configs.Count; $j++) {
            $config=$configs[($j+$run-1)%$configs.Count]
            $f=Invoke-Measurement $config $Repeat
            $signature=(@('Files','Bytes','Hands','Players','Actions','Parse errors','Checksum') | ForEach-Object {$f[$_]}) -join '|'
            if ($expected.ContainsKey($config.Dataset)) {if ($expected[$config.Dataset] -ne $signature) {throw 'Counter/checksum mismatch'}} else {$expected[$config.Dataset]=$signature}
            $rows.Add([pscustomobject]@{Dataset=$config.Dataset;Memory=$config.Memory;Threads=$config.Threads;ActualWorkers=[int]$f.Workers;Blocks=$config.Blocks;Run=$run;Seconds=[double]::Parse($f['Elapsed seconds'],[cultureinfo]::InvariantCulture);MBps=[double]::Parse($f['MB/s'],[cultureinfo]::InvariantCulture);Hands=$f.Hands;Players=$f.Players;Actions=$f.Actions;Checksum=$f.Checksum})
        }
        $rows | Export-Csv -LiteralPath (Join-Path $outputPath 'runs.csv') -NoTypeInformation
        Write-Host "Completed run $run/$Runs"
    }
    if ((Digest (ConvertTo-Json -InputObject @(Get-Manifest) -Depth 5 -Compress)) -ne $corpusHash) {throw 'Corpus changed during benchmark'}
    $summary=@($rows | Group-Object Dataset,Memory,Threads,Blocks | ForEach-Object {
        $g=$_.Group; $values=@($g.MBps | Sort-Object); $median=if($values.Count%2){$values[[int][Math]::Floor($values.Count/2)]}else{($values[$values.Count/2-1]+$values[$values.Count/2])/2}; $mean=($values | Measure-Object -Average).Average; $variance=0.0; foreach($v in $values){$variance+=($v-$mean)*($v-$mean)}
        [pscustomobject]@{Dataset=$g[0].Dataset;Memory=$g[0].Memory;Threads=$g[0].Threads;Blocks=$g[0].Blocks;MedianMBps=[Math]::Round($median,2);MinMBps=$values[0];MaxMBps=$values[-1];StdDevMBps=[Math]::Round([Math]::Sqrt($variance/$values.Count),2)}
    })
    $summary | Export-Csv -LiteralPath (Join-Path $outputPath 'summary.csv') -NoTypeInformation
    $summary | Format-Table
    Write-Host "Verified results: $outputPath"
} finally { Pop-Location }
