# PowerShell 7 / Windows. Synthetic scale test; generates data only under target/.
[CmdletBinding()]
param(
    [string]$Corpus='data',
    [int[]]$SizesMiB=@(256,1024,2048),
    [ValidateRange(1,20)][int]$Runs=3
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
$project=Split-Path $PSScriptRoot -Parent
Push-Location $project
try {
    if(@($SizesMiB | Where-Object {$_ -lt 1 -or $_ -gt 4096}).Count -gt 0){throw 'Sizes must be 1..4096 MiB'}
    $exe=(Resolve-Path target/release/pokerforge.exe).Path
    $source=(Resolve-Path -LiteralPath $Corpus).Path
    $dir=Join-Path $project ('target/scale/'+(Get-Date -Format 'yyyyMMdd-HHmmss-fff'))
    $null=New-Item -ItemType Directory -Path $dir
    $files=@(Get-ChildItem -LiteralPath $source -File -Recurse | Where-Object {$_.Extension -ieq '.txt' -and -not ($_.Attributes -band [IO.FileAttributes]::ReparsePoint)} | Sort-Object FullName)
    if(-not $files.Count){throw 'No TXT files'}
    $stream=[IO.MemoryStream]::new()
    $manifest=@()
    foreach($file in $files){
        $b=[IO.File]::ReadAllBytes($file.FullName); $offset=0
        if($b.Length -ge 3 -and $b[0] -eq 239 -and $b[1] -eq 187 -and $b[2] -eq 191){$offset=3}
        $stream.Write($b,$offset,$b.Length-$offset); $stream.WriteByte(10)
        $manifest+=[pscustomobject]@{Path=[IO.Path]::GetRelativePath($project,$file.FullName);Bytes=$file.Length;SHA256=(Get-FileHash -LiteralPath $file.FullName).Hash}
    }
    $base=$stream.ToArray(); $stream.Dispose()
    $basePath=Join-Path $dir 'base.txt'; [IO.File]::WriteAllBytes($basePath,$base)
    function Fields($lines){$f=@{}; foreach($line in $lines){$line=$line.TrimEnd(); if($line -match '^([^:]+): (.+)$'){$f[$matches[1]]=$matches[2]}}; return $f}
    $baseline=Fields (& $exe $basePath --memory)
    if($LASTEXITCODE -ne 0 -or $baseline['Parse errors'] -ne '0'){throw 'Synthetic base rejected'}
    [ordered]@{CreatedUTC=[DateTime]::UtcNow.ToString('o');Culture=[cultureinfo]::CurrentCulture.Name;SourceFiles=$manifest;BaseBytes=$base.Length;BaseSHA256=(Get-FileHash $basePath).Hash;BaseFacts=$baseline;ExecutableSHA256=(Get-FileHash $exe).Hash;Runs=$Runs;SizesMiB=$SizesMiB;Notes='Synthetic concatenation, source BOMs removed, newline appended per file. Whole base repeated. Not unique poker hands. Warm OS cache; no disk cache flush. Peak working set is OS high-water mark sampled every 50 ms until exit; last interval may be missed. Includes preload even with --memory. PowerShell generator memory excluded.'} | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $dir 'metadata.json') -Encoding utf8
    $rows=[Collections.Generic.List[object]]::new()
    $generated=[Collections.Generic.List[object]]::new()
    foreach($size in $SizesMiB){
        $copies=[int][Math]::Ceiling(($size*1MB)/[double]$base.Length)
        $inputPath=Join-Path $dir ("synthetic-$size-MiB.txt")
        $out=[IO.File]::Open($inputPath,[IO.FileMode]::CreateNew)
        try {for($i=0;$i -lt $copies;$i++){$out.Write($base,0,$base.Length)}} finally {$out.Dispose()}
        $bytes=[long]$base.Length*$copies
        $generated.Add([pscustomobject]@{File=[IO.Path]::GetFileName($inputPath);Copies=$copies;Bytes=$bytes;SHA256=(Get-FileHash $inputPath).Hash})
        $baseSum=[Numerics.BigInteger]::Parse('0'+$baseline.Checksum,[Globalization.NumberStyles]::AllowHexSpecifier)
        $expectedSum=($baseSum*$copies)%([Numerics.BigInteger]::Pow(2,64))
        $configs=@(@{Memory=$false;Threads=1},@{Memory=$false;Threads=4},@{Memory=$true;Threads=1},@{Memory=$true;Threads=4})
        for($run=1;$run -le $Runs;$run++){ for($j=0;$j -lt $configs.Count;$j++){
            $config=$configs[($j+$run-1)%$configs.Count]
            $info=[Diagnostics.ProcessStartInfo]::new($exe); $info.UseShellExecute=$false; $info.CreateNoWindow=$true; $info.RedirectStandardOutput=$true; $info.RedirectStandardError=$true
            $info.ArgumentList.Add($inputPath); $info.ArgumentList.Add('--threads'); $info.ArgumentList.Add([string]$config.Threads)
            if($config.Memory){$info.ArgumentList.Add('--memory')}
            if($config.Threads -gt 1){$info.ArgumentList.Add('--block-bytes'); $info.ArgumentList.Add('1048576')}
            $process=[Diagnostics.Process]::new(); $process.StartInfo=$info
            $null=$process.Start(); $stdout=$process.StandardOutput.ReadToEndAsync(); $stderr=$process.StandardError.ReadToEndAsync()
            $peak=0L; $samples=0
            while(-not $process.WaitForExit(50)){
                try {$process.Refresh(); $peak=[Math]::Max($peak,$process.PeakWorkingSet64); $samples++} catch [InvalidOperationException] {}
            }
            $text=$stdout.GetAwaiter().GetResult(); $errorText=$stderr.GetAwaiter().GetResult(); $exitCode=$process.ExitCode; $process.Dispose()
            if($exitCode -ne 0){throw "Parsing failed: $errorText"}
            $f=Fields ($text -split "`n")
            foreach($key in @('Hands','Players','Actions')){if([long]$f[$key] -ne [long]$baseline[$key]*$copies){throw "Incorrect $key"}}
            $actualSum=[Numerics.BigInteger]::Parse('0'+$f.Checksum,[Globalization.NumberStyles]::AllowHexSpecifier)
            if($actualSum -ne $expectedSum -or [long]$f.Bytes -ne $bytes -or $f['Parse errors'] -ne '0'){throw 'Checksum/bytes/errors mismatch'}
            $rows.Add([pscustomobject]@{TargetMiB=$size;Bytes=$bytes;Copies=$copies;Run=$run;Memory=$config.Memory;Threads=$config.Threads;Seconds=[double]::Parse($f['Elapsed seconds'],[cultureinfo]::InvariantCulture);MBps=[double]::Parse($f['MB/s'],[cultureinfo]::InvariantCulture);PeakWorkingSetBytes=$peak;MemorySamples=$samples;Hands=$f.Hands;Actions=$f.Actions;Checksum=$f.Checksum})
            $rows | Export-Csv -LiteralPath (Join-Path $dir 'runs.csv') -NoTypeInformation
            Write-Host "Size $size MiB, run $run, memory=$($config.Memory), workers=$($config.Threads): $($f['MB/s']) MB/s, peak $([Math]::Round($peak/1MB)) MiB"
        }}
    }
    $generated | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $dir 'generated.json') -Encoding utf8
    $summary=@($rows | Group-Object TargetMiB,Memory,Threads | ForEach-Object {
        $g=$_.Group; $speeds=@($g.MBps | Sort-Object)
        $median=if($speeds.Count%2){$speeds[[int][Math]::Floor($speeds.Count/2)]}else{($speeds[$speeds.Count/2-1]+$speeds[$speeds.Count/2])/2}
        [pscustomobject]@{TargetMiB=$g[0].TargetMiB;ActualBytes=$g[0].Bytes;Memory=$g[0].Memory;Threads=$g[0].Threads;MedianMBps=$median;MinMBps=$speeds[0];MaxMBps=$speeds[-1];MaxObservedPeakMiB=[Math]::Round(($g | Measure-Object PeakWorkingSetBytes -Maximum).Maximum/1MB,2)}
    })
    $summary | Export-Csv -LiteralPath (Join-Path $dir 'summary.csv') -NoTypeInformation
    Write-Host "Verified scale results: $dir"
} finally {Pop-Location}
