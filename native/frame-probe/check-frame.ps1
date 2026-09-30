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
  [StructLayout(LayoutKind.Sequential)]
  public struct POINT { public int X, Y; }
  [StructLayout(LayoutKind.Sequential)]
  public struct WINDOWPLACEMENT { public uint length, flags, showCmd; public POINT ptMinPosition, ptMaxPosition; public RECT rcNormalPosition, rcDevice; }
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowW(string cls, string title);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr hwnd, StringBuilder name, int capacity);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out RECT rect);
  [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern IntPtr GetSystemMenu(IntPtr hwnd, bool revert);
  [DllImport("user32.dll")] public static extern uint GetMenuState(IntPtr menu, uint item, uint flags);
  [DllImport("user32.dll")] public static extern bool IsZoomed(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hwnd, int command);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extraInfo);
  [DllImport("user32.dll")] public static extern bool GetWindowPlacement(IntPtr hwnd, ref WINDOWPLACEMENT placement);
  [DllImport("user32.dll")] public static extern bool SetWindowPlacement(IntPtr hwnd, ref WINDOWPLACEMENT placement);
  [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT point);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll", EntryPoint="SendMessageW")] public static extern IntPtr SendMessage(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp);
  [DllImport("user32.dll", EntryPoint="PostMessageW")] public static extern bool PostMessage(IntPtr hwnd, uint msg, IntPtr wp, IntPtr lp);
}
'@
$out = Join-Path $PSScriptRoot 'out'
$exe = Join-Path $out 'fedorawin-frame-probe.exe'
if (-not (Test-Path -LiteralPath $exe)) { throw "No real native frame probe: $exe" }
if (-not $env:MSYS2_ROOT) { throw 'MSYS2_ROOT missing.' }
$env:PATH = "$(Join-Path $env:MSYS2_ROOT 'ucrt64\bin');$env:PATH"
$WM_APP = 0x8000; $WM_KEYDOWN = 0x0100; $WM_NCHITTEST = 0x0084; $WM_NCLBUTTONDOWN = 0x00A1; $WM_NCLBUTTONDBLCLK = 0x00A3; $WM_NCRBUTTONUP = 0x00A5; $WM_CANCELMODE = 0x001F; $WM_CLOSE = 0x0010; $VK_LWIN = 0x5B; $VK_LEFT = 0x25; $KEYEVENTF_KEYUP = 0x0002
function Send([IntPtr]$h, [uint32]$message, [int]$w = 0, [IntPtr]$l = [IntPtr]::Zero) { return [FrameProbeApi]::SendMessage($h,$message,[IntPtr]::new($w),$l).ToInt64() }
function Check($condition,$message) { if (-not $condition) { throw $message } }
function Capture($hwnd,$label) {
  [void][FrameProbeApi]::SetForegroundWindow($hwnd); Start-Sleep -Milliseconds 250
  $rect=New-Object FrameProbeApi+RECT; if(-not [FrameProbeApi]::GetWindowRect($hwnd,[ref]$rect)){throw "No $label bounds"}
  $width=$rect.Right-$rect.Left; $height=$rect.Bottom-$rect.Top; Check ($width-ge 550 -and $height-ge 350) "Unexpected $label bounds: $width x $height"
  $bmp=[Drawing.Bitmap]::new($width,$height); $graphics=[Drawing.Graphics]::FromImage($bmp)
  try{$graphics.CopyFromScreen($rect.Left,$rect.Top,0,0,[Drawing.Size]::new($width,$height));$path=Join-Path $out "frame-$label.png";$bmp.Save($path,[Drawing.Imaging.ImageFormat]::Png);$pixel=$bmp.GetPixel([int]($width/2),38);$topPixel=$bmp.GetPixel([int]($width/2),6);$maxPixel=$bmp.GetPixel($width-70,23)}finally{$graphics.Dispose();$bmp.Dispose()}
  Check ((Get-Item -LiteralPath $path).Length-gt 5000) "Empty $label screen"; return [pscustomobject]@{Path=$path;Pixel=$pixel;TopPixel=$topPixel;MaxPixel=$maxPixel;Rect=$rect;Width=$width}
}
$stdout=Join-Path $out 'frame.stdout.txt'; $stderr=Join-Path $out 'frame.stderr.txt'
$process=Start-Process -FilePath $exe -WorkingDirectory $out -PassThru -WindowStyle Normal -RedirectStandardOutput $stdout -RedirectStandardError $stderr
$hwnd=[IntPtr]::Zero
$cursorBefore=New-Object FrameProbeApi+POINT
[void][FrameProbeApi]::GetCursorPos([ref]$cursorBefore)
try {
  for($i=0;$i-lt 100;$i++){ $process.Refresh(); if($process.HasExited){$err=try{Get-Content $stderr -Raw}catch{'<unavailable>'};throw "Native probe exited early $($process.ExitCode); stderr=$err"};$hwnd=$process.MainWindowHandle;if($hwnd-ne[IntPtr]::Zero){break};Start-Sleep -Milliseconds 200 }
  if($hwnd-eq[IntPtr]::Zero){throw "Native HWND not found. PID=$($process.Id)"}
  $owner=[uint32]0;[void][FrameProbeApi]::GetWindowThreadProcessId($hwnd,[ref]$owner);Check ($owner-eq$process.Id) 'Window owned by unexpected PID.'
  $className=[Text.StringBuilder]::new(256);[void][FrameProbeApi]::GetClassNameW($hwnd,$className,$className.Capacity);Check ($className.ToString()-eq'FedoraWinNativeFrameProbe') "Unexpected native class: $className"
  Check ((Send $hwnd ($WM_APP+81))-eq 0) 'Expected original window on launch.';Check ((Send $hwnd ($WM_APP+82))-eq 1) 'Expected original WNDPROC.';$original=Capture $hwnd 'original'
  [void](Send $hwnd $WM_KEYDOWN 0x77);Check ((Send $hwnd ($WM_APP+81))-eq 1) 'Attach did not occur.';Check ((Send $hwnd ($WM_APP+83))-eq 1) 'Original Win32 style bits changed.';$styled=Capture $hwnd 'attached';Check ($styled.Pixel.R-lt115 -and $styled.Pixel.G-lt115) 'Dark native header not painted.';Check ($styled.TopPixel.R-lt115 -and $styled.TopPixel.G-lt115 -and $styled.TopPixel.B-lt115) 'Native renderer did not cover the top resize strip.';Check ((Get-FileHash $original.Path).Hash-ne(Get-FileHash $styled.Path).Hash) 'Original and attached images identical.'
  $x=[int]($styled.Rect.Left+$styled.Width/2);$y=[int]($styled.Rect.Top+42);$param=[int64](($x-band 0xffff)-bor(($y-band 0xffff)-shl 16));Check ((Send $hwnd $WM_NCHITTEST 0 ([IntPtr]::new($param)))-eq 2) 'Header dragging failed.'
  $maxX=[int]($styled.Rect.Right-70);$maxParam=[int64](($maxX-band 0xffff)-bor(($y-band 0xffff)-shl 16));$maxHit=Send $hwnd $WM_NCHITTEST 0 ([IntPtr]::new($maxParam));Check ($maxHit-eq 9) "Maximize button hit target failed: hit=$maxHit"
  # Real pointer movement must drive hover feedback without changing hit testing.
  [void][FrameProbeApi]::SetCursorPos($maxX,[int]($styled.Rect.Top+23));Start-Sleep -Milliseconds 250
  $hovered=Capture $hwnd 'hover-max';Check ($hovered.MaxPixel.R-gt$styled.MaxPixel.R) 'Maximize hover did not brighten the native Adwaita control.'
  [void][FrameProbeApi]::SetCursorPos($x,[int]($styled.Rect.Top+140));Start-Sleep -Milliseconds 250
  $unhovered=Capture $hwnd 'hover-cleared';Check ($unhovered.MaxPixel.ToArgb()-eq$styled.MaxPixel.ToArgb()) 'Maximize hover did not restore after pointer leave.'
  # The replacement header must not discard Windows' real Alt+Space/system-menu command source.
  $menu=[FrameProbeApi]::GetSystemMenu($hwnd,$false);Check ($menu-ne[IntPtr]::Zero) 'Native system menu missing while frame is attached.'
  Check ([FrameProbeApi]::GetMenuState($menu,0xF060,0x00000000)-ne[uint32]::MaxValue) 'Native Close system-menu command missing.'
  # A real caption right-click must still enter Windows' native system-menu loop.
  # Post instead of SendMessage because DefWindowProc owns the modal menu loop.
  Check ([FrameProbeApi]::PostMessage($hwnd,$WM_NCRBUTTONUP,[IntPtr]::new(2),[IntPtr]::new($param))) 'Could not post native caption right-click.'
  $menuWindow=[IntPtr]::Zero
  for($i=0;$i-lt 30;$i++){ $menuWindow=[FrameProbeApi]::FindWindowW('#32768',$null); if($menuWindow-ne[IntPtr]::Zero){break}; Start-Sleep -Milliseconds 50 }
  Check ($menuWindow-ne[IntPtr]::Zero) 'Caption right-click did not open the Windows-owned system menu.'
  Check ([FrameProbeApi]::PostMessage($hwnd,$WM_CANCELMODE,[IntPtr]::Zero,[IntPtr]::Zero)) 'Could not cancel native system menu.'
  for($i=0;$i-lt 30 -and [FrameProbeApi]::FindWindowW('#32768',$null)-ne[IntPtr]::Zero;$i++){ Start-Sleep -Milliseconds 50 }
  Check ([FrameProbeApi]::FindWindowW('#32768',$null)-eq[IntPtr]::Zero) 'Windows-owned system menu did not close cleanly.'
  Check ((Send $hwnd ($WM_APP+83))-eq 1) 'Caption context menu changed original style bits.'
  # Exercise the real Windows shell Win+Left Snap accelerator while our frame owns non-client hit testing.
  # Save Windows' full placement state: SetWindowPos alone does not clear Snap's restore metadata.
  $placementBefore=New-Object FrameProbeApi+WINDOWPLACEMENT
  $placementBefore.length=[Runtime.InteropServices.Marshal]::SizeOf([type][FrameProbeApi+WINDOWPLACEMENT])
  Check ([FrameProbeApi]::GetWindowPlacement($hwnd,[ref]$placementBefore)) 'Could not capture placement before Win+Left Snap.'
  [void][FrameProbeApi]::SetForegroundWindow($hwnd); Start-Sleep -Milliseconds 200
  Check ([FrameProbeApi]::GetForegroundWindow()-eq$hwnd) 'Native probe did not own foreground before Win+Left Snap.'
  [FrameProbeApi]::keybd_event($VK_LWIN,0,0,[UIntPtr]::Zero)
  [FrameProbeApi]::keybd_event($VK_LEFT,0,0,[UIntPtr]::Zero)
  [FrameProbeApi]::keybd_event($VK_LEFT,0,$KEYEVENTF_KEYUP,[UIntPtr]::Zero)
  [FrameProbeApi]::keybd_event($VK_LWIN,0,$KEYEVENTF_KEYUP,[UIntPtr]::Zero)
  Start-Sleep -Milliseconds 600
  $snappedRect=New-Object FrameProbeApi+RECT
  Check ([FrameProbeApi]::GetWindowRect($hwnd,[ref]$snappedRect)) 'Could not read bounds after Win+Left Snap.'
  $snapChanged=[Math]::Abs($snappedRect.Left-$styled.Rect.Left)-gt20 -or [Math]::Abs($snappedRect.Top-$styled.Rect.Top)-gt20 -or [Math]::Abs($snappedRect.Right-$styled.Rect.Right)-gt20 -or [Math]::Abs($snappedRect.Bottom-$styled.Rect.Bottom)-gt20
  Check $snapChanged 'Win+Left did not change the attached window bounds through Windows Snap.'
  Check ((Send $hwnd ($WM_APP+83))-eq 1) 'Win+Left Snap changed original style bits.'
  # Restore the complete pre-Snap placement, including Windows' normal-position metadata,
  # before continuing the exact detach/rollback proof.
  Check ([FrameProbeApi]::SetWindowPlacement($hwnd,[ref]$placementBefore)) 'Could not restore pre-Snap WINDOWPLACEMENT.'
  Start-Sleep -Milliseconds 350
  $restoredSnapRect=New-Object FrameProbeApi+RECT
  Check ([FrameProbeApi]::GetWindowRect($hwnd,[ref]$restoredSnapRect)) 'Could not read restored pre-Snap bounds.'
  Check ($restoredSnapRect.Left-eq$styled.Rect.Left -and $restoredSnapRect.Top-eq$styled.Rect.Top -and $restoredSnapRect.Right-eq$styled.Rect.Right -and $restoredSnapRect.Bottom-eq$styled.Rect.Bottom) 'Pre-Snap window placement was not restored exactly.'
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
  # Finally exercise the actual close circle, rather than closing the test only from the harness.
  [void](Send $hwnd $WM_KEYDOWN 0x77);Check ((Send $hwnd ($WM_APP+81))-eq 1) 'Final attach before native close failed.'
  $closeX=[int]($styled.Rect.Right-30);$closeParam=[int64](($closeX-band 0xffff)-bor(($y-band 0xffff)-shl 16));Check ((Send $hwnd $WM_NCHITTEST 0 ([IntPtr]::new($closeParam)))-eq 20) 'Native close button hit target failed.'
  [void](Send $hwnd $WM_NCLBUTTONDOWN 20 ([IntPtr]::new($closeParam)))
  Check ($process.WaitForExit(3000)) 'Native close button did not terminate the disposable window process.'
  $process.Refresh();Check ($process.ExitCode-eq 0) "Native close exited with code $($process.ExitCode)."
  Check (-not [FrameProbeApi]::IsWindow($hwnd)) 'Native close left a live HWND.'
  Write-Host "FRAME PROBE PASS: attach -> native system menu -> Win+Left Snap -> Windows maximize/minimize/double-click -> exact rollback -> reattach -> rollback -> native close; same PID=$($process.Id)."
} finally { [void][FrameProbeApi]::SetCursorPos($cursorBefore.X,$cursorBefore.Y);if($hwnd-ne[IntPtr]::Zero -and [FrameProbeApi]::IsWindow($hwnd)){[void](Send $hwnd $WM_CLOSE)};if(-not $process.WaitForExit(3000)){Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue} }
