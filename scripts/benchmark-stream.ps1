[CmdletBinding()]
param([Parameter(Mandatory)][string]$InputFile,[ValidateRange(1,20)][int]$Runs=3)
$ErrorActionPreference='Stop'
$project=Split-Path $PSScriptRoot -Parent
Push-Location $project
try {
    $exe=(Resolve-Path target/release/pokerforge.exe).Path; $inputPath=(Resolve-Path -LiteralPath $InputFile).Path
    $dir=Join-Path $project ('target/stream-bench/'+(Get-Date -Format 'yyyyMMdd-HHmmss-fff')); $null=New-Item -ItemType Directory -Path $dir
    [ordered]@{InputFile=$inputPath;InputSHA256=(Get-FileHash -LiteralPath $inputPath).Hash;InputBytes=(Get-Item -LiteralPath $inputPath).Length;ExecutableSHA256=(Get-FileHash -LiteralPath $exe).Hash;Runs=$Runs;Culture=[cultureinfo]::CurrentCulture.Name;Notes='Single-thread read + parse; warm cache, no cache flush. Process peak working set high-water mark polled every 50 ms; final interval may be missed.'} | ConvertTo-Json | Set-Content (Join-Path $dir 'metadata.json')
    $rows=[Collections.Generic.List[object]]::new(); $expected=$null; $sizes=@(0,1048576,4194304)
    for($run=1;$run -le $Runs;$run++){for($j=0;$j -lt $sizes.Count;$j++){
        $size=$sizes[($j+$run-1)%$sizes.Count]
        $info=[Diagnostics.ProcessStartInfo]::new($exe); $info.UseShellExecute=$false; $info.CreateNoWindow=$true; $info.RedirectStandardOutput=$true; $info.RedirectStandardError=$true
        $info.ArgumentList.Add($inputPath); if($size){$info.ArgumentList.Add('--stream-bytes'); $info.ArgumentList.Add([string]$size)}
        $p=[Diagnostics.Process]::new(); $p.StartInfo=$info; $null=$p.Start(); $stdout=$p.StandardOutput.ReadToEndAsync(); $stderr=$p.StandardError.ReadToEndAsync(); $peak=0L
        while(-not $p.WaitForExit(50)){try{$p.Refresh(); $peak=[Math]::Max($peak,$p.PeakWorkingSet64)}catch [InvalidOperationException]{}}
        $output=$stdout.GetAwaiter().GetResult(); $errorText=$stderr.GetAwaiter().GetResult(); $code=$p.ExitCode; $p.Dispose(); if($code -ne 0){throw "Parser failed: $errorText"}
        $f=@{}; foreach($line in ($output -split "`n")){if($line.TrimEnd() -match '^([^:]+): (.+)$'){$f[$matches[1]]=$matches[2]}}
        $signature=(@('Files','Bytes','Hands','Players','Actions','Parse errors','Checksum') | ForEach-Object {$f[$_]}) -join '|'
        if($expected -and $expected -ne $signature){throw 'Facts/checksum mismatch'}; $expected=$signature
        if($f['Parse errors'] -ne '0'){throw 'Parse errors'}
        $rows.Add([pscustomobject]@{Run=$run;BufferBytes=$size;MBps=[double]::Parse($f['MB/s'],[cultureinfo]::InvariantCulture);Seconds=[double]::Parse($f['Elapsed seconds'],[cultureinfo]::InvariantCulture);PeakMiB=[Math]::Round($peak/1MB,2);Bytes=$f.Bytes;Hands=$f.Hands;Players=$f.Players;Actions=$f.Actions;Checksum=$f.Checksum})
        $rows | Export-Csv (Join-Path $dir 'runs.csv') -NoTypeInformation
        Write-Host "Run $run, buffer $size : $($f['MB/s']) MB/s, peak $([Math]::Round($peak/1MB,2)) MiB"
    }}
    $summary=@($rows | Group-Object BufferBytes | ForEach-Object {$g=$_.Group; $v=@($g.MBps | Sort-Object); $n=$v.Count; $m=if($n%2){$v[[int][Math]::Floor($n/2)]}else{($v[$n/2-1]+$v[$n/2])/2}; [pscustomobject]@{BufferBytes=$g[0].BufferBytes;MedianMBps=$m;MinMBps=$v[0];MaxMBps=$v[-1];PeakMiB=($g | Measure-Object PeakMiB -Maximum).Maximum}})
    $summary | Export-Csv (Join-Path $dir 'summary.csv') -NoTypeInformation; $summary | Format-Table
    Write-Host "Verified stream results: $dir"
}finally{Pop-Location}
