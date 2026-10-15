# THROWAWAY spike S2 runner: plays a file with video backend A, prints mpv/frame stats + peak RSS + CPU.
# Example: .\s2-run.ps1 -File hevc-2160p30.mkv -Backends femtovg,skia-opengl -AutoResize -TwoCores
param(
    [string]$File = 'h264-1080p30.mkv',
    [string[]]$Backends = @('femtovg', 'skia-opengl'),
    [string]$Hwdec = 'auto-safe',
    [int]$Seconds = 10,
    [switch]$AutoResize,
    [switch]$NoDebounce,
    [switch]$TwoCores
)
$media = 'C:\Users\mrabeeh\dev\kinora-tools\media'
$env:Path = "C:\Users\mrabeeh\dev\kinora-tools\mpv;" + $env:Path
$exe = Join-Path $PSScriptRoot 'target\release\s2-video-gl.exe'
$logDir = Join-Path $PSScriptRoot 'target\s2-logs'
New-Item -ItemType Directory -Force $logDir | Out-Null
$rows = foreach ($b in $Backends) {
    $env:SLINT_BACKEND = "winit-$b"
    $env:RUST_LOG = 'info'
    $log = Join-Path $logDir "$b-$File.log"
    $args = @((Join-Path $media $File), '--hwdec', $Hwdec, '--seconds', $Seconds)
    if ($AutoResize) { $args += '--auto-resize' }
    if ($NoDebounce) { $args += '--no-debounce' }
    $p = Start-Process -FilePath $exe -ArgumentList $args -PassThru -RedirectStandardError $log
    if ($TwoCores) { try { $p.ProcessorAffinity = [IntPtr]3 } catch {} }
    $peak = 0; $cpu = [TimeSpan]::Zero; $t0 = Get-Date
    while (-not $p.HasExited) {
        try { $p.Refresh(); if ($p.WorkingSet64 -gt $peak) { $peak = $p.WorkingSet64 }; $cpu = $p.TotalProcessorTime } catch {}
        Start-Sleep -Milliseconds 200
    }
    $wall = ((Get-Date) - $t0).TotalSeconds
    $kv = @{}
    $line = Select-String -Path $log -Pattern 'RESULT' | Select-Object -Last 1
    if ($line) { foreach ($m in [regex]::Matches($line.Line, '(\w+)=(\S+)')) { $kv[$m.Groups[1].Value] = $m.Groups[2].Value } }
    [pscustomobject]@{
        Backend    = $b
        Hwdec      = $kv['hwdec_used']
        CpuPct     = [math]::Round(100 * $cpu.TotalSeconds / $wall / [Environment]::ProcessorCount * $(if ($TwoCores) { [Environment]::ProcessorCount / 2 } else { 1 }), 1)
        FirstMs    = $kv['first_frame_ms']
        VideoFps   = $kv['video_fps']
        FrameP95   = $kv['frame_p95_ms']
        RenderP50  = $kv['render_p50_ms']
        RenderP95  = $kv['render_p95_ms']
        VoDrops    = $kv['vo_drops']
        DecDrops   = $kv['decoder_drops']
        PeakMB     = [math]::Round($peak / 1MB)
    }
}
Remove-Item Env:SLINT_BACKEND
"file=$File hwdec=$Hwdec autoResize=$AutoResize debounce=$(-not $NoDebounce) twoCores=$TwoCores  (CpuPct = % of the cores the process may use)"
$rows | Format-Table -AutoSize | Out-String -Width 250
foreach ($b in $Backends) { Select-String -Path (Join-Path $logDir "$b-$File.log") -Pattern 'ERROR' | Select-Object -First 2 | ForEach-Object { "  $b error: " + $_.Line } }
