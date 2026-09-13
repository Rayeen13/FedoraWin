param(
    [switch]$SafeMode,
    [switch]$Diagnostic
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$script:Root = Split-Path -Parent $MyInvocation.MyCommand.Path
$script:AppCatalogPath = Join-Path $script:Root 'shell\AppCatalog.ps1'
if (-not (Test-Path -LiteralPath $script:AppCatalogPath)) {
    throw "Required app catalog module is missing: $script:AppCatalogPath"
}
. $script:AppCatalogPath
$script:RuntimeDir = Join-Path $env:LOCALAPPDATA 'FedoraWin'
$script:StatePath = Join-Path $script:RuntimeDir 'state.json'
$script:RestoreRequestPath = Join-Path $script:RuntimeDir 'restore.request'
$script:PidPath = Join-Path $script:RuntimeDir 'pid.txt'
$script:ConfigPath = Join-Path $script:Root 'config.json'
$script:WallpaperPath = Join-Path $script:RuntimeDir 'fedora-win-blue.png'
$script:Exiting = $false
$script:ActivitiesWindow = $null
$script:CalendarWindow = $null
$script:QuickWindow = $null
$script:AppearanceWindow = $null
$script:PowerWindow = $null
$script:DockWindow = $null
$script:DockTimer = $null
$script:DockSignature = ''
$script:DockMode = 'both'
$script:DockPosition = 'bottom'
$script:DockTopmost = $true
$script:DockIconSize = 48
$script:DockFavorites = @()
$script:TaskbarWasVisible = $true
$script:OriginalWallpaper = $null
$script:HotkeyRegistered = $false
$script:HotkeyId = 44050
$script:SuperLeftHotkeyRegistered = $false
$script:SuperRightHotkeyRegistered = $false
$script:SuperLeftHotkeyId = 44051
$script:SuperRightHotkeyId = 44052
$script:PanelHandle = [IntPtr]::Zero
$script:PanelWindow = $null
$script:TrayIcon = $null
$script:InstalledApps = @()
$script:OriginalWorkArea = $null
$script:Mutex = $null
$script:ExitCode = 0
$script:LogPath = Join-Path $script:RuntimeDir 'FedoraWin.log'
$script:InstalledAppsLoaded = $false
$script:CalendarMonth = (Get-Date -Day 1).Date
$script:ActivitiesMode = 'overview'
$script:AppGridPopulated = $false
$script:AppGridPage = 0
$script:AppGridPageSize = 30
$script:AppGridPageCount = 1
$script:SuppressVolumeEvent = $false
$script:SuppressBrightnessEvent = $false
$script:AudioAvailable = $false
$script:DesktopIconsWereVisible = $true
$script:ActivitiesSearchBox = $null
$script:WorkspacePresenter = $null
$script:FrameManager = $null
$script:FrameTimer = $null
$script:AppBarRegistered = $false
$script:ThemeMode = 'dark'
$script:AccentName = 'blue'
$script:AccentHex = '#3584E4'
$script:SettingsPath = Join-Path $script:RuntimeDir 'settings.json'
$script:WinRtAsTaskGeneric = $null
$script:RadioApiAvailable = $false
$script:RadioAccess = $null

# Single-instance guard. A named mutex is released automatically if the process crashes.
$createdNew = $false
$script:Mutex = [System.Threading.Mutex]::new($true, 'Local\FedoraWinShell', [ref]$createdNew)
if (-not $createdNew) {
    exit 0
}

New-Item -ItemType Directory -Force -Path $script:RuntimeDir | Out-Null

function Write-FedoraWinLog {
    param([string]$Level, [string]$Message)
    try {
        $line = '{0} [{1}] {2}' -f (Get-Date).ToString('yyyy-MM-dd HH:mm:ss.fff'), $Level.ToUpperInvariant(), $Message
        Add-Content -LiteralPath $script:LogPath -Value $line -Encoding UTF8
        if ($Diagnostic) { Write-Host $line }
    } catch { }
}

Write-FedoraWinLog 'info' ('Starting FedoraWin 0.4.0-dev. SafeMode={0}; PID={1}; PS={2}; Apartment={3}' -f [bool]$SafeMode, $PID, $PSVersionTable.PSVersion, [Threading.Thread]::CurrentThread.ApartmentState)

# Load WPF before installing the dispatcher guard. In 0.3.0 the guard ran
# before PresentationFramework/WindowsBase were loaded, so the Dispatcher
# type could be unavailable on Windows PowerShell 5.1.
Add-Type -AssemblyName PresentationFramework
Add-Type -AssemblyName PresentationCore
Add-Type -AssemblyName WindowsBase
Add-Type -AssemblyName System.Xaml
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
try {
    Add-Type -AssemblyName System.Runtime.WindowsRuntime
    [Windows.Devices.Radios.Radio,Windows.System.Devices,ContentType=WindowsRuntime] | Out-Null
    [Windows.Devices.Radios.RadioAccessStatus,Windows.System.Devices,ContentType=WindowsRuntime] | Out-Null
    [Windows.Devices.Radios.RadioState,Windows.System.Devices,ContentType=WindowsRuntime] | Out-Null
    $script:WinRtAsTaskGeneric = ([System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {
        $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1'
    })[0]
    $script:RadioApiAvailable = ($null -ne $script:WinRtAsTaskGeneric)
} catch {
    $script:RadioApiAvailable = $false
}

# Last-resort UI safety net. WPF normally lets an exception thrown by a later
# click/key event unwind through ShowDialog(), which would take down the whole
# shell. Log those event failures and keep the dispatcher alive instead.
try {
    [System.Windows.Threading.Dispatcher]::CurrentDispatcher.Add_UnhandledException({
        param($sender, $eventArgs)
        try {
            Write-FedoraWinLog 'error' ('Unhandled UI event: ' + $eventArgs.Exception.ToString())
            if ($Diagnostic) { Write-Host ('[UI ERROR] ' + $eventArgs.Exception.Message) -ForegroundColor Red }
            $eventArgs.Handled = $true
        } catch { }
    })
} catch {
    Write-FedoraWinLog 'warn' ('Dispatcher exception guard unavailable: ' + $_.Exception.Message)
}

# If a previous FedoraWin process died abruptly, restore its saved desktop state
# BEFORE recording a new session. This prevents a crashed modified state from
# becoming the next session's backup.
if (Test-Path -LiteralPath $script:PidPath) {
    $oldPid = 0
    if ([int]::TryParse((Get-Content -Raw -LiteralPath $script:PidPath).Trim(), [ref]$oldPid)) {
        $oldProcess = Get-CimInstance Win32_Process -Filter "ProcessId = $oldPid" -ErrorAction SilentlyContinue
        if (-not $oldProcess -or $oldProcess.CommandLine -notlike '*FedoraWin.ps1*') {
            $restoreScript = Join-Path $script:Root 'Restore-Windows.ps1'
            if (Test-Path -LiteralPath $restoreScript) {
                Start-Process -FilePath 'powershell.exe' -ArgumentList @('-NoProfile','-ExecutionPolicy','Bypass','-File',"`"$restoreScript`"") -Wait -WindowStyle Hidden | Out-Null
            }
        }
    }
}

Remove-Item -Force -ErrorAction SilentlyContinue $script:RestoreRequestPath
Set-Content -LiteralPath $script:PidPath -Value $PID -Encoding ASCII

$config = Get-Content -Raw -Encoding UTF8 -LiteralPath $script:ConfigPath | ConvertFrom-Json

try {
    $nativeUiPath = Join-Path $script:Root 'native\FedoraWinNativeUi.cs'
    Add-Type -TypeDefinition (Get-Content -Raw -Encoding UTF8 -LiteralPath $nativeUiPath) -ReferencedAssemblies @('System.Windows.Forms.dll','System.Drawing.dll','System.dll')
    Write-FedoraWinLog 'info' 'Native AppBar/DWM frame bridge loaded.'
} catch {
    Write-FedoraWinLog 'warn' ('Native UI bridge unavailable: ' + $_.Exception.ToString())
}

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

public static class FedoraWinNative
{
    public const int SW_HIDE = 0;
    public const int SW_SHOW = 5;
    public const int SW_RESTORE = 9;
    public const int SPI_SETDESKWALLPAPER = 20;
    public const int SPI_GETWORKAREA = 48;
    public const int SPI_SETWORKAREA = 47;
    public const int SPIF_UPDATEINIFILE = 0x01;
    public const int SPIF_SENDWININICHANGE = 0x02;
    public const int MOD_ALT = 0x0001;
    public const int MOD_NOREPEAT = 0x4000;
    public const int WM_HOTKEY = 0x0312;
    public const int VK_F1 = 0x70;
    public const byte VK_LWIN = 0x5B;
    public const byte VK_RWIN = 0x5C;
    public const byte VK_TAB = 0x09;
    public const byte VK_A = 0x41;
    public const byte VK_CONTROL = 0x11;
    public const byte VK_LEFT = 0x25;
    public const byte VK_RIGHT = 0x27;
    public const byte VK_D = 0x44;
    public const uint KEYEVENTF_KEYUP = 0x0002;

    [DllImport("user32.dll", SetLastError=true, CharSet=CharSet.Auto)]
    public static extern IntPtr FindWindow(string lpClassName, string lpWindowName);

    [DllImport("user32.dll", SetLastError=true, CharSet=CharSet.Auto)]
    public static extern IntPtr FindWindowEx(IntPtr parent, IntPtr childAfter, string className, string windowName);

    public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
    [DllImport("user32.dll")]
    public static extern bool EnumWindows(EnumWindowsProc callback, IntPtr lParam);

    public static IntPtr GetDesktopListView()
    {
        IntPtr defView = IntPtr.Zero;
        IntPtr progman = FindWindow("Progman", null);
        if (progman != IntPtr.Zero)
            defView = FindWindowEx(progman, IntPtr.Zero, "SHELLDLL_DefView", null);

        if (defView == IntPtr.Zero)
        {
            EnumWindows(delegate(IntPtr hwnd, IntPtr lParam)
            {
                IntPtr child = FindWindowEx(hwnd, IntPtr.Zero, "SHELLDLL_DefView", null);
                if (child != IntPtr.Zero) { defView = child; return false; }
                return true;
            }, IntPtr.Zero);
        }
        if (defView == IntPtr.Zero) return IntPtr.Zero;
        return FindWindowEx(defView, IntPtr.Zero, "SysListView32", "FolderView");
    }

    [DllImport("user32.dll")]
    public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);

    [DllImport("user32.dll")]
    public static extern bool IsWindowVisible(IntPtr hWnd);

    [DllImport("user32.dll")]
    public static extern bool SetForegroundWindow(IntPtr hWnd);

    [DllImport("user32.dll", SetLastError=true)]
    public static extern bool RegisterHotKey(IntPtr hWnd, int id, int fsModifiers, int vk);

    [DllImport("user32.dll", SetLastError=true)]
    public static extern bool UnregisterHotKey(IntPtr hWnd, int id);

    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }

    [DllImport("user32.dll", CharSet=CharSet.Auto, SetLastError=true, EntryPoint="SystemParametersInfo")]
    public static extern bool SystemParametersInfoString(int uAction, int uParam, string lpvParam, int fuWinIni);

    [DllImport("user32.dll", SetLastError=true, EntryPoint="SystemParametersInfo")]
    public static extern bool SystemParametersInfoRect(int uAction, int uParam, ref RECT lpvParam, int fuWinIni);

    [DllImport("user32.dll")]
    public static extern uint GetDpiForSystem();

    public static int[] GetWorkArea()
    {
        RECT r = new RECT();
        if (!SystemParametersInfoRect(SPI_GETWORKAREA, 0, ref r, 0)) return null;
        return new int[] { r.Left, r.Top, r.Right, r.Bottom };
    }

    public static bool SetWorkArea(int left, int top, int right, int bottom)
    {
        RECT r = new RECT { Left = left, Top = top, Right = right, Bottom = bottom };
        return SystemParametersInfoRect(SPI_SETWORKAREA, 0, ref r, SPIF_SENDWININICHANGE);
    }

    [DllImport("user32.dll")]
    public static extern void keybd_event(byte bVk, byte bScan, uint dwFlags, UIntPtr dwExtraInfo);

    public static void SendWinTab()
    {
        keybd_event(VK_LWIN, 0, 0, UIntPtr.Zero);
        keybd_event(VK_TAB, 0, 0, UIntPtr.Zero);
        keybd_event(VK_TAB, 0, KEYEVENTF_KEYUP, UIntPtr.Zero);
        keybd_event(VK_LWIN, 0, KEYEVENTF_KEYUP, UIntPtr.Zero);
    }

    public static void SendWinA()
    {
        keybd_event(VK_LWIN, 0, 0, UIntPtr.Zero);
        keybd_event(VK_A, 0, 0, UIntPtr.Zero);
        keybd_event(VK_A, 0, KEYEVENTF_KEYUP, UIntPtr.Zero);
        keybd_event(VK_LWIN, 0, KEYEVENTF_KEYUP, UIntPtr.Zero);
    }

    private static void SendCtrlWinKey(byte key)
    {
        keybd_event(VK_CONTROL, 0, 0, UIntPtr.Zero);
        keybd_event(VK_LWIN, 0, 0, UIntPtr.Zero);
        keybd_event(key, 0, 0, UIntPtr.Zero);
        keybd_event(key, 0, KEYEVENTF_KEYUP, UIntPtr.Zero);
        keybd_event(VK_LWIN, 0, KEYEVENTF_KEYUP, UIntPtr.Zero);
        keybd_event(VK_CONTROL, 0, KEYEVENTF_KEYUP, UIntPtr.Zero);
    }

    public static void SendDesktopLeft() { SendCtrlWinKey(VK_LEFT); }
    public static void SendDesktopRight() { SendCtrlWinKey(VK_RIGHT); }
    public static void SendDesktopNew() { SendCtrlWinKey(VK_D); }
}

'@

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class FedoraWinGdi
{
    [DllImport("gdi32.dll")]
    public static extern bool DeleteObject(IntPtr hObject);
}
'@

try {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

[ComImport]
[Guid("BCDE0395-E52F-467C-8E3D-C4579291692E")]
internal class MMDeviceEnumeratorComObject { }

internal enum EDataFlow { eRender = 0, eCapture = 1, eAll = 2 }
internal enum ERole { eConsole = 0, eMultimedia = 1, eCommunications = 2 }

[Guid("A95664D2-9614-4F35-A746-DE8DB63617E6")]
[InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
internal interface IMMDeviceEnumerator
{
    [PreserveSig] int EnumAudioEndpoints(EDataFlow dataFlow, uint dwStateMask, out IntPtr ppDevices);
    [PreserveSig] int GetDefaultAudioEndpoint(EDataFlow dataFlow, ERole role, out IMMDevice ppEndpoint);
}

[Guid("D666063F-1587-4E43-81F1-B948E807363F")]
[InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
internal interface IMMDevice
{
    [PreserveSig] int Activate(ref Guid iid, uint dwClsCtx, IntPtr pActivationParams, [MarshalAs(UnmanagedType.IUnknown)] out object ppInterface);
}

[Guid("5CDF2C82-841E-4546-9722-0CF74078229A")]
[InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
internal interface IAudioEndpointVolume
{
    int RegisterControlChangeNotify(IntPtr pNotify);
    int UnregisterControlChangeNotify(IntPtr pNotify);
    int GetChannelCount(out uint pnChannelCount);
    int SetMasterVolumeLevel(float fLevelDB, Guid pguidEventContext);
    int SetMasterVolumeLevelScalar(float fLevel, Guid pguidEventContext);
    int GetMasterVolumeLevel(out float pfLevelDB);
    int GetMasterVolumeLevelScalar(out float pfLevel);
}

public static class FedoraWinAudio
{
    private static IAudioEndpointVolume Endpoint()
    {
        IMMDeviceEnumerator enumerator = (IMMDeviceEnumerator)(new MMDeviceEnumeratorComObject());
        IMMDevice device;
        Marshal.ThrowExceptionForHR(enumerator.GetDefaultAudioEndpoint(EDataFlow.eRender, ERole.eMultimedia, out device));
        Guid iid = typeof(IAudioEndpointVolume).GUID;
        object obj;
        Marshal.ThrowExceptionForHR(device.Activate(ref iid, 23, IntPtr.Zero, out obj));
        return (IAudioEndpointVolume)obj;
    }
    public static float GetMasterVolume()
    {
        float value;
        Marshal.ThrowExceptionForHR(Endpoint().GetMasterVolumeLevelScalar(out value));
        return value;
    }
    public static void SetMasterVolume(float value)
    {
        if (value < 0f) value = 0f;
        if (value > 1f) value = 1f;
        Marshal.ThrowExceptionForHR(Endpoint().SetMasterVolumeLevelScalar(value, Guid.Empty));
    }
}
'@
    $script:AudioAvailable = $true
} catch {
    Write-FedoraWinLog 'warn' ('CoreAudio integration unavailable: ' + $_.Exception.Message)
    $script:AudioAvailable = $false
}

function Import-XamlWindow {
    param([Parameter(Mandatory)][string]$Path)
    [xml]$xaml = Get-Content -Raw -Encoding UTF8 -LiteralPath $Path
    $reader = New-Object System.Xml.XmlNodeReader $xaml
    return [Windows.Markup.XamlReader]::Load($reader)
}

$script:AccentColors = [ordered]@{
    blue   = '#3584E4'
    teal   = '#2190A4'
    green  = '#3A944A'
    yellow = '#C88800'
    orange = '#ED5B00'
    red    = '#E62D42'
    pink   = '#D56199'
    purple = '#9141AC'
    slate  = '#6F8396'
}

function Get-ResolvedThemeMode {
    if ($script:ThemeMode -ne 'system') { return $script:ThemeMode }
    try {
        $v = (Get-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize' -Name AppsUseLightTheme -ErrorAction Stop).AppsUseLightTheme
        if ([int]$v -eq 1) { return 'light' }
    } catch { }
    return 'dark'
}

function Load-FedoraWinSettings {
    $mode = [string]$config.theme
    $accent = [string]$config.accent
    $dockMode = if ($config.dockMode) { [string]$config.dockMode } else { 'both' }
    $dockPosition = if ($config.dockPosition) { [string]$config.dockPosition } else { 'bottom' }
    $dockTopmost = if ($null -ne $config.dockTopmost) { [bool]$config.dockTopmost } else { $true }
    $dockIconSize = if ($config.dockIconSize) { [int]$config.dockIconSize } else { 48 }
    $dockFavorites = @($config.dockFavorites)

    if (Test-Path -LiteralPath $script:SettingsPath) {
        try {
            $saved = Get-Content -Raw -Encoding UTF8 -LiteralPath $script:SettingsPath | ConvertFrom-Json
            if ($saved.theme) { $mode = [string]$saved.theme }
            if ($saved.accent) { $accent = [string]$saved.accent }
            if ($saved.dockMode) { $dockMode = [string]$saved.dockMode }
            if ($saved.dockPosition) { $dockPosition = [string]$saved.dockPosition }
            if ($null -ne $saved.dockTopmost) { $dockTopmost = [bool]$saved.dockTopmost }
            if ($saved.dockIconSize) { $dockIconSize = [int]$saved.dockIconSize }
            if ($saved.dockFavorites) { $dockFavorites = @($saved.dockFavorites | ForEach-Object { [string]$_ }) }
        } catch { Write-FedoraWinLog 'warn' ('Could not read FedoraWin settings: ' + $_.Exception.Message) }
    }

    if ($mode -notin @('light','dark','system')) { $mode = 'dark' }
    if (-not $script:AccentColors.Contains($accent)) { $accent = 'blue' }
    if ($dockMode -notin @('overview','desktop','both')) { $dockMode = 'both' }
    if ($dockPosition -notin @('bottom','left','right')) { $dockPosition = 'bottom' }
    $dockIconSize = [Math]::Max(32,[Math]::Min(64,$dockIconSize))
    if ($dockFavorites.Count -eq 0) { $dockFavorites = @('Files','Terminal','Browser','Visual Studio Code','Settings') }

    $script:ThemeMode = $mode
    $script:AccentName = $accent
    $script:AccentHex = [string]$script:AccentColors[$accent]
    $script:DockMode = $dockMode
    $script:DockPosition = $dockPosition
    $script:DockTopmost = $dockTopmost
    $script:DockIconSize = $dockIconSize
    $script:DockFavorites = @($dockFavorites)
}

function Save-FedoraWinSettings {
    try {
        [ordered]@{
            theme = $script:ThemeMode
            accent = $script:AccentName
            dockMode = $script:DockMode
            dockPosition = $script:DockPosition
            dockTopmost = $script:DockTopmost
            dockIconSize = $script:DockIconSize
            dockFavorites = @($script:DockFavorites)
        } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $script:SettingsPath -Encoding UTF8
    } catch { Write-FedoraWinLog 'warn' ('Could not save FedoraWin settings: ' + $_.Exception.Message) }
}

function Set-ResourceBrush {
    param($Window, [string]$Key, [string]$Color)
    if ($null -eq $Window) { return }
    try {
        $converted = [System.Windows.Media.ColorConverter]::ConvertFromString($Color)
        $brush = [System.Windows.Media.SolidColorBrush]::new($converted)
        $Window.Resources[$Key] = $brush
    } catch { }
}

function Apply-ThemeToWindow {
    param($Window)
    if ($null -eq $Window) { return }
    $resolved = Get-ResolvedThemeMode
    if ($resolved -eq 'light') {
        $palette = @{
            PanelBg = '#FFF6F5F4'; OverlayBg = '#FAF6F5F4'; CardBg = '#FFE9E7E5'; CardHover = '#FFDAD8D5';
            Foreground = '#FF202020'; Muted = '#FF666666'; Border = '#FFD0CECB'; Accent = $script:AccentHex; AccentForeground = '#FFFFFFFF'
        }
    } else {
        $palette = @{
            PanelBg = '#FF242424'; OverlayBg = '#FA242424'; CardBg = '#FF353535'; CardHover = '#FF484848';
            Foreground = '#FFFFFFFF'; Muted = '#FFBDBDBD'; Border = '#FF4B4B4B'; Accent = $script:AccentHex; AccentForeground = '#FFFFFFFF'
        }
    }
    foreach ($key in $palette.Keys) { Set-ResourceBrush -Window $Window -Key $key -Color ([string]$palette[$key]) }
}

function Set-QuickTileActive {
    param([string]$Name, [bool]$Active)
    if ($null -eq $script:QuickWindow) { return }
    $button = $script:QuickWindow.FindName($Name)
    if (-not $button) { return }
    if ($Active) {
        $button.Background = $script:QuickWindow.Resources['Accent']
        $button.Foreground = $script:QuickWindow.Resources['AccentForeground']
    } else {
        $button.ClearValue([System.Windows.Controls.Control]::BackgroundProperty)
        $button.ClearValue([System.Windows.Controls.Control]::ForegroundProperty)
    }
}

function Update-AppearanceControls {
    try {
        $resolved = Get-ResolvedThemeMode
        if ($script:QuickWindow) {
            $title = $script:QuickWindow.FindName('DarkStyleTitle')
            if ($title) {
                if ($script:ThemeMode -eq 'system') {
                    $pretty = $resolved.Substring(0,1).ToUpperInvariant() + $resolved.Substring(1)
                    $title.Text = ('System Style ({0})' -f $pretty)
                } elseif ($resolved -eq 'light') { $title.Text = 'Light Style' }
                else { $title.Text = 'Dark Style' }
            }
            Set-QuickTileActive -Name 'DarkStyleButton' -Active ($resolved -eq 'dark')
            $appearance = $script:QuickWindow.FindName('AppearanceSubText')
            if ($appearance) { $appearance.Text = ('{0}, {1}' -f $script:ThemeMode,$script:AccentName) }
        }

        if ($script:AppearanceWindow) {
            foreach ($pair in @(@('ThemeLightButton','light'),@('ThemeDarkButton','dark'),@('ThemeSystemButton','system'))) {
                $button = $script:AppearanceWindow.FindName($pair[0])
                if ($button) {
                    if ($script:ThemeMode -eq $pair[1]) {
                        $button.Background = $script:AppearanceWindow.Resources['Accent']
                        $button.Foreground = $script:AppearanceWindow.Resources['AccentForeground']
                    } else {
                        $button.ClearValue([System.Windows.Controls.Control]::BackgroundProperty)
                        $button.ClearValue([System.Windows.Controls.Control]::ForegroundProperty)
                    }
                }
            }
            foreach ($name in $script:AccentColors.Keys) {
                $buttonName = 'Accent' + $name.Substring(0,1).ToUpperInvariant() + $name.Substring(1) + 'Button'
                $button = $script:AppearanceWindow.FindName($buttonName)
                if ($button) { $button.Opacity = if ($name -eq $script:AccentName) { 1.0 } else { 0.52 } }
            }
            foreach ($pair in @(@('DockOverviewButton','overview'),@('DockDesktopButton','desktop'),@('DockBothButton','both'))) {
                $button = $script:AppearanceWindow.FindName($pair[0])
                if ($button) {
                    if ($script:DockMode -eq $pair[1]) { $button.Background = $script:AppearanceWindow.Resources['Accent']; $button.Foreground = $script:AppearanceWindow.Resources['AccentForeground'] }
                    else { $button.ClearValue([System.Windows.Controls.Control]::BackgroundProperty); $button.ClearValue([System.Windows.Controls.Control]::ForegroundProperty) }
                }
            }
            foreach ($pair in @(@('DockBottomButton','bottom'),@('DockLeftButton','left'),@('DockRightButton','right'))) {
                $button = $script:AppearanceWindow.FindName($pair[0])
                if ($button) {
                    if ($script:DockPosition -eq $pair[1]) { $button.Background = $script:AppearanceWindow.Resources['Accent']; $button.Foreground = $script:AppearanceWindow.Resources['AccentForeground'] }
                    else { $button.ClearValue([System.Windows.Controls.Control]::BackgroundProperty); $button.ClearValue([System.Windows.Controls.Control]::ForegroundProperty) }
                }
            }
        }
    } catch { Write-FedoraWinLog 'warn' ('Appearance indicator refresh failed: ' + $_.Exception.Message) }
}

function Apply-FedoraWinTheme {
    foreach ($w in @($script:PanelWindow,$script:ActivitiesWindow,$script:CalendarWindow,$script:QuickWindow,$script:AppearanceWindow,$script:PowerWindow,$script:DockWindow)) {
        Apply-ThemeToWindow -Window $w
    }
    Update-AppearanceControls
    if ($script:FrameManager) {
        try { $script:FrameManager.SetTheme((Get-ResolvedThemeMode), $script:AccentHex) } catch { }
    }
}

function Set-FedoraThemeMode {
    param([string]$Mode)
    if ($Mode -notin @('light','dark','system')) { return }
    $script:ThemeMode = $Mode
    Save-FedoraWinSettings
    Apply-FedoraWinTheme
    Write-FedoraWinLog 'info' ('Appearance changed: theme={0}; resolved={1}' -f $script:ThemeMode,(Get-ResolvedThemeMode))
}

function Set-FedoraAccent {
    param([string]$Name)
    if (-not $script:AccentColors.Contains($Name)) { return }
    $script:AccentName = $Name
    $script:AccentHex = [string]$script:AccentColors[$Name]
    Save-FedoraWinSettings
    Apply-FedoraWinTheme
    Write-FedoraWinLog 'info' ('Appearance changed: accent={0} ({1})' -f $Name,$script:AccentHex)
}

function Await-WinRt {
    param($Operation, [Type]$ResultType)
    if (-not $script:RadioApiAvailable -or $null -eq $script:WinRtAsTaskGeneric) { throw 'Windows radio API unavailable' }
    $asTask = $script:WinRtAsTaskGeneric.MakeGenericMethod($ResultType)
    $task = $asTask.Invoke($null, @($Operation))
    $task.Wait(-1) | Out-Null
    return $task.Result
}

function Get-SystemRadios {
    if (-not $script:RadioApiAvailable) { return @() }
    try {
        $listType = [System.Collections.Generic.IReadOnlyList[Windows.Devices.Radios.Radio]]
        return @(Await-WinRt -Operation ([Windows.Devices.Radios.Radio]::GetRadiosAsync()) -ResultType $listType)
    } catch {
        Write-FedoraWinLog 'warn' ('Radio enumeration failed: ' + $_.Exception.Message)
        return @()
    }
}

function Get-RadioState {
    param([Parameter(Mandatory)][ValidateSet('WiFi','Bluetooth')][string]$Kind)
    $radio = Get-SystemRadios | Where-Object { [string]$_.Kind -eq $Kind } | Select-Object -First 1
    if (-not $radio) { return 'Unavailable' }
    return [string]$radio.State
}

function Set-RadioState {
    param(
        [Parameter(Mandatory)][ValidateSet('WiFi','Bluetooth')][string]$Kind,
        [Parameter(Mandatory)][ValidateSet('On','Off')][string]$State
    )
    if (-not $script:RadioApiAvailable) { return $false }
    try {
        if ($script:RadioAccess -ne 'Allowed') {
            $script:RadioAccess = [string](Await-WinRt -Operation ([Windows.Devices.Radios.Radio]::RequestAccessAsync()) -ResultType ([Windows.Devices.Radios.RadioAccessStatus]))
        }
        if ($script:RadioAccess -ne 'Allowed') { return $false }
        $radios = @(Get-SystemRadios | Where-Object { [string]$_.Kind -eq $Kind })
        if ($radios.Count -eq 0) { return $false }
        $ok = $false
        foreach ($radio in $radios) {
            $result = Await-WinRt -Operation ($radio.SetStateAsync($State)) -ResultType ([Windows.Devices.Radios.RadioAccessStatus])
            if ([string]$result -eq 'Allowed') { $ok = $true }
        }
        return $ok
    } catch {
        Write-FedoraWinLog 'warn' ('Radio change failed for {0}: {1}' -f $Kind,$_.Exception.Message)
        return $false
    }
}

function Toggle-RadioState {
    param([Parameter(Mandatory)][ValidateSet('WiFi','Bluetooth')][string]$Kind)
    $state = Get-RadioState -Kind $Kind
    if ($state -eq 'On') { return Set-RadioState -Kind $Kind -State 'Off' }
    if ($state -eq 'Off') { return Set-RadioState -Kind $Kind -State 'On' }
    return $false
}

function Get-PowerModeName {
    try {
        $onAc = ([System.Windows.Forms.SystemInformation]::PowerStatus.PowerLineStatus -eq [System.Windows.Forms.PowerLineStatus]::Online)
        return [FedoraWinPowerMode]::Get($onAc)
    } catch { return 'Unavailable' }
}

function Cycle-PowerMode {
    try {
        $current = Get-PowerModeName
        $next = if ($current -eq 'Power Saver') { 'Balanced' } elseif ($current -eq 'Balanced') { 'Performance' } else { 'Power Saver' }
        if ([FedoraWinPowerMode]::Set($next)) { return $next }
    } catch { Write-FedoraWinLog 'warn' ('Power mode change failed: ' + $_.Exception.Message) }
    return 'Unavailable'
}

function Refresh-QuickSettingsState {
    if ($null -eq $script:QuickWindow) { return }
    try {
        $wifiState = Get-RadioState -Kind 'WiFi'
        $btState = Get-RadioState -Kind 'Bluetooth'
        $wifiAvailable = ($wifiState -ne 'Unavailable')
        $btAvailable = ($btState -ne 'Unavailable')

        $wifiButton = $script:QuickWindow.FindName('WifiButton')
        $btButton = $script:QuickWindow.FindName('BluetoothButton')
        $airButton = $script:QuickWindow.FindName('AirplaneButton')
        if ($wifiButton) { $wifiButton.Visibility = if ($wifiAvailable) { 'Visible' } else { 'Collapsed' } }
        if ($btButton) { $btButton.Visibility = if ($btAvailable) { 'Visible' } else { 'Collapsed' } }
        if ($airButton) { $airButton.Visibility = if ($wifiAvailable -or $btAvailable) { 'Visible' } else { 'Collapsed' } }

        $wifiSub = $script:QuickWindow.FindName('WifiSubText')
        $btSub = $script:QuickWindow.FindName('BluetoothSubText')
        if ($wifiSub) {
            $ssid = if ($wifiState -eq 'On') { Get-WifiName } else { $null }
            $wifiSub.Text = if ($wifiState -eq 'On' -and $ssid) { $ssid } else { $wifiState }
        }
        if ($btSub) { $btSub.Text = $btState }

        Set-QuickTileActive -Name 'WifiButton' -Active ($wifiState -eq 'On')
        Set-QuickTileActive -Name 'BluetoothButton' -Active ($btState -eq 'On')

        $availableRadioStates = @()
        if ($wifiAvailable) { $availableRadioStates += $wifiState }
        if ($btAvailable) { $availableRadioStates += $btState }
        $airOn = ($availableRadioStates.Count -gt 0 -and @($availableRadioStates | Where-Object { $_ -eq 'On' }).Count -eq 0)
        Set-QuickTileActive -Name 'AirplaneButton' -Active $airOn
        $airText = $script:QuickWindow.FindName('AirplaneSubText')
        if ($airText) { $airText.Text = if ($airOn) { 'Radios off' } else { 'Radios on' } }

        $powerMode = Get-PowerModeName
        $powerButton = $script:QuickWindow.FindName('PowerModeButton')
        $powerText = $script:QuickWindow.FindName('PowerModeText')
        if ($powerText) { $powerText.Text = $powerMode }
        if ($powerButton) { $powerButton.Visibility = if ($powerMode -eq 'Unavailable') { 'Collapsed' } else { 'Visible' } }

        $visibleTiles = 0
        foreach ($tileName in @('WifiButton','BluetoothButton','PowerModeButton','DarkStyleButton','AirplaneButton')) {
            $tile = $script:QuickWindow.FindName($tileName)
            if ($tile -and $tile.Visibility -eq [System.Windows.Visibility]::Visible) { $visibleTiles++ }
        }
        $rows = [Math]::Max(1,[int][Math]::Ceiling($visibleTiles / 2.0))
        $script:QuickWindow.Height = [Math]::Min(428,[Math]::Max(288,218 + (70 * $rows)))
    } catch { Write-FedoraWinLog 'warn' ('Quick Settings state refresh failed: ' + $_.Exception.Message) }
}

function Toggle-AppearancePopover {
    if ($null -ne $script:AppearanceWindow -and $script:AppearanceWindow.IsVisible) { $script:AppearanceWindow.Hide(); return }
    if ($null -ne $script:QuickWindow) { $script:QuickWindow.Hide() }
    if ($null -eq $script:AppearanceWindow) {
        $script:AppearanceWindow = Import-XamlWindow -Path (Join-Path $script:Root 'ui\Appearance.xaml')
        Apply-ThemeToWindow -Window $script:AppearanceWindow
        $script:AppearanceWindow.Top = ([double]$config.panelHeight + 8)
        $script:AppearanceWindow.Left = [Math]::Max(8, [System.Windows.SystemParameters]::PrimaryScreenWidth - $script:AppearanceWindow.Width - 10)
        foreach ($pair in @(@('ThemeLightButton','light'),@('ThemeDarkButton','dark'),@('ThemeSystemButton','system'))) {
            $button = $script:AppearanceWindow.FindName($pair[0]); $mode = $pair[1]
            if ($button) { $button.Add_Click({ param($sender,$eventArgs) Set-FedoraThemeMode -Mode ([string]$sender.Tag) }) }
        }
        foreach ($accentButtonName in @('AccentBlueButton','AccentTealButton','AccentGreenButton','AccentYellowButton','AccentOrangeButton','AccentRedButton','AccentPinkButton','AccentPurpleButton','AccentSlateButton')) {
            $button = $script:AppearanceWindow.FindName($accentButtonName)
            if ($button) { $button.Add_Click({ param($sender,$eventArgs) Set-FedoraAccent -Name ([string]$sender.Tag) }) }
        }
        foreach ($dockButtonName in @('DockOverviewButton','DockDesktopButton','DockBothButton')) {
            $button = $script:AppearanceWindow.FindName($dockButtonName)
            if ($button) { $button.Add_Click({ param($sender,$eventArgs) Set-FedoraDockMode -Mode ([string]$sender.Tag) }) }
        }
        foreach ($dockButtonName in @('DockBottomButton','DockLeftButton','DockRightButton')) {
            $button = $script:AppearanceWindow.FindName($dockButtonName)
            if ($button) { $button.Add_Click({ param($sender,$eventArgs) Set-FedoraDockPosition -Position ([string]$sender.Tag) }) }
        }
        $script:AppearanceWindow.FindName('AppearanceCloseButton').Add_Click({ $script:AppearanceWindow.Hide() })
        $script:AppearanceWindow.Add_Deactivated({ if ($script:AppearanceWindow.IsVisible) { $script:AppearanceWindow.Hide() } })
    }
    Apply-ThemeToWindow -Window $script:AppearanceWindow
    Update-AppearanceControls
    $script:AppearanceWindow.Show()
    $script:AppearanceWindow.Activate() | Out-Null
}

function Invoke-PowerAction {
    param([Parameter(Mandatory)][ValidateSet('Lock','Suspend','Restart','PowerOff','LogOut')][string]$Action)
    try {
        switch ($Action) {
            'Lock'     { [void][FedoraWinSession]::Lock() }
            'Suspend'  { [void][FedoraWinSession]::Suspend() }
            'Restart'  { Start-Process -FilePath 'shutdown.exe' -ArgumentList @('/r','/t','0') -WindowStyle Hidden | Out-Null }
            'PowerOff' { Start-Process -FilePath 'shutdown.exe' -ArgumentList @('/s','/t','0') -WindowStyle Hidden | Out-Null }
            'LogOut'   { Start-Process -FilePath 'shutdown.exe' -ArgumentList @('/l') -WindowStyle Hidden | Out-Null }
        }
        Write-FedoraWinLog 'info' ('Power action requested: ' + $Action)
    } catch {
        Write-FedoraWinLog 'warn' ('Power action failed ({0}): {1}' -f $Action,$_.Exception.Message)
    }
}

function Toggle-PowerPopover {
    if ($null -ne $script:PowerWindow -and $script:PowerWindow.IsVisible) { $script:PowerWindow.Hide(); return }
    if ($null -ne $script:CalendarWindow) { $script:CalendarWindow.Hide() }
    if ($null -ne $script:AppearanceWindow) { $script:AppearanceWindow.Hide() }
    if ($null -eq $script:PowerWindow) {
        $script:PowerWindow = Import-XamlWindow -Path (Join-Path $script:Root 'ui\PowerMenu.xaml')
        Apply-ThemeToWindow -Window $script:PowerWindow
        $script:PowerWindow.Top = ([double]$config.panelHeight + 8)
        $script:PowerWindow.Left = [Math]::Max(8, [System.Windows.SystemParameters]::PrimaryScreenWidth - $script:PowerWindow.Width - 10)
        foreach ($pair in @(@('LockButton','Lock'),@('SuspendButton','Suspend'),@('RestartButton','Restart'),@('PowerOffButton','PowerOff'),@('LogOutButton','LogOut'))) {
            $button = $script:PowerWindow.FindName($pair[0])
            if ($button) {
                $button.Tag = $pair[1]
                $button.Add_Click({
                    param($sender,$eventArgs)
                    $action = [string]$sender.Tag
                    $script:PowerWindow.Hide()
                    if ($action -in @('Restart','PowerOff','LogOut')) {
                        $pretty = if ($action -eq 'PowerOff') { 'Power Off' } elseif ($action -eq 'LogOut') { 'Log Out' } else { 'Restart' }
                        $result = [System.Windows.MessageBox]::Show("$pretty now?", 'FedoraWin', 'YesNo', 'Question')
                        if ($result -ne [System.Windows.MessageBoxResult]::Yes) { return }
                    }
                    Invoke-PowerAction -Action $action
                })
            }
        }
        $script:PowerWindow.Add_Deactivated({ if ($script:PowerWindow.IsVisible) { $script:PowerWindow.Hide() } })
    }
    Apply-ThemeToWindow -Window $script:PowerWindow
    $script:PowerWindow.Show()
    $script:PowerWindow.Activate() | Out-Null
}

function Get-TaskbarHandle {
    return [FedoraWinNative]::FindWindow('Shell_TrayWnd', $null)
}

function Show-WindowsTaskbar {
    $handle = Get-TaskbarHandle
    if ($handle -ne [IntPtr]::Zero) {
        [FedoraWinNative]::ShowWindow($handle, [FedoraWinNative]::SW_SHOW) | Out-Null
    }
}

function Hide-WindowsTaskbar {
    $handle = Get-TaskbarHandle
    if ($handle -ne [IntPtr]::Zero) {
        [FedoraWinNative]::ShowWindow($handle, [FedoraWinNative]::SW_HIDE) | Out-Null
    }
}

function Get-DesktopIconsHandle {
    return [FedoraWinNative]::GetDesktopListView()
}

function Show-DesktopIcons {
    $handle = Get-DesktopIconsHandle
    if ($handle -ne [IntPtr]::Zero) { [FedoraWinNative]::ShowWindow($handle, [FedoraWinNative]::SW_SHOW) | Out-Null }
}

function Hide-DesktopIcons {
    $handle = Get-DesktopIconsHandle
    if ($handle -ne [IntPtr]::Zero) { [FedoraWinNative]::ShowWindow($handle, [FedoraWinNative]::SW_HIDE) | Out-Null }
}

function Get-CurrentWallpaper {
    try {
        return (Get-ItemProperty -Path 'HKCU:\Control Panel\Desktop' -Name WallPaper -ErrorAction Stop).WallPaper
    } catch {
        return $null
    }
}

function Set-Wallpaper {
    param([Parameter(Mandatory)][string]$Path)
    if (-not (Test-Path -LiteralPath $Path)) { return }
    [FedoraWinNative]::SystemParametersInfoString(
        [FedoraWinNative]::SPI_SETDESKWALLPAPER,
        0,
        $Path,
        [FedoraWinNative]::SPIF_UPDATEINIFILE -bor [FedoraWinNative]::SPIF_SENDWININICHANGE
    ) | Out-Null
}

function Ensure-FedoraWinWallpaper {
    if (Test-Path -LiteralPath $script:WallpaperPath) { return $script:WallpaperPath }
    try {
        $width = 1920
        $height = 1080
        $bitmap = New-Object System.Drawing.Bitmap -ArgumentList $width,$height,([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        try {
            $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
            $rect = New-Object System.Drawing.Rectangle -ArgumentList 0,0,$width,$height
            $start = [System.Drawing.Color]::FromArgb(255,18,36,82)
            $finish = [System.Drawing.Color]::FromArgb(255,46,116,190)
            $gradient = New-Object System.Drawing.Drawing2D.LinearGradientBrush -ArgumentList $rect,$start,$finish,28.0
            try { $graphics.FillRectangle($gradient,$rect) } finally { $gradient.Dispose() }

            $haloOne = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(54,120,190,255))
            $haloTwo = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(46,108,72,188))
            $haloThree = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(44,20,45,105))
            try {
                $graphics.FillEllipse($haloOne,980,-330,1180,1180)
                $graphics.FillEllipse($haloTwo,-320,520,980,980)
                $graphics.FillEllipse($haloThree,1180,610,850,850)
            } finally {
                $haloOne.Dispose()
                $haloTwo.Dispose()
                $haloThree.Dispose()
            }

            $bitmap.Save($script:WallpaperPath,[System.Drawing.Imaging.ImageFormat]::Png)
        } finally {
            $graphics.Dispose()
            $bitmap.Dispose()
        }
        Write-FedoraWinLog 'info' ('Generated FedoraWin session wallpaper: ' + $script:WallpaperPath)
        return $script:WallpaperPath
    } catch {
        Write-FedoraWinLog 'warn' ('Could not generate FedoraWin wallpaper: ' + $_.Exception.Message)
        return $null
    }
}

function Save-State {
    $state = [ordered]@{
        version = '0.4.0-dev'
        pid = $PID
        originalWallpaper = $script:OriginalWallpaper
        taskbarWasVisible = $script:TaskbarWasVisible
        desktopIconsWereVisible = $script:DesktopIconsWereVisible
        originalWorkArea = $script:OriginalWorkArea
        safeMode = [bool]$SafeMode
        startedUtc = [DateTime]::UtcNow.ToString('o')
    }
    $state | ConvertTo-Json | Set-Content -LiteralPath $script:StatePath -Encoding UTF8
}

function Restore-DesktopState {
    if ($script:OriginalWorkArea -and $script:OriginalWorkArea.Count -eq 4) {
        [FedoraWinNative]::SetWorkArea(
            [int]$script:OriginalWorkArea[0],
            [int]$script:OriginalWorkArea[1],
            [int]$script:OriginalWorkArea[2],
            [int]$script:OriginalWorkArea[3]
        ) | Out-Null
    }
    if ($script:TaskbarWasVisible) {
        Show-WindowsTaskbar
    } else {
        Hide-WindowsTaskbar
    }
    if ($script:DesktopIconsWereVisible) {
        Show-DesktopIcons
    } else {
        Hide-DesktopIcons
    }
    if (-not $SafeMode -and [bool]$config.applyWallpaperWhileRunning -and $script:OriginalWallpaper) {
        if (Test-Path -LiteralPath $script:OriginalWallpaper) {
            Set-Wallpaper -Path $script:OriginalWallpaper
        }
    }
}

function Get-StartMenuApps {
    try {
        $apps = @(Get-FedoraWinInstalledApps)
        if ($config.maxLauncherApps) {
            $apps = @($apps | Select-Object -First ([int]$config.maxLauncherApps))
        }
        return $apps
    } catch {
        Write-FedoraWinLog 'warn' ('App discovery failed: ' + $_.Exception.Message)
        return @()
    }
}

function Get-AppIconSource {
    param([string]$ExecutablePath, [int]$Size = 48)
    if (-not $ExecutablePath -or -not (Test-Path -LiteralPath $ExecutablePath)) { return $null }
    try {
        $icon = [System.Drawing.Icon]::ExtractAssociatedIcon([string]$ExecutablePath)
        if (-not $icon) { return $null }
        $source = [System.Windows.Interop.Imaging]::CreateBitmapSourceFromHIcon(
            $icon.Handle,
            [System.Windows.Int32Rect]::Empty,
            [System.Windows.Media.Imaging.BitmapSizeOptions]::FromWidthAndHeight($Size, $Size)
        )
        $source.Freeze()
        $icon.Dispose()
        return $source
    } catch {
        return $null
    }
}

function Get-ScreenCaptureSource {
    try {
        $screen = [System.Windows.Forms.Screen]::PrimaryScreen
        $bounds = $screen.Bounds
        $working = $screen.WorkingArea
        $dpi = [FedoraWinNative]::GetDpiForSystem(); if ($dpi -le 0) { $dpi = 96 }
        $panelPixels = [int][Math]::Round(([double]$config.panelHeight) * $dpi / 96.0)
        $captureTop = $bounds.Top + $panelPixels
        $captureBottom = if ($SafeMode) { [Math]::Min($bounds.Bottom, $working.Bottom) } else { $bounds.Bottom }
        $captureHeight = [Math]::Max(100, $captureBottom - $captureTop)
        $bitmap = New-Object System.Drawing.Bitmap -ArgumentList $bounds.Width, $captureHeight, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        $captureSize = New-Object System.Drawing.Size -ArgumentList $bounds.Width, $captureHeight
        $graphics.CopyFromScreen($bounds.Left, $captureTop, 0, 0, $captureSize, [System.Drawing.CopyPixelOperation]::SourceCopy)
        $graphics.Dispose()
        $hBitmap = $bitmap.GetHbitmap()
        try {
            $source = [System.Windows.Interop.Imaging]::CreateBitmapSourceFromHBitmap(
                $hBitmap,
                [IntPtr]::Zero,
                [System.Windows.Int32Rect]::Empty,
                [System.Windows.Media.Imaging.BitmapSizeOptions]::FromEmptyOptions()
            )
            $source.Freeze()
            return $source
        } finally {
            [FedoraWinGdi]::DeleteObject($hBitmap) | Out-Null
            $bitmap.Dispose()
        }
    } catch {
        Write-FedoraWinLog 'warn' ('Workspace preview capture failed: ' + $_.Exception.Message)
        return $null
    }
}

function Get-BatteryPercent {
    try {
        $p = [System.Windows.Forms.SystemInformation]::PowerStatus.BatteryLifePercent
        if ($p -lt 0) { return $null }
        return [Math]::Min(100, [Math]::Max(0, [int][Math]::Round($p * 100)))
    } catch { return $null }
}

function Get-WifiName {
    try {
        $output = & netsh.exe wlan show interfaces 2>$null
        foreach ($line in $output) {
            if ($line -match '^\s*SSID\s*:\s*(.+)$' -and $line -notmatch 'BSSID') {
                return $Matches[1].Trim()
            }
        }
    } catch { }
    return $null
}

function Get-DisplayBrightness {
    try {
        $item = Get-CimInstance -Namespace root/WMI -ClassName WmiMonitorBrightness -ErrorAction Stop | Select-Object -First 1
        if ($item) { return [int]$item.CurrentBrightness }
    } catch { }
    return $null
}

function Set-DisplayBrightness {
    param([int]$Value)
    try {
        $methods = Get-CimInstance -Namespace root/WMI -ClassName WmiMonitorBrightnessMethods -ErrorAction Stop
        foreach ($m in @($methods)) {
            Invoke-CimMethod -InputObject $m -MethodName WmiSetBrightness -Arguments @{ Timeout = [uint32]1; Brightness = [byte]([Math]::Max(0,[Math]::Min(100,$Value))) } -ErrorAction Stop | Out-Null
        }
    } catch {
        Write-FedoraWinLog 'warn' ('Brightness change unavailable: ' + $_.Exception.Message)
    }
}

function Start-Target {
    param([Parameter(Mandatory)][string]$Target)
    try {
        if ($Target.StartsWith('shell:AppsFolder\', [StringComparison]::OrdinalIgnoreCase)) {
            Start-Process -FilePath 'explorer.exe' -ArgumentList $Target | Out-Null
        } else {
            Start-Process -FilePath $Target | Out-Null
        }
    } catch {
        Write-FedoraWinLog 'warn' ('Could not launch {0}: {1}' -f $Target,$_.Exception.Message)
        [System.Windows.MessageBox]::Show("Could not launch:`n$Target", 'FedoraWin') | Out-Null
    }
}

function New-AppButton {
    param($App, [switch]$Compact)
    $button = New-Object System.Windows.Controls.Button
    if ($script:ActivitiesWindow) {
        $button.Style = $script:ActivitiesWindow.FindResource('AppButtonStyle')
    }
    if ($Compact) {
        $button.Width = 132
        $button.Height = 104
        $button.Margin = '7'
    }

    $stack = New-Object System.Windows.Controls.StackPanel
    $stack.HorizontalAlignment = 'Center'
    $stack.VerticalAlignment = 'Center'

    $source = Get-AppIconSource -ExecutablePath $App.TargetPath -Size 48
    if ($source) {
        $image = New-Object System.Windows.Controls.Image
        $image.Width = 48
        $image.Height = 48
        $image.Margin = '0,0,0,8'
        $image.Source = $source
        [void]$stack.Children.Add($image)
    } else {
        $fallback = New-Object System.Windows.Controls.Border
        $fallback.Width = 46
        $fallback.Height = 46
        $fallback.CornerRadius = 11
        $fallback.Background = $script:ActivitiesWindow.Resources['CardBg']
        $fallback.Margin = '0,0,0,8'
        $letter = New-Object System.Windows.Controls.TextBlock
        $letter.Text = if ($App.Name) { $App.Name.Substring(0,1).ToUpperInvariant() } else { '?' }
        $letter.Foreground = $script:ActivitiesWindow.Resources['Foreground']
        $letter.FontSize = 20
        $letter.FontWeight = 'SemiBold'
        $letter.HorizontalAlignment = 'Center'
        $letter.VerticalAlignment = 'Center'
        $fallback.Child = $letter
        [void]$stack.Children.Add($fallback)
    }

    $label = New-Object System.Windows.Controls.TextBlock
    $label.Text = $App.Name
    $label.TextAlignment = 'Center'
    $label.TextWrapping = 'Wrap'
    $label.TextTrimming = 'CharacterEllipsis'
    $label.Foreground = $script:ActivitiesWindow.Resources['Foreground']
    $label.FontFamily = 'Segoe UI Variable Text'
    $label.FontSize = 12.5
    $label.MaxWidth = 108
    $label.MaxHeight = 34
    [void]$stack.Children.Add($label)

    $button.Content = $stack
    $button.Tag = $App
    $button.Add_Click({
        param($sender, $eventArgs)
        try {
            Start-FedoraWinApp -App $sender.Tag
            Hide-Activities
        } catch {
            Write-FedoraWinLog 'warn' ('Could not launch app: ' + $_.Exception.Message)
        }
    })
    return $button
}

function Render-AppGridPage {
    if ($null -eq $script:ActivitiesWindow) { return }
    $panel = $script:ActivitiesWindow.FindName('AppPanel')
    if ($null -eq $panel) { return }

    $apps = @($script:InstalledApps)
    $script:AppGridPageCount = [Math]::Max(1, [int][Math]::Ceiling($apps.Count / [double]$script:AppGridPageSize))
    if ($script:AppGridPage -ge $script:AppGridPageCount) { $script:AppGridPage = $script:AppGridPageCount - 1 }
    if ($script:AppGridPage -lt 0) { $script:AppGridPage = 0 }

    $panel.Children.Clear()
    $start = $script:AppGridPage * $script:AppGridPageSize
    $pageApps = @($apps | Select-Object -Skip $start -First $script:AppGridPageSize)
    foreach ($app in $pageApps) { [void]$panel.Children.Add((New-AppButton -App $app)) }

    $dots = $script:ActivitiesWindow.FindName('AppPageDotsPanel')
    if ($dots) {
        $dots.Children.Clear()
        for ($i = 0; $i -lt $script:AppGridPageCount; $i++) {
            $dot = New-Object System.Windows.Controls.Button
            $dot.Style = $script:ActivitiesWindow.FindResource('PageDotButtonStyle')
            $dot.Tag = $i
            $dot.ToolTip = ('Application page {0}' -f ($i + 1))
            if ($i -eq $script:AppGridPage) {
                $dot.Width = 18
                $dot.Background = $script:ActivitiesWindow.Resources['Accent']
                $dot.Opacity = 1.0
            } else {
                $dot.Width = 8
                $dot.Background = $script:ActivitiesWindow.Resources['Muted']
                $dot.Opacity = 0.58
            }
            $dot.Add_Click({
                param($sender,$eventArgs)
                $script:AppGridPage = [int]$sender.Tag
                Render-AppGridPage
            })
            [void]$dots.Children.Add($dot)
        }
    }
    $script:AppGridPopulated = $true
}

function Move-AppGridPage {
    param([Parameter(Mandatory)][int]$Delta)
    $target = [Math]::Max(0, [Math]::Min($script:AppGridPageCount - 1, $script:AppGridPage + $Delta))
    if ($target -eq $script:AppGridPage -and $script:AppGridPopulated) { return }
    $script:AppGridPage = $target
    Render-AppGridPage
}

function Populate-Apps {
    param([string]$Filter = '')
    if ($null -eq $script:ActivitiesWindow) { return }
    $needle = $Filter.Trim()
    $overview = $script:ActivitiesWindow.FindName('OverviewView')
    $appsView = $script:ActivitiesWindow.FindName('AppsView')
    $searchView = $script:ActivitiesWindow.FindName('SearchResultsView')
    $clear = $script:ActivitiesWindow.FindName('ClearSearchButton')
    $hint = $script:ActivitiesWindow.FindName('SearchHint')

    if ($needle) {
        Set-WorkspacePresenterVisible -Visible $false
        $overview.Visibility = 'Collapsed'
        $appsView.Visibility = 'Collapsed'
        $searchView.Visibility = 'Visible'
        $clear.Visibility = 'Visible'
        $hint.Visibility = 'Collapsed'
        $panel = $script:ActivitiesWindow.FindName('SearchResultsPanel')
        $panel.Children.Clear()
        $matches = @(Search-FedoraWinApps -Apps $script:InstalledApps -Query $needle -Limit 16)
        foreach ($app in $matches) { [void]$panel.Children.Add((New-AppButton -App $app -Compact)) }
        $script:ActivitiesWindow.FindName('NoResultsText').Visibility = if ($matches.Count -eq 0) { 'Visible' } else { 'Collapsed' }
        return
    }

    $clear.Visibility = 'Collapsed'
    $hint.Visibility = 'Visible'
    $searchView.Visibility = 'Collapsed'
    if ($script:ActivitiesMode -eq 'apps') {
        Set-WorkspacePresenterVisible -Visible $false
        $overview.Visibility = 'Collapsed'
        $appsView.Visibility = 'Visible'
        if (-not $script:AppGridPopulated) { Render-AppGridPage }
    } else {
        $appsView.Visibility = 'Collapsed'
        $overview.Visibility = 'Visible'
        Set-WorkspacePresenterVisible -Visible $true
    }
}

function Get-DashProcessName {
    param($App)
    if ($null -eq $App) { return $null }
    if ($App.PSObject.Properties['ProcessName'] -and $App.ProcessName) { return [string]$App.ProcessName }
    $name = [string]$App.Name
    if ($name -match 'Windows Terminal|^Terminal$') { return 'WindowsTerminal' }
    if ($name -match 'Visual Studio Code') { return 'Code' }
    if ($name -match 'Settings') { return 'SystemSettings' }
    if ($name -match 'File Explorer|^Files$') { return 'explorer' }
    $path = if ($App.PSObject.Properties['TargetPath']) { [string]$App.TargetPath } else { $null }
    if (-not $path -and $App.PSObject.Properties['Target']) { $path = [string]$App.Target }
    if ($path -and $path.EndsWith('.exe',[StringComparison]::OrdinalIgnoreCase)) { return [IO.Path]::GetFileNameWithoutExtension($path) }
    return $null
}

function New-DashRecord {
    param([string]$Name,[string]$FavoriteKey,[string]$Target,[string]$TargetPath,[string]$AppId,[string]$Arguments,[string]$IconPath,[string]$ProcessName,[bool]$IsFavorite=$false)
    [pscustomobject]@{ Name=$Name; FavoriteKey=if($FavoriteKey){$FavoriteKey}else{$Name}; Target=$Target; TargetPath=$TargetPath; AppId=$AppId; Arguments=$Arguments; IconPath=$IconPath; ProcessName=$ProcessName; IsFavorite=$IsFavorite; IsRunning=$false }
}

function ConvertTo-DashRecord {
    param($App,[string]$FavoriteKey,[bool]$IsFavorite=$false)
    if ($null -eq $App) { return $null }
    $target = if ($App.AppId) { 'shell:AppsFolder\' + [string]$App.AppId } else { [string]$App.TargetPath }
    return New-DashRecord -Name ([string]$App.Name) -FavoriteKey $FavoriteKey -Target $target -TargetPath ([string]$App.TargetPath) -AppId ([string]$App.AppId) -Arguments ([string]$App.Arguments) -IconPath ([string]$App.TargetPath) -ProcessName (Get-DashProcessName -App $App) -IsFavorite $IsFavorite
}

function Resolve-DashFavorite {
    param([string]$Key)
    if ($Key -eq 'Files') {
        $path=Join-Path $env:WINDIR 'explorer.exe'
        return New-DashRecord -Name 'Files' -FavoriteKey 'Files' -Target $path -TargetPath $path -IconPath $path -ProcessName 'explorer' -IsFavorite $true
    }
    if ($Key -eq 'Terminal') {
        $app=$null
        if(@($script:InstalledApps).Count -gt 0){$app=Search-FedoraWinApps -Apps $script:InstalledApps -Query 'terminal' -Limit 8 | Where-Object { $_.Name -match 'Windows Terminal|Terminal|PowerShell' } | Select-Object -First 1}
        if ($app) { return ConvertTo-DashRecord -App $app -FavoriteKey 'Terminal' -IsFavorite $true }
        $cmd=Get-Command 'powershell.exe' -ErrorAction SilentlyContinue
        if ($cmd) { return New-DashRecord -Name 'Terminal' -FavoriteKey 'Terminal' -Target $cmd.Source -TargetPath $cmd.Source -IconPath $cmd.Source -ProcessName 'powershell' -IsFavorite $true }
        return $null
    }
    if ($Key -eq 'Browser') {
        $app=$script:InstalledApps | Where-Object { $_.Name -match '^(Google Chrome|Microsoft Edge|Opera|Firefox)' } | Select-Object -First 1
        if ($app) { return ConvertTo-DashRecord -App $app -FavoriteKey 'Browser' -IsFavorite $true }
        return $null
    }
    if ($Key -eq 'Settings') {
        $icon=Join-Path $env:WINDIR 'ImmersiveControlPanel\SystemSettings.exe'
        return New-DashRecord -Name 'Settings' -FavoriteKey 'Settings' -Target 'ms-settings:' -IconPath $icon -ProcessName 'SystemSettings' -IsFavorite $true
    }
    $app=$script:InstalledApps | Where-Object { $_.Name -eq $Key } | Select-Object -First 1
    if (-not $app -and @($script:InstalledApps).Count -gt 0) { $app=Search-FedoraWinApps -Apps $script:InstalledApps -Query $Key -Limit 1 | Select-Object -First 1 }
    if ($app) { return ConvertTo-DashRecord -App $app -FavoriteKey $Key -IsFavorite $true }
    return $null
}

function Test-DashAppRunning {
    param($App)
    $processName=Get-DashProcessName -App $App
    if (-not $processName) { return $false }
    try { return $null -ne (Get-Process -Name $processName -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero } | Select-Object -First 1) } catch { return $false }
}

function Get-RunningDashRecords {
    param([object[]]$SkipProcessNames = @())
    $skip=@{}
    foreach($name in $SkipProcessNames){if($name){$skip[([string]$name).ToLowerInvariant()]=$true}}
    $ignored=@('ShellExperienceHost','StartMenuExperienceHost','SearchHost','TextInputHost','ApplicationFrameHost','dwm')
    $records=@()
    foreach($process in (Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero })){
        $processName=[string]$process.ProcessName
        if(-not $processName -or $ignored -contains $processName){continue}
        if($skip.ContainsKey($processName.ToLowerInvariant())){continue}
        $path=$null
        try{$path=[string]$process.Path}catch{}
        $catalogApp=$null
        if($path -and $script:InstalledAppsLoaded){
            $catalogApp=$script:InstalledApps | Where-Object { $_.TargetPath -and ([IO.Path]::GetFileNameWithoutExtension([string]$_.TargetPath) -ieq $processName) } | Select-Object -First 1
        }
        if(-not $catalogApp -and $script:InstalledAppsLoaded -and $processName -eq 'WindowsTerminal'){
            $catalogApp=$script:InstalledApps | Where-Object { $_.Name -match 'Windows Terminal' } | Select-Object -First 1
        }
        if($catalogApp){$record=ConvertTo-DashRecord -App $catalogApp -FavoriteKey ([string]$catalogApp.Name) -IsFavorite $false; $record.ProcessName=$processName}
        else{continue}
        $record.IsRunning=$true
        $records += $record
        $skip[$processName.ToLowerInvariant()]=$true
    }
    return $records | Sort-Object Name
}

function Get-FedoraDashItems {
    $items=@()
    $processNames=@()
    foreach($key in $script:DockFavorites){
        $record=Resolve-DashFavorite -Key ([string]$key)
        if(-not $record){continue}
        $record.IsRunning=Test-DashAppRunning -App $record
        $items += $record
        if($record.ProcessName){$processNames += [string]$record.ProcessName}
    }
    $running = Get-RunningDashRecords -SkipProcessNames $processNames
    foreach($record in $running){$items += $record}
    return $items
}

function Invoke-DashApp {
    param($App,[switch]$NewWindow)
    if($null -eq $App){return}
    if(-not $NewWindow){
        $processName=Get-DashProcessName -App $App
        if($processName){
            try{
                $running=Get-Process -Name $processName -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero } | Sort-Object StartTime -Descending | Select-Object -First 1
                if($running){[void][FedoraWinNative]::ShowWindow($running.MainWindowHandle,[FedoraWinNative]::SW_RESTORE); [void][FedoraWinNative]::SetForegroundWindow($running.MainWindowHandle); return}
            }catch{}
        }
    }
    if($App.AppId -or $App.TargetPath){try{Start-FedoraWinApp -App $App; return}catch{}}
    if($App.Target){Start-Target -Target ([string]$App.Target)}
}

function Set-DashFavorite {
    param($App,[bool]$Favorite)
    if($null -eq $App){return}
    $key=if($App.FavoriteKey){[string]$App.FavoriteKey}else{[string]$App.Name}
    $current=@()
    foreach($item in $script:DockFavorites){if(-not [string]::Equals([string]$item,$key,[StringComparison]::OrdinalIgnoreCase)){$current += [string]$item}}
    if($Favorite){$current += $key}
    $script:DockFavorites=$current
    Save-FedoraWinSettings
    Refresh-Docks -Force
}

function New-DashButton {
    param($App,$HostWindow,[string]$StyleName,[int]$IconSize=44)
    $button=New-Object System.Windows.Controls.Button
    $button.Style=$HostWindow.FindResource($StyleName)
    $button.ToolTip=[string]$App.Name
    $button.Tag=$App
    $grid=New-Object System.Windows.Controls.Grid
    $grid.Width=$IconSize+8; $grid.Height=$IconSize+8
    $source=Get-AppIconSource -ExecutablePath ([string]$App.IconPath) -Size $IconSize
    if($source){$image=New-Object System.Windows.Controls.Image; $image.Width=$IconSize; $image.Height=$IconSize; $image.HorizontalAlignment='Center'; $image.VerticalAlignment='Top'; $image.Source=$source; [void]$grid.Children.Add($image)}
    else{$fallback=New-Object System.Windows.Controls.Border; $fallback.Width=$IconSize; $fallback.Height=$IconSize; $fallback.CornerRadius=[Math]::Max(9,[int]($IconSize/4)); $fallback.Background=$HostWindow.Resources['CardBg']; $letter=New-Object System.Windows.Controls.TextBlock; $letter.Text=if($App.Name){([string]$App.Name).Substring(0,1).ToUpperInvariant()}else{'?'}; $letter.Foreground=$HostWindow.Resources['Foreground']; $letter.FontSize=[Math]::Max(16,[int]($IconSize/2.2)); $letter.FontWeight='SemiBold'; $letter.HorizontalAlignment='Center'; $letter.VerticalAlignment='Center'; $fallback.Child=$letter; [void]$grid.Children.Add($fallback)}
    if([bool]$App.IsRunning){$dot=New-Object System.Windows.Shapes.Ellipse; $dot.Width=5; $dot.Height=5; $dot.Fill=$HostWindow.Resources['Foreground']; $dot.HorizontalAlignment='Center'; $dot.VerticalAlignment='Bottom'; [void]$grid.Children.Add($dot)}
    $button.Content=$grid
    $button.Add_Click({param($sender,$eventArgs); $newWindow=([System.Windows.Input.Keyboard]::Modifiers -band [System.Windows.Input.ModifierKeys]::Control) -ne 0; Invoke-DashApp -App $sender.Tag -NewWindow:$newWindow; if($script:ActivitiesWindow -and $script:ActivitiesWindow.IsVisible){Hide-Activities}})
    $menu=New-Object System.Windows.Controls.ContextMenu
    $newItem=New-Object System.Windows.Controls.MenuItem; $newItem.Header='New Window'; $newItem.Tag=$App; $newItem.Add_Click({param($sender,$eventArgs); Invoke-DashApp -App $sender.Tag -NewWindow}); [void]$menu.Items.Add($newItem)
    $favoriteItem=New-Object System.Windows.Controls.MenuItem; $favoriteItem.Header=if($App.IsFavorite){'Remove from Favorites'}else{'Add to Favorites'}; $favoriteItem.Tag=$App; $favoriteItem.Add_Click({param($sender,$eventArgs); Set-DashFavorite -App $sender.Tag -Favorite (-not [bool]$sender.Tag.IsFavorite)}); [void]$menu.Items.Add($favoriteItem)
    $dockSettingsItem=New-Object System.Windows.Controls.MenuItem; $dockSettingsItem.Header='Dock Settings'; $dockSettingsItem.Add_Click({Toggle-AppearancePopover}); [void]$menu.Items.Add($dockSettingsItem)
    $button.ContextMenu=$menu
    return $button
}

function New-ShowAppsDashButton {
    param($HostWindow,[string]$StyleName,[int]$IconSize=44)
    $button=New-Object System.Windows.Controls.Button
    $button.Style=$HostWindow.FindResource($StyleName); $button.ToolTip='Show Applications'
    $grid=New-Object System.Windows.Controls.Primitives.UniformGrid; $grid.Rows=3; $grid.Columns=3; $grid.Width=[Math]::Max(20,[int]($IconSize*0.52)); $grid.Height=$grid.Width
    for($i=0;$i -lt 9;$i++){$dot=New-Object System.Windows.Shapes.Ellipse; $dot.Width=4; $dot.Height=4; $dot.Margin='1'; $dot.Fill=$HostWindow.Resources['Foreground']; [void]$grid.Children.Add($dot)}
    $button.Content=$grid
    $button.Add_Click({Show-Activities; $script:ActivitiesMode='apps'; $script:AppGridPage=0; $script:AppGridPopulated=$false; $script:ActivitiesWindow.FindName('SearchBox').Text=''; Populate-Apps})
    $menu=New-Object System.Windows.Controls.ContextMenu
    $settingsItem=New-Object System.Windows.Controls.MenuItem; $settingsItem.Header='Dock Settings'; $settingsItem.Add_Click({Toggle-AppearancePopover}); [void]$menu.Items.Add($settingsItem)
    $button.ContextMenu=$menu
    return $button
}

function Render-OverviewDash {
    if($null -eq $script:ActivitiesWindow){return}
    $panel=$script:ActivitiesWindow.FindName('DashPanel'); if(-not $panel){return}
    $showApps=$script:ActivitiesWindow.FindName('ShowAppsButton')
    $panel.Children.Clear()
    foreach($app in @(Get-FedoraDashItems)){[void]$panel.Children.Add((New-DashButton -App $app -HostWindow $script:ActivitiesWindow -StyleName 'DashButtonStyle' -IconSize 42))}
    [void]$panel.Children.Add($showApps)
    $border=$script:ActivitiesWindow.FindName('OverviewDashBorder'); if($border){$border.Visibility=if($script:DockMode -eq 'desktop'){'Collapsed'}else{'Visible'}}
}

function Update-DesktopDockPosition {
    if($null -eq $script:DockWindow){return}
    $screen=[System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
    $panel=$script:DockWindow.FindName('DesktopDockPanel')
    $vertical=$script:DockPosition -in @('left','right')
    $panel.Orientation=if($vertical){'Vertical'}else{'Horizontal'}
    $script:DockWindow.UpdateLayout()
    $width=[Math]::Max(1,$script:DockWindow.ActualWidth); $height=[Math]::Max(1,$script:DockWindow.ActualHeight)
    if($script:DockPosition -eq 'left'){$script:DockWindow.Left=$screen.Left+12; $script:DockWindow.Top=$screen.Top+(($screen.Height-$height)/2)}
    elseif($script:DockPosition -eq 'right'){$script:DockWindow.Left=$screen.Right-$width-12; $script:DockWindow.Top=$screen.Top+(($screen.Height-$height)/2)}
    else{$script:DockWindow.Left=$screen.Left+(($screen.Width-$width)/2); $script:DockWindow.Top=$screen.Bottom-$height-12}
}

function Render-DesktopDock {
    if($null -eq $script:DockWindow){return}
    $panel=$script:DockWindow.FindName('DesktopDockPanel'); if(-not $panel){return}
    $items=@(Get-FedoraDashItems)
    $screen=[System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
    $available=if($script:DockPosition -in @('left','right')){$screen.Height-80}else{$screen.Width-80}
    $count=[Math]::Max(1,$items.Count+1)
    $iconSize=[Math]::Max(32,[Math]::Min($script:DockIconSize,[int](($available/$count)-12)))
    $panel.Children.Clear()
    foreach($app in $items){[void]$panel.Children.Add((New-DashButton -App $app -HostWindow $script:DockWindow -StyleName 'DockButtonStyle' -IconSize $iconSize))}
    [void]$panel.Children.Add((New-ShowAppsDashButton -HostWindow $script:DockWindow -StyleName 'DockButtonStyle' -IconSize $iconSize))
    $script:DockWindow.Topmost=$script:DockTopmost
    Update-DesktopDockPosition
}

function Initialize-DesktopDock {
    if($script:DockMode -eq 'overview'){return}
    if($null -eq $script:DockWindow){
        $script:DockWindow=Import-XamlWindow -Path (Join-Path $script:Root 'ui\Dock.xaml')
        Apply-ThemeToWindow -Window $script:DockWindow
        $surface=$script:DockWindow.FindName('DockSurface')
        if($surface){
            $surfaceMenu=New-Object System.Windows.Controls.ContextMenu
            $surfaceSettings=New-Object System.Windows.Controls.MenuItem
            $surfaceSettings.Header='Dock Settings'
            $surfaceSettings.Add_Click({Toggle-AppearancePopover})
            [void]$surfaceMenu.Items.Add($surfaceSettings)
            $surface.ContextMenu=$surfaceMenu
        }
        Render-DesktopDock
    }
    if(-not ($script:ActivitiesWindow -and $script:ActivitiesWindow.IsVisible)){$script:DockWindow.Show(); $script:DockWindow.UpdateLayout(); Update-DesktopDockPosition}
}

function Update-DockModeVisibility {
    if($script:ActivitiesWindow){$border=$script:ActivitiesWindow.FindName('OverviewDashBorder'); if($border){$border.Visibility=if($script:DockMode -eq 'desktop'){'Collapsed'}else{'Visible'}}}
    if($script:DockMode -eq 'overview'){if($script:DockWindow){$script:DockWindow.Hide()}}
    else{Initialize-DesktopDock; if($script:ActivitiesWindow -and $script:ActivitiesWindow.IsVisible){$script:DockWindow.Hide()}elseif($script:DockWindow){$script:DockWindow.Show(); Update-DesktopDockPosition}}
}

function Refresh-Docks {
    param([switch]$Force)
    $items=@(Get-FedoraDashItems)
    $signature=(($items | ForEach-Object { '{0}:{1}:{2}' -f $_.FavoriteKey,$_.ProcessName,$_.IsRunning }) -join '|')+'|'+$script:DockMode+'|'+$script:DockPosition+'|'+$script:ThemeMode+'|'+$script:AccentName
    if(-not $Force -and $signature -eq $script:DockSignature){return}
    $script:DockSignature=$signature
    if($script:ActivitiesWindow){Render-OverviewDash}
    if($script:DockWindow){Render-DesktopDock}
}

function Build-Dash {
    if($null -eq $script:ActivitiesWindow){return}
    Render-OverviewDash
    Update-DockModeVisibility
}

function Set-FedoraDockMode {
    param([string]$Mode)
    if($Mode -notin @('overview','desktop','both')){return}
    $script:DockMode=$Mode; Save-FedoraWinSettings; Update-DockModeVisibility; Refresh-Docks -Force; Update-AppearanceControls
}

function Set-FedoraDockPosition {
    param([string]$Position)
    if($Position -notin @('bottom','left','right')){return}
    $script:DockPosition=$Position; Save-FedoraWinSettings; if($script:DockWindow){Render-DesktopDock}; Update-AppearanceControls
}

function Set-WorkspacePresenterVisible {
    param([bool]$Visible)
    if ($script:WorkspacePresenter) {
        try { $script:WorkspacePresenter.SetVisible($Visible) } catch { }
    }
}

function Refresh-WorkspacePresenter {
    if ($null -eq $script:ActivitiesWindow -or -not $script:ActivitiesWindow.IsVisible) { return 0 }
    try {
        if ($null -eq $script:WorkspacePresenter) {
            $helper = New-Object System.Windows.Interop.WindowInteropHelper -ArgumentList $script:ActivitiesWindow
            if ($helper.Handle -eq [IntPtr]::Zero) { return 0 }
            $script:WorkspacePresenter = New-Object FedoraWinWorkspacePresenter -ArgumentList $helper.Handle,([string]$config.windowFrameExcludedProcesses)
        }

        $card = $script:ActivitiesWindow.FindName('WorkspaceCard')
        if (-not $card) { return 0 }
        $script:ActivitiesWindow.UpdateLayout()
        $origin = $card.TranslatePoint((New-Object System.Windows.Point -ArgumentList 0,0), $script:ActivitiesWindow)
        $dpi = [System.Windows.Media.VisualTreeHelper]::GetDpi($script:ActivitiesWindow)
        $left = [int][Math]::Round($origin.X * $dpi.DpiScaleX)
        $top = [int][Math]::Round($origin.Y * $dpi.DpiScaleY)
        $width = [int][Math]::Round($card.ActualWidth * $dpi.DpiScaleX)
        $height = [int][Math]::Round($card.ActualHeight * $dpi.DpiScaleY)
        return $script:WorkspacePresenter.Refresh($left,$top,$width,$height)
    } catch {
        Write-FedoraWinLog 'warn' ('Live workspace thumbnails unavailable: ' + $_.Exception.Message)
        return 0
    }
}

function Hide-Activities {
    if ($script:WorkspacePresenter) { try { $script:WorkspacePresenter.SetVisible($false) } catch { } }
    if ($null -ne $script:ActivitiesWindow) { $script:ActivitiesWindow.Hide() }
    if ($script:DockMode -ne 'overview' -and $script:DockWindow) { $script:DockWindow.Show(); Update-DesktopDockPosition }
}

function Show-Activities {
    if ($script:DockWindow) { $script:DockWindow.Hide() }
    if (-not $script:InstalledAppsLoaded) {
        Write-FedoraWinLog 'info' 'Discovering launcher applications.'
        $script:InstalledApps = Get-StartMenuApps
        $script:InstalledAppsLoaded = $true
        Write-FedoraWinLog 'info' ('Discovered {0} user-facing applications.' -f $script:InstalledApps.Count)
        Refresh-Docks -Force
    }

    if ($null -eq $script:ActivitiesWindow) {
        $script:ActivitiesWindow = Import-XamlWindow -Path (Join-Path $script:Root 'ui\Activities.xaml')
        Apply-ThemeToWindow -Window $script:ActivitiesWindow
        if ($SafeMode) {
            $workArea = [System.Windows.SystemParameters]::WorkArea
            $script:ActivitiesWindow.Left = $workArea.Left
            $script:ActivitiesWindow.Top = $workArea.Top
            $script:ActivitiesWindow.Width = $workArea.Width
            $script:ActivitiesWindow.Height = [Math]::Max(300, $workArea.Height)
        } else {
            $script:ActivitiesWindow.Left = 0
            $script:ActivitiesWindow.Top = [double]$config.panelHeight
            $script:ActivitiesWindow.Width = [System.Windows.SystemParameters]::PrimaryScreenWidth
            $script:ActivitiesWindow.Height = [Math]::Max(300, [System.Windows.SystemParameters]::PrimaryScreenHeight - [double]$config.panelHeight)
        }
        $card = $script:ActivitiesWindow.FindName('WorkspaceCard')
        $workspaceWidth = [Math]::Max(520, [Math]::Min(760, $script:ActivitiesWindow.Width - 220))
        $workspaceHeight = [Math]::Round($workspaceWidth * 9.0 / 16.0)
        $maxWorkspaceHeight = [Math]::Max(292, $script:ActivitiesWindow.Height - 210)
        if ($workspaceHeight -gt $maxWorkspaceHeight) {
            $workspaceHeight = $maxWorkspaceHeight
            $workspaceWidth = [Math]::Round($workspaceHeight * 16.0 / 9.0)
        }
        $card.Width = $workspaceWidth
        $card.Height = $workspaceHeight

        $script:ActivitiesSearchBox = $script:ActivitiesWindow.FindName('SearchBox')
        $clear = $script:ActivitiesWindow.FindName('ClearSearchButton')
        $showApps = $script:ActivitiesWindow.FindName('ShowAppsButton')
        $back = $script:ActivitiesWindow.FindName('BackToOverviewButton')
        $workspace = $script:ActivitiesWindow.FindName('WorkspaceButton')
        $previousWorkspace = $script:ActivitiesWindow.FindName('PreviousWorkspaceButton')
        $nextWorkspace = $script:ActivitiesWindow.FindName('NextWorkspaceButton')
        $newWorkspace = $script:ActivitiesWindow.FindName('NewWorkspaceButton')

        $script:ActivitiesSearchBox.Add_TextChanged({
            param($sender,$eventArgs)
            try { Populate-Apps -Filter $sender.Text }
            catch { Write-FedoraWinLog 'error' ('Activities search failed: ' + $_.Exception.ToString()) }
        })
        $clear.Add_Click({
            try {
                $box = $script:ActivitiesWindow.FindName('SearchBox')
                $box.Text = ''
                $box.Focus() | Out-Null
            } catch { Write-FedoraWinLog 'error' ('Clear search failed: ' + $_.Exception.ToString()) }
        })
        $showApps.Add_Click({
            try {
                if ($script:ActivitiesMode -eq 'apps') {
                    $script:ActivitiesMode = 'overview'
                } else {
                    $script:ActivitiesMode = 'apps'
                    $script:AppGridPage = 0
                    $script:AppGridPopulated = $false
                }
                $script:ActivitiesWindow.FindName('SearchBox').Text = ''
                Populate-Apps
            } catch { Write-FedoraWinLog 'error' ('Show applications toggle failed: ' + $_.Exception.ToString()) }
        })
        $back.Add_Click({
            try {
                $script:ActivitiesMode = 'overview'
                $script:ActivitiesWindow.FindName('SearchBox').Text = ''
                Populate-Apps
            } catch { Write-FedoraWinLog 'error' ('Back to overview failed: ' + $_.Exception.ToString()) }
        })
        $workspace.Add_PreviewMouseLeftButtonDown({
            param($sender,$eventArgs)
            try {
                if ($script:WorkspacePresenter) {
                    $point = $eventArgs.GetPosition($script:ActivitiesWindow)
                    $dpi = [System.Windows.Media.VisualTreeHelper]::GetDpi($script:ActivitiesWindow)
                    $x = [int][Math]::Round($point.X * $dpi.DpiScaleX)
                    $y = [int][Math]::Round($point.Y * $dpi.DpiScaleY)
                    if ($script:WorkspacePresenter.ActivateAt($x,$y)) {
                        Hide-Activities
                        $eventArgs.Handled = $true
                    }
                }
            } catch { Write-FedoraWinLog 'warn' ('Workspace thumbnail activation failed: ' + $_.Exception.Message) }
        })
        $workspace.Add_Click({
            try { Hide-Activities; [FedoraWinNative]::SendWinTab() }
            catch { Write-FedoraWinLog 'error' ('Workspace action failed: ' + $_.Exception.ToString()) }
        })
        if ($previousWorkspace) {
            $previousWorkspace.Add_Click({
                try { Hide-Activities; [FedoraWinNative]::SendDesktopLeft() }
                catch { Write-FedoraWinLog 'error' ('Previous workspace action failed: ' + $_.Exception.ToString()) }
            })
        }
        if ($nextWorkspace) {
            $nextWorkspace.Add_Click({
                try { Hide-Activities; [FedoraWinNative]::SendDesktopRight() }
                catch { Write-FedoraWinLog 'error' ('Next workspace action failed: ' + $_.Exception.ToString()) }
            })
        }
        if ($newWorkspace) {
            $newWorkspace.Add_Click({
                try { Hide-Activities; [FedoraWinNative]::SendDesktopNew() }
                catch { Write-FedoraWinLog 'error' ('New workspace action failed: ' + $_.Exception.ToString()) }
            })
        }
        $script:ActivitiesWindow.Add_KeyDown({
            param($sender,$e)
            try {
                if ($script:ActivitiesMode -eq 'apps' -and $e.Key -eq [System.Windows.Input.Key]::PageDown) {
                    Move-AppGridPage -Delta 1; $e.Handled = $true; return
                }
                if ($script:ActivitiesMode -eq 'apps' -and $e.Key -eq [System.Windows.Input.Key]::PageUp) {
                    Move-AppGridPage -Delta -1; $e.Handled = $true; return
                }
                if ($script:ActivitiesMode -eq 'apps' -and $e.Key -eq [System.Windows.Input.Key]::Right) {
                    Move-AppGridPage -Delta 1; $e.Handled = $true; return
                }
                if ($script:ActivitiesMode -eq 'apps' -and $e.Key -eq [System.Windows.Input.Key]::Left) {
                    Move-AppGridPage -Delta -1; $e.Handled = $true; return
                }
                if ($e.Key -eq [System.Windows.Input.Key]::Escape) {
                    $box = $script:ActivitiesWindow.FindName('SearchBox')
                    if ($script:ActivitiesMode -eq 'apps' -or -not [string]::IsNullOrWhiteSpace([string]$box.Text)) {
                        $script:ActivitiesMode = 'overview'
                        $box.Text = ''
                        Populate-Apps
                    } else {
                        Hide-Activities
                    }
                    $e.Handled = $true
                }
            } catch { Write-FedoraWinLog 'error' ('Activities keyboard handler failed: ' + $_.Exception.ToString()) }
        })
        $script:ActivitiesWindow.Add_PreviewMouseWheel({
            param($sender,$eventArgs)
            if ($script:ActivitiesMode -ne 'apps') { return }
            if ($eventArgs.Delta -lt 0) { Move-AppGridPage -Delta 1 }
            elseif ($eventArgs.Delta -gt 0) { Move-AppGridPage -Delta -1 }
            $eventArgs.Handled = $true
        })
        Build-Dash
    }

    $script:ActivitiesMode = 'overview'
    $script:ActivitiesWindow.FindName('SearchBox').Text = ''
    Populate-Apps
    $script:ActivitiesWindow.Show()
    $script:ActivitiesWindow.Activate() | Out-Null
    $script:ActivitiesWindow.UpdateLayout()
    $liveCount = Refresh-WorkspacePresenter
    $previewControl = $script:ActivitiesWindow.FindName('WorkspacePreview')
    if ($liveCount -gt 0) {
        $previewControl.Source = $null
        $previewControl.Visibility = 'Collapsed'
        Set-WorkspacePresenterVisible -Visible $true
    } else {
        $previewControl.Visibility = 'Visible'
        $preview = Get-ScreenCaptureSource
        if ($preview) { $previewControl.Source = $preview }
    }
    $script:ActivitiesWindow.FindName('SearchBox').Focus() | Out-Null
}

function Toggle-Activities {
    if ($null -ne $script:ActivitiesWindow -and $script:ActivitiesWindow.IsVisible) {
        Hide-Activities
    } else {
        Show-Activities
    }
}

function Hide-TopPopovers {
    if ($null -ne $script:CalendarWindow) { $script:CalendarWindow.Hide() }
    if ($null -ne $script:QuickWindow) { $script:QuickWindow.Hide() }
    if ($null -ne $script:AppearanceWindow) { $script:AppearanceWindow.Hide() }
    if ($null -ne $script:PowerWindow) { $script:PowerWindow.Hide() }
}

function Render-CalendarMonth {
    if ($null -eq $script:CalendarWindow) { return }
    $panel = $script:CalendarWindow.FindName('CalendarDaysPanel')
    $panel.Children.Clear()
    $first = $script:CalendarMonth
    $script:CalendarWindow.FindName('MonthTitle').Text = $first.ToString('MMMM yyyy')
    $offset = (([int]$first.DayOfWeek + 6) % 7)
    $start = $first.AddDays(-$offset)
    $today = (Get-Date).Date

    for ($i = 0; $i -lt 42; $i++) {
        $d = $start.AddDays($i)
        $button = New-Object System.Windows.Controls.Button
        $button.Style = $script:CalendarWindow.FindResource('DayButtonStyle')
        $button.Content = $d.Day.ToString()
        $button.Tag = $d
        if ($d.Month -ne $first.Month) { $button.Foreground = $script:CalendarWindow.Resources['Muted']; $button.Opacity = 0.45 }
        elseif ($d.DayOfWeek -eq [DayOfWeek]::Saturday -or $d.DayOfWeek -eq [DayOfWeek]::Sunday) { $button.Foreground = $script:CalendarWindow.Resources['Muted'] }
        else { $button.Foreground = $script:CalendarWindow.Resources['Foreground'] }
        if ($d -eq $today) {
            $button.Background = $script:CalendarWindow.Resources['Accent']
            $button.Foreground = $script:CalendarWindow.Resources['AccentForeground']
            $button.Opacity = 1.0
        }
        [void]$panel.Children.Add($button)
    }
}

function Toggle-CalendarPopover {
    if ($null -ne $script:CalendarWindow -and $script:CalendarWindow.IsVisible) {
        $script:CalendarWindow.Hide()
        return
    }
    if ($null -ne $script:QuickWindow) { $script:QuickWindow.Hide() }
    if ($null -eq $script:CalendarWindow) {
        $script:CalendarWindow = Import-XamlWindow -Path (Join-Path $script:Root 'ui\Calendar.xaml')
        Apply-ThemeToWindow -Window $script:CalendarWindow
        $script:CalendarWindow.Top = ([double]$config.panelHeight + 8)
        $script:CalendarWindow.Left = [Math]::Max(8, ([System.Windows.SystemParameters]::PrimaryScreenWidth - $script:CalendarWindow.Width) / 2)
        $script:CalendarWindow.FindName('PrevMonthButton').Add_Click({ $script:CalendarMonth = $script:CalendarMonth.AddMonths(-1); Render-CalendarMonth })
        $script:CalendarWindow.FindName('NextMonthButton').Add_Click({ $script:CalendarMonth = $script:CalendarMonth.AddMonths(1); Render-CalendarMonth })
        $script:CalendarWindow.FindName('OpenCalendarButton').Add_Click({ Start-Target -Target 'outlookcal:'; $script:CalendarWindow.Hide() })
        $script:CalendarWindow.Add_Deactivated({ if ($script:CalendarWindow.IsVisible) { $script:CalendarWindow.Hide() } })
    }
    $now = Get-Date
    $script:CalendarMonth = Get-Date -Year $now.Year -Month $now.Month -Day 1
    $script:CalendarWindow.FindName('WeekdayTitle').Text = $now.ToString('dddd')
    $script:CalendarWindow.FindName('DateTitle').Text = $now.ToString('MMMM d')
    $script:CalendarWindow.FindName('YearTitle').Text = $now.ToString('yyyy')
    Render-CalendarMonth
    $script:CalendarWindow.Show()
    $script:CalendarWindow.Activate() | Out-Null
}

function Toggle-QuickPopover {
    if ($null -ne $script:QuickWindow -and $script:QuickWindow.IsVisible) {
        $script:QuickWindow.Hide()
        return
    }
    if ($null -ne $script:CalendarWindow) { $script:CalendarWindow.Hide() }
    if ($null -eq $script:QuickWindow) {
        $script:QuickWindow = Import-XamlWindow -Path (Join-Path $script:Root 'ui\QuickSettings.xaml')
        Apply-ThemeToWindow -Window $script:QuickWindow
        $script:QuickWindow.Top = ([double]$config.panelHeight + 8)
        $script:QuickWindow.Left = [Math]::Max(8, [System.Windows.SystemParameters]::PrimaryScreenWidth - $script:QuickWindow.Width - 10)

        $script:QuickWindow.FindName('WifiButton').Add_Click({ [void](Toggle-RadioState -Kind 'WiFi'); Refresh-QuickSettingsState })
        $script:QuickWindow.FindName('BluetoothButton').Add_Click({ [void](Toggle-RadioState -Kind 'Bluetooth'); Refresh-QuickSettingsState })
        $script:QuickWindow.FindName('PowerModeButton').Add_Click({ [void](Cycle-PowerMode); Refresh-QuickSettingsState })
        $script:QuickWindow.FindName('DarkStyleButton').Add_Click({
            try {
                if ((Get-ResolvedThemeMode) -eq 'dark') { Set-FedoraThemeMode -Mode 'light' } else { Set-FedoraThemeMode -Mode 'dark' }
            } catch { Write-FedoraWinLog 'warn' ('Appearance toggle failed: ' + $_.Exception.Message) }
        })
        $script:QuickWindow.FindName('AirplaneButton').Add_Click({ $off = ((Get-RadioState -Kind 'WiFi') -eq 'On' -or (Get-RadioState -Kind 'Bluetooth') -eq 'On'); $state = if ($off) { 'Off' } else { 'On' }; [void](Set-RadioState -Kind 'WiFi' -State $state); [void](Set-RadioState -Kind 'Bluetooth' -State $state); Refresh-QuickSettingsState })
        $script:QuickWindow.FindName('SettingsButton').Add_Click({ Start-Target -Target 'ms-settings:'; $script:QuickWindow.Hide() })
        $script:QuickWindow.FindName('QuickLockButton').Add_Click({ $script:QuickWindow.Hide(); Invoke-PowerAction -Action 'Lock' })
        $script:QuickWindow.FindName('MoreSettingsButton').Add_Click({ Start-Target -Target 'ms-settings:'; $script:QuickWindow.Hide() })
        $script:QuickWindow.FindName('PowerButton').Add_Click({ $script:QuickWindow.Hide(); Toggle-PowerPopover })
        $script:QuickWindow.FindName('ScreenshotButton').Add_Click({ Start-Target -Target 'ms-screenclip:'; $script:QuickWindow.Hide() })

        $script:QuickWindow.FindName('AppearanceButton').Add_Click({ Toggle-AppearancePopover })
        Update-AppearanceControls

        $volume = $script:QuickWindow.FindName('VolumeSlider')
        if ($script:AudioAvailable) {
            try {
                $script:SuppressVolumeEvent = $true
                $volume.Value = [Math]::Round([FedoraWinAudio]::GetMasterVolume() * 100)
            } catch { Write-FedoraWinLog 'warn' ('Volume read unavailable: ' + $_.Exception.Message) }
            finally { $script:SuppressVolumeEvent = $false }
            $volume.Add_ValueChanged({
                param($sender,$eventArgs)
                if (-not $script:SuppressVolumeEvent) {
                    try { [FedoraWinAudio]::SetMasterVolume(([double]$sender.Value / 100.0)) } catch { }
                }
            })
        } else {
            $volume.IsEnabled = $false
            $volume.Opacity = 0.45
        }

        $brightness = $script:QuickWindow.FindName('BrightnessSlider')
        $b = Get-DisplayBrightness
        if ($null -ne $b) { $brightness.Value = $b } else { $brightness.IsEnabled = $false; $brightness.Opacity = 0.45 }
        $brightness.Add_ValueChanged({
            param($sender,$eventArgs)
            if (-not $script:SuppressBrightnessEvent -and $sender.IsEnabled) { Set-DisplayBrightness -Value ([int][Math]::Round($sender.Value)) }
        })
        $script:QuickWindow.Add_Deactivated({ if ($script:QuickWindow.IsVisible) { $script:QuickWindow.Hide() } })
    }

    $battery = Get-BatteryPercent
    if ($null -ne $battery) {
        $script:QuickWindow.FindName('BatteryText').Text = ('{0}%' -f $battery)
        $script:QuickWindow.FindName('BatteryFill').Width = [Math]::Max(2, 16 * $battery / 100.0)
    } else {
        $script:QuickWindow.FindName('BatteryText').Text = 'Power'
    }
    Update-AppearanceControls
    Refresh-QuickSettingsState

    $script:QuickWindow.Show()
    $script:QuickWindow.Activate() | Out-Null
}

function Stop-FedoraWin {
    if ($script:Exiting) { return }
    $script:Exiting = $true

    if ($script:PanelHandle -ne [IntPtr]::Zero) {
        if ($script:HotkeyRegistered) {
            try { [FedoraWinNative]::UnregisterHotKey($script:PanelHandle, $script:HotkeyId) | Out-Null } catch { }
        }
        if ($script:SuperLeftHotkeyRegistered) {
            try { [FedoraWinNative]::UnregisterHotKey($script:PanelHandle, $script:SuperLeftHotkeyId) | Out-Null } catch { }
        }
        if ($script:SuperRightHotkeyRegistered) {
            try { [FedoraWinNative]::UnregisterHotKey($script:PanelHandle, $script:SuperRightHotkeyId) | Out-Null } catch { }
        }
    }

    if ($script:DockTimer) { try { $script:DockTimer.Stop() } catch { }; $script:DockTimer = $null }
    if ($script:WorkspacePresenter) { try { $script:WorkspacePresenter.Dispose() } catch { }; $script:WorkspacePresenter = $null }
    if ($script:WorkspacePresenter) { try { $script:WorkspacePresenter.Dispose() } catch { }; $script:WorkspacePresenter = $null }
    if ($script:FrameTimer) { try { $script:FrameTimer.Stop() } catch { }; $script:FrameTimer = $null }
    if ($script:FrameManager) { try { $script:FrameManager.Dispose() } catch { }; $script:FrameManager = $null }
    if ($script:AppBarRegistered -and $script:PanelHandle -ne [IntPtr]::Zero) {
        try { [FedoraWinAppBar]::Remove($script:PanelHandle) } catch { }
        $script:AppBarRegistered = $false
    }

    try { Restore-DesktopState } catch { try { Show-WindowsTaskbar } catch { } }

    if ($script:TrayIcon) {
        try { $script:TrayIcon.Visible = $false } catch { }
        try { $script:TrayIcon.Dispose() } catch { }
        $script:TrayIcon = $null
    }

    Remove-Item -Force -ErrorAction SilentlyContinue $script:PidPath

    if ($null -ne $script:ActivitiesWindow) { try { $script:ActivitiesWindow.Close() } catch { } }
    if ($null -ne $script:CalendarWindow) { try { $script:CalendarWindow.Close() } catch { } }
    if ($null -ne $script:QuickWindow) { try { $script:QuickWindow.Close() } catch { } }
    if ($null -ne $script:AppearanceWindow) { try { $script:AppearanceWindow.Close() } catch { } }
    if ($null -ne $script:PowerWindow) { try { $script:PowerWindow.Close() } catch { } }
    if ($null -ne $script:DockWindow) { try { $script:DockWindow.Close() } catch { }; $script:DockWindow = $null }
    if ($null -ne $script:PanelWindow) { try { $script:PanelWindow.Close() } catch { } }

    if ($script:Mutex) {
        try { $script:Mutex.ReleaseMutex() } catch { }
        try { $script:Mutex.Dispose() } catch { }
        $script:Mutex = $null
    }
}

Load-FedoraWinSettings
Write-FedoraWinLog 'info' ('Appearance: theme={0}; resolved={1}; accent={2}' -f $script:ThemeMode,(Get-ResolvedThemeMode),$script:AccentName)

$taskbarHandle = Get-TaskbarHandle
if ($taskbarHandle -ne [IntPtr]::Zero) {
    $script:TaskbarWasVisible = [FedoraWinNative]::IsWindowVisible($taskbarHandle)
}
$script:OriginalWallpaper = Get-CurrentWallpaper
$script:OriginalWorkArea = [FedoraWinNative]::GetWorkArea()
$desktopIconsHandle = Get-DesktopIconsHandle
if ($desktopIconsHandle -ne [IntPtr]::Zero) { $script:DesktopIconsWereVisible = [FedoraWinNative]::IsWindowVisible($desktopIconsHandle) }
Save-State

try {
if (-not $SafeMode) {
    if ([bool]$config.applyWallpaperWhileRunning) {
        $sessionWallpaper = Ensure-FedoraWinWallpaper
        if ($sessionWallpaper) { Set-Wallpaper -Path $sessionWallpaper }
    }
    if ([bool]$config.hideTaskbarWhileRunning) {
        Hide-WindowsTaskbar
        # Explorer's taskbar appbar can leave its old reserved strip behind after
        # being hidden. Expand the temporary work area first; our own top AppBar
        # is registered after the panel HWND exists.
        try {
            $bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
            [FedoraWinNative]::SetWorkArea($bounds.Left, $bounds.Top, $bounds.Right, $bounds.Bottom) | Out-Null
        } catch { Write-FedoraWinLog 'warn' ('Could not release taskbar work area: ' + $_.Exception.Message) }
    }
    if ([bool]$config.hideDesktopIconsWhileRunning) { Hide-DesktopIcons }
}

Write-FedoraWinLog 'info' 'Loading top panel.'
$script:PanelWindow = Import-XamlWindow -Path (Join-Path $script:Root 'ui\Panel.xaml')
$script:PanelWindow.Left = 0
$script:PanelWindow.Top = 0
$script:PanelWindow.Width = [System.Windows.SystemParameters]::PrimaryScreenWidth
$script:PanelWindow.Height = [double]$config.panelHeight
Write-FedoraWinLog 'info' ('Panel prepared: {0}x{1} DIPs.' -f $script:PanelWindow.Width, $script:PanelWindow.Height)

Apply-ThemeToWindow -Window $script:PanelWindow

$activitiesButton = $script:PanelWindow.FindName('ActivitiesButton')
$clockButton = $script:PanelWindow.FindName('ClockButton')
$quickButton = $script:PanelWindow.FindName('QuickButton')

$activitiesButton.Add_Click({ Toggle-Activities })
$clockButton.Add_Click({ Toggle-CalendarPopover })
$quickButton.Add_Click({ Toggle-QuickPopover })

$clockTimer = New-Object System.Windows.Threading.DispatcherTimer
$clockTimer.Interval = [TimeSpan]::FromSeconds(1)
$clockTimer.Add_Tick({
    $clockButton.Content = (Get-Date).ToString('ddd HH:mm')
    $battery = Get-BatteryPercent
    if ($null -ne $battery) {
        $fill = $script:PanelWindow.FindName('PanelBatteryFill')
        if ($fill) { $fill.Width = [Math]::Max(2, 16 * $battery / 100.0) }
    }
    if (Test-Path -LiteralPath $script:RestoreRequestPath) {
        Remove-Item -Force -ErrorAction SilentlyContinue $script:RestoreRequestPath
        Stop-FedoraWin
    }
})
$clockTimer.Start()
$clockButton.Content = (Get-Date).ToString('ddd HH:mm')

if ($script:DockMode -ne 'overview') { Write-FedoraWinLog 'info' 'Initializing desktop dock.'; Initialize-DesktopDock; Write-FedoraWinLog 'info' 'Desktop dock initialized.' }
$script:DockTimer = New-Object System.Windows.Threading.DispatcherTimer
$script:DockTimer.Interval = [TimeSpan]::FromSeconds(2)
$script:DockTimer.Add_Tick({ try { Refresh-Docks } catch { Write-FedoraWinLog 'warn' ('Dock refresh failed: ' + $_.Exception.Message) } })
$script:DockTimer.Start()

$script:TrayIcon = $null
if ([bool]$config.showTrayIcon) {
    try {
    $script:TrayIcon = New-Object System.Windows.Forms.NotifyIcon
    $script:TrayIcon.Text = 'FedoraWin'
    $script:TrayIcon.Icon = [System.Drawing.SystemIcons]::Application
    $script:TrayIcon.Visible = $true
    $menu = New-Object System.Windows.Forms.ContextMenuStrip
    $menuActivities = New-Object System.Windows.Forms.ToolStripMenuItem('Activities')
    $menuShowTaskbar = New-Object System.Windows.Forms.ToolStripMenuItem('Show Windows taskbar')
    $menuHideTaskbar = New-Object System.Windows.Forms.ToolStripMenuItem('Hide Windows taskbar')
    $menuRestore = New-Object System.Windows.Forms.ToolStripMenuItem('Restore and exit')
    $menuActivities.Add_Click({ Toggle-Activities })
    $menuShowTaskbar.Add_Click({ Show-WindowsTaskbar })
    $menuHideTaskbar.Add_Click({ Hide-WindowsTaskbar })
    $menuRestore.Add_Click({ Stop-FedoraWin })
    [void]$menu.Items.Add($menuActivities)
    [void]$menu.Items.Add($menuShowTaskbar)
    [void]$menu.Items.Add($menuHideTaskbar)
    [void]$menu.Items.Add($menuRestore)
    $script:TrayIcon.ContextMenuStrip = $menu
    $script:TrayIcon.Add_DoubleClick({ Toggle-Activities })
    } catch {
        Write-FedoraWinLog 'warn' ('Tray icon unavailable: ' + $_.Exception.Message)
        $script:TrayIcon = $null
    }
}

$script:PanelWindow.Add_SourceInitialized({
    try {
        $helper = New-Object System.Windows.Interop.WindowInteropHelper -ArgumentList $script:PanelWindow
        $script:PanelHandle = $helper.Handle

        # Register as a real Windows AppBar. This is the key difference from the
        # v0.2 overlay: maximized/snapped windows now receive a working area that
        # begins below the GNOME panel instead of being covered by it.
        if ((-not $SafeMode) -or [bool]$config.reservePanelInSafePreview) {
            try {
                $dpi = [FedoraWinNative]::GetDpiForSystem(); if ($dpi -le 0) { $dpi = 96 }
                $panelPixels = [int][Math]::Round(([double]$config.panelHeight) * $dpi / 96.0)
                $script:AppBarRegistered = [FedoraWinAppBar]::RegisterTop($script:PanelHandle, $panelPixels)
                if ($script:AppBarRegistered) { Write-FedoraWinLog 'info' ('Top AppBar registered: {0}px.' -f $panelPixels) }
                else { Write-FedoraWinLog 'warn' 'Top AppBar registration returned false.' }
            } catch { Write-FedoraWinLog 'warn' ('Top AppBar unavailable: ' + $_.Exception.Message) }
        }

        $source = [System.Windows.Interop.HwndSource]::FromHwnd($script:PanelHandle)
        $hook = [System.Windows.Interop.HwndSourceHook]{
            param($hwnd, $msg, $wParam, $lParam, [ref]$handled)
            if ($msg -eq [FedoraWinNative]::WM_HOTKEY) {
                $hotkeyId = $wParam.ToInt32()
                if ($hotkeyId -eq $script:HotkeyId) {
                    try { Toggle-Activities }
                    catch { Write-FedoraWinLog 'error' ('Alt+F1 Activities handler failed: ' + $_.Exception.ToString()) }
                    $handled.Value = $true
                } elseif ($hotkeyId -eq $script:SuperLeftHotkeyId -or $hotkeyId -eq $script:SuperRightHotkeyId) {
                    try { Toggle-Activities }
                    catch { Write-FedoraWinLog 'error' ('Super Activities handler failed: ' + $_.Exception.ToString()) }
                    $handled.Value = $true
                }
            }
            return [IntPtr]::Zero
        }
        $source.AddHook($hook)

        $script:HotkeyRegistered = [FedoraWinNative]::RegisterHotKey(
            $script:PanelHandle,
            $script:HotkeyId,
            [FedoraWinNative]::MOD_ALT -bor [FedoraWinNative]::MOD_NOREPEAT,
            [FedoraWinNative]::VK_F1
        )
        if (-not $script:HotkeyRegistered) {
            Write-FedoraWinLog 'warn' 'Alt+F1 could not be registered. The Activities button still works.'
        } else {
            Write-FedoraWinLog 'info' 'Alt+F1 global hotkey registered.'
        }

        # Windows reserves many Win-key combinations. Register the bare left/right
        # Super keys only through the documented hotkey API; never install a
        # low-level/global keyboard hook. If Windows refuses the reservation,
        # Activities and Alt+F1 remain fully functional.
        $script:SuperLeftHotkeyRegistered = [FedoraWinNative]::RegisterHotKey(
            $script:PanelHandle,
            $script:SuperLeftHotkeyId,
            [FedoraWinNative]::MOD_NOREPEAT,
            [FedoraWinNative]::VK_LWIN
        )
        $script:SuperRightHotkeyRegistered = [FedoraWinNative]::RegisterHotKey(
            $script:PanelHandle,
            $script:SuperRightHotkeyId,
            [FedoraWinNative]::MOD_NOREPEAT,
            [FedoraWinNative]::VK_RWIN
        )
        if ($script:SuperLeftHotkeyRegistered -or $script:SuperRightHotkeyRegistered) {
            Write-FedoraWinLog 'info' ('Super Activities hotkey registered: left={0}; right={1}.' -f $script:SuperLeftHotkeyRegistered,$script:SuperRightHotkeyRegistered)
        } else {
            Write-FedoraWinLog 'warn' 'Windows reserved the bare Super key; Alt+F1 and the Activities button remain the safe fallback.'
        }
    } catch {
        Write-FedoraWinLog 'warn' ('Panel native integration failed: ' + $_.Exception.Message)
        $script:HotkeyRegistered = $false
    }
})

if ([bool]$config.windowFrameEnabled) {
    try {
        $script:FrameManager = New-Object FedoraWinDwmFrameManager -ArgumentList ([string]$config.windowFrameExcludedProcesses)
        $script:FrameManager.SetTheme((Get-ResolvedThemeMode), $script:AccentHex)
        $script:FrameTimer = New-Object System.Windows.Threading.DispatcherTimer
        $script:FrameTimer.Interval = [TimeSpan]::FromMilliseconds(180)
        $script:FrameTimer.Add_Tick({
            try { if ($script:FrameManager) { $script:FrameManager.Refresh() } }
            catch { Write-FedoraWinLog 'warn' ('DWM frame refresh failed: ' + $_.Exception.Message) }
        })
        $script:FrameTimer.Start()
        Write-FedoraWinLog 'info' 'Native DWM frame theming enabled.'
    } catch {
        Write-FedoraWinLog 'warn' ('Native DWM frame theming unavailable: ' + $_.Exception.ToString())
        $script:FrameManager = $null
    }
}

$script:PanelWindow.Add_Closed({
    if (-not $script:Exiting) { Stop-FedoraWin }
})

    Write-FedoraWinLog 'info' 'Showing top panel.'
    [void]$script:PanelWindow.ShowDialog()
    Write-FedoraWinLog 'info' 'Top panel closed normally.'
} catch {
    $script:ExitCode = 1
    $message = $_.Exception.ToString()
    Write-FedoraWinLog 'error' $message
    try { [System.Windows.MessageBox]::Show("FedoraWin could not start.`n`n$($_.Exception.Message)`n`nLog: $script:LogPath", 'FedoraWin startup error', 'OK', 'Error') | Out-Null } catch { }
} finally {
    if ($script:FrameTimer) { try { $script:FrameTimer.Stop() } catch { }; $script:FrameTimer = $null }
    if ($script:FrameManager) { try { $script:FrameManager.Dispose() } catch { }; $script:FrameManager = $null }
    if ($script:AppBarRegistered -and $script:PanelHandle -ne [IntPtr]::Zero) {
        try { [FedoraWinAppBar]::Remove($script:PanelHandle) } catch { }
        $script:AppBarRegistered = $false
    }
    try { Restore-DesktopState } catch { Show-WindowsTaskbar }
    if ($script:TrayIcon) {
        $script:TrayIcon.Visible = $false
        $script:TrayIcon.Dispose()
    }
    Remove-Item -Force -ErrorAction SilentlyContinue $script:PidPath
    if ($script:Mutex) { try { $script:Mutex.ReleaseMutex() } catch { }; $script:Mutex.Dispose(); $script:Mutex = $null }
}

exit $script:ExitCode
