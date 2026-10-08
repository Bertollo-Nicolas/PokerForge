[CmdletBinding()]
param(
    [string]$Corpus = 'data',
    [ValidateRange(1,100000)][int]$Repeat = 150,
    [ValidateRange(1,256)][int]$Threads = 1
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'WPR requires an elevated terminal. Open PowerShell as administrator and run this script again.'
}
$project = Split-Path $PSScriptRoot -Parent
Push-Location $project
try {
    $path = (Resolve-Path -LiteralPath $Corpus).Path
    & cargo build --profile profiling --locked
    if ($LASTEXITCODE -ne 0) {throw 'Build failed'}
    $dir = Join-Path $project ('target/profiles/' + (Get-Date -Format 'yyyyMMdd-HHmmss-fff'))
    $null = New-Item -ItemType Directory -Path $dir
    $exe = Join-Path $project 'target/profiling/pokerforge.exe'
    $trace = Join-Path $dir 'cpu.etl'
    # Do not cancel any pre-existing recorder. A failed start leaves it untouched.
    & wpr -start CPU -filemode
    if ($LASTEXITCODE -ne 0) {throw 'WPR start failed. No existing recorder was stopped.'}
    $runExit = -1
    try {
        & $exe $path --memory --repeat $Repeat --threads $Threads | Tee-Object -FilePath (Join-Path $dir 'run.txt')
        $runExit = $LASTEXITCODE
    } finally {
        & wpr -stop $trace
        if ($LASTEXITCODE -ne 0) {Write-Warning 'WPR stop failed. Check wpr -status in this elevated terminal.'}
    }
    [ordered]@{
        CapturedUTC=[DateTime]::UtcNow.ToString('o');Executable=$exe
        ExecutableSHA256=(Get-FileHash -LiteralPath $exe).Hash
        Symbols=(Join-Path $project 'target/profiling/pokerforge.pdb')
        Corpus=$path;Repeat=$Repeat;Threads=$Threads;RunExitCode=$runExit
        Note='System-wide CPU samples. Analyze only pokerforge.exe. Trace stays local under ignored target/. Keep the matching EXE and PDB; rebuilding can invalidate symbol matching.'
    } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $dir 'metadata.json') -Encoding utf8
    Copy-Item -LiteralPath $exe -Destination $dir
    Copy-Item -LiteralPath (Join-Path $project 'target/profiling/pokerforge.pdb') -Destination $dir
    Write-Host "Capture directory: $dir"
    if ($runExit -ne 0) {throw 'Parser failed; inspect run.txt before interpreting the trace.'}
    if (-not (Test-Path -LiteralPath $trace)) {throw 'No ETL trace was created.'}
} finally {Pop-Location}
