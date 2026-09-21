Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class AdwaitaProbe {
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll", CharSet=CharSet.Unicode)]
    public static extern IntPtr FindWindowW(string className, string title);
    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32.dll")]
    public static extern bool GetWindowRect(IntPtr hwnd, out RECT rect);
    [DllImport("user32.dll")]
    public static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll")]
    public static extern bool SetForegroundWindow(IntPtr hwnd);
}
'@
$out = Join-Path $PSScriptRoot 'out'
$exe = Join-Path $out 'fedorawin-adwaita.exe'
if (-not (Test-Path -LiteralPath $exe)) { throw "Native executable absent: $exe" }
$env:PATH = "C:\msys64\ucrt64\bin;$env:PATH"
$title = 'FedoraWin Adwaita Native Probe'
foreach ($theme in @('dark','light')) {
    $env:FEDORAWIN_ADWAITA_THEME = $theme
    $process = Start-Process -FilePath $exe -WorkingDirectory $out -PassThru
    try {
        $hwnd = [IntPtr]::Zero
        for ($attempt=0; $attempt -lt 150; $attempt++) {
            if ($process.HasExited) { throw "Adwaita $theme exited: $($process.ExitCode)" }
            $candidate = [AdwaitaProbe]::FindWindowW($null, $title)
            if ($candidate -ne [IntPtr]::Zero -and [AdwaitaProbe]::IsWindowVisible($candidate)) {
                $owner = [uint32]0
                [void][AdwaitaProbe]::GetWindowThreadProcessId($candidate, [ref]$owner)
                if ($owner -eq $process.Id) { $hwnd=$candidate; break }
            }
            Start-Sleep -Milliseconds 200
        }
        if ($hwnd -eq [IntPtr]::Zero) { throw "No visible real Adwaita HWND ($theme)." }
        [void][AdwaitaProbe]::SetForegroundWindow($hwnd)
        Start-Sleep -Milliseconds 900
        $rect = New-Object AdwaitaProbe+RECT
        if (-not [AdwaitaProbe]::GetWindowRect($hwnd,[ref]$rect)) { throw 'Window bounds unavailable.' }
        $width=$rect.Right-$rect.Left
        $height=$rect.Bottom-$rect.Top
        if ($width -lt 300 -or $height -lt 250) { throw 'Invalid GTK4 window bounds.' }
        $bitmap=[Drawing.Bitmap]::new($width,$height)
        $graphics=[Drawing.Graphics]::FromImage($bitmap)
        try {
            $graphics.CopyFromScreen($rect.Left,$rect.Top,0,0,[Drawing.Size]::new($width,$height))
            $path=Join-Path $out "adwaita-$theme.png"
            $bitmap.Save($path,[Drawing.Imaging.ImageFormat]::Png)
        } finally {
            $graphics.Dispose()
            $bitmap.Dispose()
        }
        if ((Get-Item -LiteralPath $path).Length -lt 4000) { throw "Capture uninitialized ($theme)." }
        Write-Host "ADWAITA: real $theme Win32 HWND captured ($width x $height)."
    } finally {
        if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
    }
}
Remove-Item Env:FEDORAWIN_ADWAITA_THEME -ErrorAction SilentlyContinue
