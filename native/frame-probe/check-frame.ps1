Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class FrameProbeApi {
  [StructLayout(LayoutKind.Sequential)]
  public struct RECT { public int Left, Top, Right, Bottom; }
  [DllImport("user32.dll", CharSet=CharSet.Unicode)]
  public static extern IntPtr FindWindowW(string cls, string title);
  [DllImport("user32.dll")]
  public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)]
  public static extern int GetClassNameW(IntPtr hwnd, StringBuilder name, int capacity);
  [DllImport("user32.dll")]
  public static extern bool GetWindowRect(IntPtr hwnd, out RECT rect);
  [DllImport("user32.dll")]
  public static extern bool IsWindow(IntPtr hwnd);
  [DllImport("user32.dll")]
  public static extern bool SetForegroundWindow(IntPtr hwnd);
  [DllImport("user32.dll", EntryPoint="SendMessageW")]
  public static extern IntPtr SendMessage(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp);
}
'@
$out = Join-Path $PSScriptRoot 'out'
$exe = Join-Path $out 'fedorawin-frame-probe.exe'
if (-not (Test-Path -LiteralPath $exe)) { throw "No real native frame probe: $exe" }
if (-not $env:MSYS2_ROOT) { throw 'MSYS2_ROOT missing.' }
$env:PATH = "$(Join-Path $env:MSYS2_ROOT 'ucrt64\bin');$env:PATH"
$WM_APP = 0x8000
$WM_KEYDOWN = 0x0100
$WM_NCHITTEST = 0x0084
$WM_CLOSE = 0x0010
function Send([IntPtr]$h, [uint32]$message, [int]$w = 0, [IntPtr]$l = [IntPtr]::Zero) {
    return [FrameProbeApi]::SendMessage($h, $message, [IntPtr]::new($w), $l).ToInt64()
}
function Check($condition, $message) { if (-not $condition) { throw $message } }
function Capture($hwnd, $label) {
    [void][FrameProbeApi]::SetForegroundWindow($hwnd)
    Start-Sleep -Milliseconds 250
    $rect = New-Object FrameProbeApi+RECT
    if (-not [FrameProbeApi]::GetWindowRect($hwnd, [ref]$rect)) { throw "No $label bounds" }
    $width = $rect.Right - $rect.Left
    $height = $rect.Bottom - $rect.Top
    Check ($width -ge 550 -and $height -ge 350) "Unexpected $label bounds: $width x $height"
    $bmp = [Drawing.Bitmap]::new($width, $height)
    $graphics = [Drawing.Graphics]::FromImage($bmp)
    try {
      $graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, [Drawing.Size]::new($width,$height))
      $path = Join-Path $out "frame-$label.png"
      $bmp.Save($path, [Drawing.Imaging.ImageFormat]::Png)
      $pixel = $bmp.GetPixel([int]($width/2), 38)
    } finally {
      $graphics.Dispose()
      $bmp.Dispose()
    }
    Check ((Get-Item -LiteralPath $path).Length -gt 5000) "Empty $label screen"
    return [pscustomobject]@{ Path=$path; Pixel=$pixel; Rect=$rect; Width=$width }
}
$stdout = Join-Path $out 'frame.stdout.txt'
$stderr = Join-Path $out 'frame.stderr.txt'
$process = Start-Process -FilePath $exe -WorkingDirectory $out -PassThru -WindowStyle Normal -RedirectStandardOutput $stdout -RedirectStandardError $stderr
$hwnd = [IntPtr]::Zero
try {
  for ($i=0; $i -lt 100; $i++) {
    $process.Refresh()
    if ($process.HasExited) {
      $err = try { Get-Content -LiteralPath $stderr -Raw -ErrorAction Stop } catch { '<unavailable>' }
      throw "Native probe exited early $($process.ExitCode); stderr=$err"
    }
    $hwnd = $process.MainWindowHandle
    if ($hwnd -ne [IntPtr]::Zero) { break }
    Start-Sleep -Milliseconds 200
  }
  if ($hwnd -eq [IntPtr]::Zero) {
    $err = try { Get-Content -LiteralPath $stderr -Raw -ErrorAction Stop } catch { '<unavailable>' }
    $app = try { Get-Process -Id $process.Id -ErrorAction Stop | Select-Object Id,ProcessName,MainWindowHandle,MainWindowTitle | Out-String } catch { '<process unavailable>' }
    throw "Native HWND not found. PID=$($process.Id); process=$app; stderr=$err"
  }
  $owner = [uint32]0
  [void][FrameProbeApi]::GetWindowThreadProcessId($hwnd,[ref]$owner)
  Check ($owner -eq $process.Id) 'Window owned by unexpected PID.'
  $className = [Text.StringBuilder]::new(256)
  [void][FrameProbeApi]::GetClassNameW($hwnd, $className, $className.Capacity)
  Check ($className.ToString() -eq 'FedoraWinNativeFrameProbe') "Unexpected native class: $className"
  Check ((Send $hwnd ($WM_APP+81)) -eq 0) 'Expected original window on launch.'
  Check ((Send $hwnd ($WM_APP+82)) -eq 1) 'Expected original WNDPROC.'
  $original = Capture $hwnd 'original'

  [void](Send $hwnd $WM_KEYDOWN 0x77)
  Check ((Send $hwnd ($WM_APP+81)) -eq 1) 'Attach did not occur.'
  Check ((Send $hwnd ($WM_APP+83)) -eq 1) 'Original Win32 style bits changed.'
  $styled = Capture $hwnd 'attached'
  Check ($styled.Pixel.R -lt 115 -and $styled.Pixel.G -lt 115) 'Dark native header not painted.'
  Check ((Get-FileHash $original.Path -Algorithm SHA256).Hash -ne
         (Get-FileHash $styled.Path -Algorithm SHA256).Hash) 'Original and attached images identical.'
  $x = [int]($styled.Rect.Left + $styled.Width / 2)
  $y = [int]($styled.Rect.Top + 42)
  $param = [int64](($x -band 0xffff) -bor (($y -band 0xffff) -shl 16))
  $drag = Send $hwnd $WM_NCHITTEST 0 ([IntPtr]::new($param))
  Check ($drag -eq 2) "Header dragging failed: hit=$drag"

  [void](Send $hwnd $WM_KEYDOWN 0x77)
  Check ((Send $hwnd ($WM_APP+81)) -eq 0) 'Detach did not occur.'
  Check ((Send $hwnd ($WM_APP+82)) -eq 1) 'Original WNDPROC not restored.'
  Check ((Send $hwnd ($WM_APP+83)) -eq 1) 'Original style bits not preserved.'
  Check (-not $process.HasExited) 'Target app died during detach.'
  $restored = Capture $hwnd 'restored'
  Check ($restored.Pixel.R -gt 115) 'Windows caption not restored.'

  [void](Send $hwnd $WM_KEYDOWN 0x77)
  Check ((Send $hwnd ($WM_APP+81)) -eq 1) 'Reattach failed.'
  [void](Send $hwnd $WM_KEYDOWN 0x77)
  Check ((Send $hwnd ($WM_APP+82)) -eq 1) 'Second restore failed.'
  Write-Host "FRAME PROBE PASS: original -> native headerbar -> original -> native headerbar -> original; same PID=$($process.Id); drag and style checks."
} finally {
  if ($hwnd -ne [IntPtr]::Zero -and [FrameProbeApi]::IsWindow($hwnd)) {
    [void](Send $hwnd $WM_CLOSE)
  }
  if (-not $process.WaitForExit(3000)) {
    Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
  }
}
