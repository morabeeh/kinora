# THROWAWAY spike S1 runner: launches s1-renderers once per renderer, measures frames + peak memory.
# Usage:  .\s1-run.ps1                      # all renderers, all cores
#         .\s1-run.ps1 -TwoCores            # simulate a 2-core Lite PC (CPU affinity = cores 0,1)
#         .\s1-run.ps1 -Renderers software  # just one
param(
    [string[]]$Renderers = @('femtovg', 'femtovg-wgpu', 'skia', 'skia-opengl', 'skia-software', 'software'),
    [int]$Seconds = 10,
    [switch]$TwoCores,
    [switch]$NoAutoResize
)
$exe = Join-Path $PSScriptRoot 'target\release\s1-renderers.exe'
$logDir = Join-Path $PSScriptRoot 'target\s1-logs'
New-Item -ItemType Directory -Force $logDir | Out-Null
$results = foreach ($r in $Renderers) {
    $env:SLINT_BACKEND = "winit-$r"
    $env:RUST_LOG = 'info'
    # Slint's built-in counter works with every renderer (our notifier doesn't fire for all of them).
    $env:SLINT_DEBUG_PERFORMANCE = 'refresh_lazy,console'
    $log = Join-Path $logDir "$r.log"
    $args = @('--seconds', $Seconds)
    if (-not $NoAutoResize) { $args += '--auto-resize' }
    $p = Start-Process -FilePath $exe -ArgumentList $args -PassThru -RedirectStandardError $log -RedirectStandardOutput "$log.out"
    if ($TwoCores) { try { $p.ProcessorAffinity = [IntPtr]3 } catch {} }
    $peak = 0
    while (-not $p.HasExited) {
        try { $p.Refresh(); if ($p.WorkingSet64 -gt $peak) { $peak = $p.WorkingSet64 } } catch {}
        Start-Sleep -Milliseconds 200
    }
    $line = Select-String -Path $log -Pattern 'RESULT' | Select-Object -Last 1
    $kv = @{}
    if ($line) { foreach ($m in [regex]::Matches($line.Line, '(\w+)=(\S+)')) { $kv[$m.Groups[1].Value] = $m.Groups[2].Value } }
    $err = (Select-String -Path $log -Pattern 'ERROR|panicked|error' | Select-Object -First 1).Line
    # Average of Slint's per-second fps samples, skipping the first (startup) second.
    $samples = @(Select-String -Path $log -Pattern 'average frames per second: (\d+)' |
        ForEach-Object { [int]$_.Matches[0].Groups[1].Value } | Select-Object -Skip 1)
    $slintFps = if ($samples.Count) { [math]::Round(($samples | Measure-Object -Average).Average) } else { $null }
    $minFps = if ($samples.Count) { ($samples | Measure-Object -Minimum).Minimum } else { $null }
    [pscustomobject]@{
        Renderer   = $r
        FirstFrame = $kv['first_frame_ms']
        SlintFps   = $slintFps
        MinFps     = $minFps
        Fps        = $kv['fps']
        P50ms      = $kv['p50_ms']
        P95ms      = $kv['p95_ms']
        MaxMs      = $kv['max_ms']
        Slow33     = $kv['slow_frames_over_33ms']
        PeakMB     = [math]::Round($peak / 1MB)
        Exit       = $p.ExitCode
        Note       = $err
    }
}
Remove-Item Env:SLINT_BACKEND, Env:SLINT_DEBUG_PERFORMANCE
$results | Format-Table -AutoSize | Out-String -Width 220
