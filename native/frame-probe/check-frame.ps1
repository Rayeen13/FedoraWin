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
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowW(string cls, string title);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr hwnd, StringBuilder name, int capacity);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out RECT rect);
  [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern bool IsZoomed(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hwnd, int command);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
  [DllImport("user32.dll", EntryPoint="SendMessageW")] public static extern IntPtr SendMessage(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp);
}
'@
$out = Join-Path $PSScriptRoot 'out'
$exe = Join-Path $out 'fedorawin-frame-probe.exe'
if (-not (Test-Path -LiteralPath $exe)) { throw "No real native frame probe: $exe" }
if (-not $env:MSYS2_ROOT) { throw 'MSYS2_ROOT missing.' }
$env:PATH = "$(Join-Path $env:MSYS2_ROOT 'ucrt64\bin');$env:PATH"
$WM_APP = 0x8000; $WM_KEYDOWN = 0x0100; $WM_NCHITTEST = 0x0084; $WM_NCLBUTTONDOWN = 0x00A1; $WM_NCLBUTTONDBLCLK = 0x00A3; $WM_CLOSE = 0x0010
function Send([IntPtr]$h, [uint32]$message, [int]$w = 0, [IntPtr]$l = [IntPtr]::Zero) { return [FrameProbeApi]::SendMessage($h,$message,[IntPtr]::new($w),$l).ToInt64() }
function Check($condition,$message) { if (-not $condition) { throw $message } }
function Capture($hwnd,$label) {
  [void][FrameProbeApi]::SetForegroundWindow($hwnd); Start-Sleep -Milliseconds 250
  $rect=New-Object FrameProbeApi+RECT; if(-not [FrameProbeApi]::GetWindowRect($hwnd,[ref]$rect)){throw "No $label bounds"}
  $width=$rect.Right-$rect.Left; $height=$rect.Bottom-$rect.Top; Check ($width-ge 550 -and $height-ge 350) "Unexpected $label bounds: $width x $height"
  $bmp=[Drawing.Bitmap]::new($width,$height); $graphics=[Drawing.Graphics]::FromImage($bmp)
  try{$graphics.CopyFromScreen($rect.Left,$rect.Top,0,0,[Drawing.Size]::new($width,$height));$path=Join-Path $out "frame-$label.png";$bmp.Save($path,[Drawing.Imaging.ImageFormat]::Png);$pixel=$bmp.GetPixel([int]($width/2),38)}finally{$graphics.Dispose();$bmp.Dispose()}
  Check ((Get-Item -LiteralPath $path).Length-gt 5000) "Empty $label screen"; return [pscustomobject]@{Path=$path;Pixel=$pixel;Rect=$rect;Width=$width}
}
$stdout=Join-Path $out 'frame.stdout.txt'; $stderr=Join-Path $out 'frame.stderr.txt'
$process=Start-Process -FilePath $exe -WorkingDirectory $out -PassThru -WindowStyle Normal -RedirectStandardOutput $stdout -RedirectStandardError $stderr
$hwnd=[IntPtr]::Zero
try {
  for($i=0;$i-lt 100;$i++){ $process.Refresh(); if($process.HasExited){$err=try{Get-Content $stderr -Raw}catch{'<unavailable>'};throw "Native probe exited early $($process.ExitCode); stderr=$err"};$hwnd=$process.MainWindowHandle;if($hwnd-ne[IntPtr]::Zero){break};Start-Sleep -Milliseconds 200 }
  if($hwnd-eq[IntPtr]::Zero){throw "Native HWND not found. PID=$($process.Id)"}
  $owner=[uint32]0;[void][FrameProbeApi]::GetWindowThreadProcessId($hwnd,[ref]$owner);Check ($owner-eq$process.Id) 'Window owned by unexpected PID.'
  $className=[Text.StringBuilder]::new(256);[void][FrameProbeApi]::GetClassNameW($hwnd,$className,$className.Capacity);Check ($className.ToString()-eq'FedoraWinNativeFrameProbe') "Unexpected native class: $className"
  Check ((Send $hwnd ($WM_APP+81))-eq 0) 'Expected original window on launch.';Check ((Send $hwnd ($WM_APP+82))-eq 1) 'Expected original WNDPROC.';$original=Capture $hwnd 'original'
  [void](Send $hwnd $WM_KEYDOWN 0x77);Check ((Send $hwnd ($WM_APP+81))-eq 1) 'Attach did not occur.';Check ((Send $hwnd ($WM_APP+83))-eq 1) 'Original Win32 style bits changed.';$styled=Capture $hwnd 'attached';Check ($styled.Pixel.R-lt115 -and $styled.Pixel.G-lt115) 'Dark native header not painted.';Check ((Get-FileHash $original.Path).Hash-ne(Get-FileHash $styled.Path).Hash) 'Original and attached images identical.'
  $x=[int]($styled.Rect.Left+$styled.Width/2);$y=[int]($styled.Rect.Top+42);$param=[int64](($x-band 0xffff)-bor(($y-band 0xffff)-shl 16));Check ((Send $hwnd $WM_NCHITTEST 0 ([IntPtr]::new($param)))-eq 2) 'Header dragging failed.'
  $maxX=[int]($styled.Rect.Right-70);$maxParam=[int64](($maxX-band 0xffff)-bor(($y-band 0xffff)-shl 16));$maxHit=Send $hwnd $WM_NCHITTEST 0 ([IntPtr]::new($maxParam));Check ($maxHit-eq 9) "Maximize button hit target failed: hit=$maxHit"
  $leftX=[int]($styled.Rect.Left+3);$leftParam=[int64](($leftX-band 0xffff)-bor(($y-band 0xffff)-shl 16));Check ((Send $hwnd $WM_NCHITTEST 0 ([IntPtr]::new($leftParam)))-eq 10) 'Left resize border hit target failed.'
  # Exercise the actual Windows-owned maximize/restore system command path, not only hit-test geometry.
  [void](Send $hwnd $WM_NCLBUTTONDOWN 9 ([IntPtr]::new($maxParam)));Start-Sleep -Milliseconds 250;Check ([FrameProbeApi]::IsZoomed($hwnd)) 'Native maximize control did not maximize through WM_SYSCOMMAND.'
  [void](Send $hwnd $WM_NCLBUTTONDOWN 9 ([IntPtr]::new($maxParam)));Start-Sleep -Milliseconds 250;Check (-not [FrameProbeApi]::IsZoomed($hwnd)) 'Native maximize control did not restore through WM_SYSCOMMAND.'
  Check ((Send $hwnd ($WM_APP+83))-eq 1) 'System-command round trip changed original style bits.'
  # Caption double-click must still reach DefWindowProc and toggle the real Win32 window state.
  [void](Send $hwnd $WM_NCLBUTTONDBLCLK 2 ([IntPtr]::new($param)));Start-Sleep -Milliseconds 250;Check ([FrameProbeApi]::IsZoomed($hwnd)) 'Native caption double-click did not maximize.'
  [void](Send $hwnd $WM_NCLBUTTONDBLCLK 2 ([IntPtr]::new($param)));Start-Sleep -Milliseconds 250;Check (-not [FrameProbeApi]::IsZoomed($hwnd)) 'Native caption double-click did not restore.'
  Check ((Send $hwnd ($WM_APP+83))-eq 1) 'Caption double-click changed original style bits.'
  # Verify the adjacent GNOME-style minimize circle calls Windows-owned SC_MINIMIZE.
  $minX=[int]($styled.Rect.Right-108);$minParam=[int64](($minX-band 0xffff)-bor(($y-band 0xffff)-shl 16));Check ((Send $hwnd $WM_NCHITTEST 0 ([IntPtr]::new($minParam)))-eq 8) 'Minimize button hit target failed.'
  [void](Send $hwnd $WM_NCLBUTTONDOWN 8 ([IntPtr]::new($minParam)));Start-Sleep -Milliseconds 250;Check ([FrameProbeApi]::IsIconic($hwnd)) 'Native minimize control did not minimize through WM_SYSCOMMAND.'
  [void][FrameProbeApi]::ShowWindow($hwnd,9);Start-Sleep -Milliseconds 250;Check (-not [FrameProbeApi]::IsIconic($hwnd)) 'Original Win32 window did not restore from minimize.'
  Check ((Send $hwnd ($WM_APP+83))-eq 1) 'Minimize/restore changed original style bits.'
  [void](Send $hwnd $WM_KEYDOWN 0x77);Check ((Send $hwnd ($WM_APP+81))-eq 0) 'Detach did not occur.';Check ((Send $hwnd ($WM_APP+82))-eq 1) 'Original WNDPROC not restored.';Check ((Send $hwnd ($WM_APP+83))-eq 1) 'Original style bits not preserved.';Check (-not $process.HasExited) 'Target app died during detach.'
  $restored=Capture $hwnd 'restored';Check ($restored.Pixel.R-gt115) 'Windows caption not restored.';Check ((Get-FileHash $original.Path).Hash-eq(Get-FileHash $restored.Path).Hash) 'Restored frame differs pixel-for-pixel from original.'
  [void](Send $hwnd $WM_KEYDOWN 0x77);Check ((Send $hwnd ($WM_APP+81))-eq 1) 'Reattach failed.';[void](Send $hwnd $WM_KEYDOWN 0x77);Check ((Send $hwnd ($WM_APP+82))-eq 1) 'Second restore failed.'
  Write-Host "FRAME PROBE PASS: attach -> Windows system-command maximize/restore, caption double-click, and minimize/restore -> exact rollback -> reattach -> rollback; same PID=$($process.Id)."
} finally { if($hwnd-ne[IntPtr]::Zero -and [FrameProbeApi]::IsWindow($hwnd)){[void](Send $hwnd $WM_CLOSE)};if(-not $process.WaitForExit(3000)){Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue} }
