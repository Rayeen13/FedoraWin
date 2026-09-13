using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;

public static class FedoraWinAppBar
{
    const uint ABM_NEW = 0x00000000;
    const uint ABM_REMOVE = 0x00000001;
    const uint ABM_QUERYPOS = 0x00000002;
    const uint ABM_SETPOS = 0x00000003;
    const uint ABE_TOP = 1;
    const uint MONITOR_DEFAULTTONEAREST = 2;
    const uint SWP_NOACTIVATE = 0x0010;
    const uint SWP_SHOWWINDOW = 0x0040;
    static readonly IntPtr HWND_TOPMOST = new IntPtr(-1);

    [StructLayout(LayoutKind.Sequential)]
    struct RECT { public int left, top, right, bottom; }

    [StructLayout(LayoutKind.Sequential)]
    struct APPBARDATA
    {
        public int cbSize;
        public IntPtr hWnd;
        public uint uCallbackMessage;
        public uint uEdge;
        public RECT rc;
        public IntPtr lParam;
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Auto)]
    struct MONITORINFO
    {
        public int cbSize;
        public RECT rcMonitor;
        public RECT rcWork;
        public uint dwFlags;
    }

    [DllImport("shell32.dll", CallingConvention = CallingConvention.StdCall)]
    static extern uint SHAppBarMessage(uint dwMessage, ref APPBARDATA pData);

    [DllImport("user32.dll")]
    static extern IntPtr MonitorFromWindow(IntPtr hwnd, uint dwFlags);

    [DllImport("user32.dll", CharSet = CharSet.Auto)]
    static extern bool GetMonitorInfo(IntPtr hMonitor, ref MONITORINFO lpmi);

    [DllImport("user32.dll", SetLastError = true)]
    static extern bool SetWindowPos(IntPtr hWnd, IntPtr hWndInsertAfter, int X, int Y, int cx, int cy, uint uFlags);

    public static bool RegisterTop(IntPtr hwnd, int heightPixels)
    {
        if (hwnd == IntPtr.Zero || heightPixels <= 0) return false;

        APPBARDATA abd = new APPBARDATA();
        abd.cbSize = Marshal.SizeOf(typeof(APPBARDATA));
        abd.hWnd = hwnd;
        if (SHAppBarMessage(ABM_NEW, ref abd) == 0) return false;

        IntPtr monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        MONITORINFO mi = new MONITORINFO();
        mi.cbSize = Marshal.SizeOf(typeof(MONITORINFO));
        if (!GetMonitorInfo(monitor, ref mi))
        {
            SHAppBarMessage(ABM_REMOVE, ref abd);
            return false;
        }

        abd.uEdge = ABE_TOP;
        abd.rc = mi.rcMonitor;
        abd.rc.bottom = abd.rc.top + heightPixels;
        SHAppBarMessage(ABM_QUERYPOS, ref abd);
        abd.rc.bottom = abd.rc.top + heightPixels;
        if (SHAppBarMessage(ABM_SETPOS, ref abd) == 0)
        {
            SHAppBarMessage(ABM_REMOVE, ref abd);
            return false;
        }

        return SetWindowPos(hwnd, HWND_TOPMOST, abd.rc.left, abd.rc.top,
            abd.rc.right - abd.rc.left, abd.rc.bottom - abd.rc.top,
            SWP_NOACTIVATE | SWP_SHOWWINDOW);
    }

    public static void Remove(IntPtr hwnd)
    {
        if (hwnd == IntPtr.Zero) return;
        APPBARDATA abd = new APPBARDATA();
        abd.cbSize = Marshal.SizeOf(typeof(APPBARDATA));
        abd.hWnd = hwnd;
        SHAppBarMessage(ABM_REMOVE, ref abd);
    }
}

/// <summary>
/// Applies reversible Windows 11 DWM attributes to the REAL non-client frame.
/// FedoraWin never creates a fake caption overlay and never rewrites another
/// process' window style. Windows keeps ownership of caption buttons, hit tests,
/// resize borders, Snap Layouts, accessibility and per-monitor DPI behavior.
/// </summary>
public sealed class FedoraWinDwmFrameManager : IDisposable
{
    const int GWL_STYLE = -16;
    const int GWL_EXSTYLE = -20;
    const long WS_CAPTION = 0x00C00000L;
    const long WS_CHILD = 0x40000000L;
    const long WS_EX_TOOLWINDOW = 0x00000080L;
    const long WS_EX_NOACTIVATE = 0x08000000L;
    const int GW_OWNER = 4;

    const int DWMWA_CLOAKED = 14;
    const int DWMWA_USE_IMMERSIVE_DARK_MODE = 20;
    const int DWMWA_WINDOW_CORNER_PREFERENCE = 33;
    const int DWMWA_BORDER_COLOR = 34;
    const int DWMWA_CAPTION_COLOR = 35;
    const int DWMWA_TEXT_COLOR = 36;

    const int DWMWCP_DEFAULT = 0;
    const int DWMWCP_ROUND = 2;
    const int DWMWA_COLOR_DEFAULT = unchecked((int)0xFFFFFFFF);
    const int DWMWA_COLOR_NONE = unchecked((int)0xFFFFFFFE);

    readonly Dictionary<IntPtr, Snapshot> snapshots = new Dictionary<IntPtr, Snapshot>();
    readonly HashSet<string> excludedProcesses = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
    readonly int ownPid;
    bool disposed;
    bool dirty = true;
    string mode = "dark";
    Native.WinEventDelegate winEventCallback;
    IntPtr systemHook = IntPtr.Zero;
    IntPtr objectHook = IntPtr.Zero;

    struct OptionalInt
    {
        public bool HasValue;
        public int Value;
    }

    sealed class Snapshot
    {
        public OptionalInt Dark;
        public OptionalInt Corner;
        public OptionalInt Border;
        public OptionalInt Caption;
        public OptionalInt Text;
    }

    public FedoraWinDwmFrameManager(string excludedCsv)
    {
        ownPid = Process.GetCurrentProcess().Id;
        if (!String.IsNullOrWhiteSpace(excludedCsv))
        {
            string[] parts = excludedCsv.Split(new char[] { ',', ';' }, StringSplitOptions.RemoveEmptyEntries);
            foreach (string raw in parts) excludedProcesses.Add(raw.Trim());
        }

        winEventCallback = new Native.WinEventDelegate(OnWinEvent);
        const uint WINEVENT_OUTOFCONTEXT = 0x0000;
        const uint WINEVENT_SKIPOWNPROCESS = 0x0002;
        const uint flags = WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS;
        systemHook = Native.SetWinEventHook(0x0003, 0x0017, IntPtr.Zero, winEventCallback, 0, 0, flags);
        objectHook = Native.SetWinEventHook(0x8001, 0x800B, IntPtr.Zero, winEventCallback, 0, 0, flags);
    }

    void OnWinEvent(IntPtr hook, uint eventType, IntPtr hwnd, int idObject, int idChild, uint eventThread, uint eventTime)
    {
        dirty = true;
    }

    public void SetTheme(string newMode, string accentHex)
    {
        mode = String.Equals(newMode, "light", StringComparison.OrdinalIgnoreCase) ? "light" : "dark";
        dirty = true;
        foreach (IntPtr hwnd in new List<IntPtr>(snapshots.Keys))
        {
            if (Native.IsWindow(hwnd)) Apply(hwnd);
        }
    }

    public void Refresh()
    {
        if (disposed || !dirty) return;
        dirty = false;

        HashSet<IntPtr> seen = new HashSet<IntPtr>();
        Native.EnumWindows(delegate(IntPtr hwnd, IntPtr lParam)
        {
            try
            {
                if (!IsEligible(hwnd)) return true;
                seen.Add(hwnd);
                if (!snapshots.ContainsKey(hwnd)) snapshots.Add(hwnd, Capture(hwnd));
                Apply(hwnd);
            }
            catch { }
            return true;
        }, IntPtr.Zero);

        List<IntPtr> dead = new List<IntPtr>();
        foreach (IntPtr hwnd in snapshots.Keys)
        {
            if (!Native.IsWindow(hwnd)) dead.Add(hwnd);
        }
        foreach (IntPtr hwnd in dead) snapshots.Remove(hwnd);
    }

    bool IsEligible(IntPtr hwnd)
    {
        if (hwnd == IntPtr.Zero || !Native.IsWindow(hwnd) || !Native.IsWindowVisible(hwnd)) return false;
        if (Native.GetWindow(hwnd, GW_OWNER) != IntPtr.Zero) return false;

        int pid;
        Native.GetWindowThreadProcessId(hwnd, out pid);
        if (pid == 0 || pid == ownPid) return false;

        long style = Native.GetWindowStyle(hwnd);
        long exStyle = Native.GetWindowExStyle(hwnd);
        if ((style & WS_CHILD) != 0 || (style & WS_CAPTION) != WS_CAPTION) return false;
        if ((exStyle & WS_EX_TOOLWINDOW) != 0 || (exStyle & WS_EX_NOACTIVATE) != 0) return false;

        int cloaked = 0;
        if (Native.DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, out cloaked, sizeof(int)) == 0 && cloaked != 0) return false;

        StringBuilder title = new StringBuilder(512);
        Native.GetWindowText(hwnd, title, title.Capacity);
        if (String.IsNullOrWhiteSpace(title.ToString())) return false;

        try
        {
            Process p = Process.GetProcessById(pid);
            if (excludedProcesses.Contains(p.ProcessName)) return false;
        }
        catch { return false; }

        return true;
    }

    Snapshot Capture(IntPtr hwnd)
    {
        Snapshot s = new Snapshot();
        s.Dark = Read(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE);
        s.Corner = Read(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE);
        s.Border = Read(hwnd, DWMWA_BORDER_COLOR);
        s.Caption = Read(hwnd, DWMWA_CAPTION_COLOR);
        s.Text = Read(hwnd, DWMWA_TEXT_COLOR);
        return s;
    }

    static OptionalInt Read(IntPtr hwnd, int attribute)
    {
        int value = 0;
        int hr = Native.DwmGetWindowAttribute(hwnd, attribute, out value, sizeof(int));
        OptionalInt result = new OptionalInt();
        result.HasValue = (hr == 0);
        result.Value = value;
        return result;
    }

    void Apply(IntPtr hwnd)
    {
        int dark = mode == "dark" ? 1 : 0;
        int corner = DWMWCP_ROUND;
        int border = DWMWA_COLOR_NONE;
        int caption = mode == "dark" ? ColorRef(48, 48, 48) : ColorRef(246, 245, 244);
        int text = mode == "dark" ? ColorRef(255, 255, 255) : ColorRef(32, 32, 32);

        Native.DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, ref dark, sizeof(int));
        Native.DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, ref corner, sizeof(int));
        Native.DwmSetWindowAttribute(hwnd, DWMWA_BORDER_COLOR, ref border, sizeof(int));
        Native.DwmSetWindowAttribute(hwnd, DWMWA_CAPTION_COLOR, ref caption, sizeof(int));
        Native.DwmSetWindowAttribute(hwnd, DWMWA_TEXT_COLOR, ref text, sizeof(int));
    }

    static int ColorRef(int r, int g, int b)
    {
        return r | (g << 8) | (b << 16);
    }

    static void RestoreAttribute(IntPtr hwnd, int attribute, OptionalInt saved, int fallback)
    {
        int value = saved.HasValue ? saved.Value : fallback;
        Native.DwmSetWindowAttribute(hwnd, attribute, ref value, sizeof(int));
    }

    void Restore(IntPtr hwnd, Snapshot s)
    {
        if (!Native.IsWindow(hwnd)) return;
        RestoreAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, s.Dark, 0);
        RestoreAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, s.Corner, DWMWCP_DEFAULT);
        RestoreAttribute(hwnd, DWMWA_BORDER_COLOR, s.Border, DWMWA_COLOR_DEFAULT);
        RestoreAttribute(hwnd, DWMWA_CAPTION_COLOR, s.Caption, DWMWA_COLOR_DEFAULT);
        RestoreAttribute(hwnd, DWMWA_TEXT_COLOR, s.Text, DWMWA_COLOR_DEFAULT);
    }

    public void Dispose()
    {
        if (disposed) return;
        disposed = true;
        try { if (systemHook != IntPtr.Zero) Native.UnhookWinEvent(systemHook); } catch { }
        try { if (objectHook != IntPtr.Zero) Native.UnhookWinEvent(objectHook); } catch { }
        systemHook = IntPtr.Zero;
        objectHook = IntPtr.Zero;

        foreach (KeyValuePair<IntPtr, Snapshot> pair in snapshots)
        {
            try { Restore(pair.Key, pair.Value); } catch { }
        }
        snapshots.Clear();
    }

    static class Native
    {
        public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
        public delegate void WinEventDelegate(IntPtr hWinEventHook, uint eventType, IntPtr hwnd, int idObject, int idChild, uint eventThread, uint eventTime);

        [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc callback, IntPtr lParam);
        [DllImport("user32.dll")] [return: MarshalAs(UnmanagedType.Bool)] public static extern bool IsWindow(IntPtr hWnd);
        [DllImport("user32.dll")] [return: MarshalAs(UnmanagedType.Bool)] public static extern bool IsWindowVisible(IntPtr hWnd);
        [DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr hWnd, int uCmd);
        [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out int processId);
        [DllImport("user32.dll", CharSet = CharSet.Auto)] public static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int maxCount);
        [DllImport("user32.dll", EntryPoint = "GetWindowLongPtr")] static extern IntPtr GetWindowLongPtr64(IntPtr hWnd, int nIndex);
        [DllImport("user32.dll", EntryPoint = "GetWindowLong")] static extern int GetWindowLong32(IntPtr hWnd, int nIndex);
        [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr hwnd, int attribute, out int value, int size);
        [DllImport("dwmapi.dll")] public static extern int DwmSetWindowAttribute(IntPtr hwnd, int attribute, ref int value, int size);
        [DllImport("user32.dll")] public static extern IntPtr SetWinEventHook(uint eventMin, uint eventMax, IntPtr module, WinEventDelegate callback, uint processId, uint threadId, uint flags);
        [DllImport("user32.dll")] [return: MarshalAs(UnmanagedType.Bool)] public static extern bool UnhookWinEvent(IntPtr hook);

        static long GetWindowLongValue(IntPtr hwnd, int index)
        {
            if (IntPtr.Size == 8) return GetWindowLongPtr64(hwnd, index).ToInt64();
            return GetWindowLong32(hwnd, index);
        }

        public static long GetWindowStyle(IntPtr hwnd) { return GetWindowLongValue(hwnd, GWL_STYLE); }
        public static long GetWindowExStyle(IntPtr hwnd) { return GetWindowLongValue(hwnd, GWL_EXSTYLE); }
    }
}


public sealed class FedoraWinWorkspacePresenter : IDisposable
{
    const int GWL_STYLE = -16;
    const int GWL_EXSTYLE = -20;
    const long WS_CAPTION = 0x00C00000L;
    const long WS_CHILD = 0x40000000L;
    const long WS_EX_TOOLWINDOW = 0x00000080L;
    const long WS_EX_NOACTIVATE = 0x08000000L;
    const int GW_OWNER = 4;
    const int DWMWA_CLOAKED = 14;
    const uint DWM_TNP_RECTDESTINATION = 0x00000001;
    const uint DWM_TNP_OPACITY = 0x00000004;
    const uint DWM_TNP_VISIBLE = 0x00000008;
    const uint DWM_TNP_SOURCECLIENTAREAONLY = 0x00000010;
    const int SW_RESTORE = 9;

    readonly IntPtr destination;
    readonly int ownPid;
    readonly HashSet<string> excluded = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
    readonly List<Entry> entries = new List<Entry>();
    bool disposed;

    [StructLayout(LayoutKind.Sequential)]
    struct RECT { public int left, top, right, bottom; }

    [StructLayout(LayoutKind.Sequential)]
    struct PSIZE { public int x, y; }

    [StructLayout(LayoutKind.Sequential)]
    struct DWM_THUMBNAIL_PROPERTIES
    {
        public uint dwFlags;
        public RECT rcDestination;
        public RECT rcSource;
        public byte opacity;
        [MarshalAs(UnmanagedType.Bool)] public bool fVisible;
        [MarshalAs(UnmanagedType.Bool)] public bool fSourceClientAreaOnly;
    }

    sealed class Entry
    {
        public IntPtr Source;
        public IntPtr Thumbnail;
        public RECT Rect;
    }

    public FedoraWinWorkspacePresenter(IntPtr destinationHwnd, string excludedCsv)
    {
        destination = destinationHwnd;
        ownPid = Process.GetCurrentProcess().Id;
        if (!String.IsNullOrWhiteSpace(excludedCsv))
        {
            foreach (string raw in excludedCsv.Split(new char[] { ',', ';' }, StringSplitOptions.RemoveEmptyEntries))
                excluded.Add(raw.Trim());
        }
        foreach (string name in new string[] { "ShellExperienceHost", "StartMenuExperienceHost", "SearchHost", "TextInputHost" })
            excluded.Add(name);
    }

    public int Refresh(int left, int top, int width, int height)
    {
        if (disposed || destination == IntPtr.Zero || width < 80 || height < 80) return 0;
        Clear();

        List<IntPtr> windows = new List<IntPtr>();
        Native.EnumWindows(delegate(IntPtr hwnd, IntPtr lParam)
        {
            if (windows.Count >= 8) return false;
            if (IsEligible(hwnd)) windows.Add(hwnd);
            return true;
        }, IntPtr.Zero);

        if (windows.Count == 0) return 0;

        int count = windows.Count;
        int columns = count <= 1 ? 1 : (count <= 4 ? 2 : 3);
        int rows = (int)Math.Ceiling(count / (double)columns);
        int gap = Math.Max(12, Math.Min(24, width / 40));
        int cellWidth = Math.Max(80, (width - gap * (columns + 1)) / columns);
        int cellHeight = Math.Max(70, (height - gap * (rows + 1)) / rows);

        for (int i = 0; i < count; i++)
        {
            IntPtr thumbnail;
            if (Native.DwmRegisterThumbnail(destination, windows[i], out thumbnail) != 0 || thumbnail == IntPtr.Zero)
                continue;

            PSIZE sourceSize;
            if (Native.DwmQueryThumbnailSourceSize(thumbnail, out sourceSize) != 0 || sourceSize.x <= 0 || sourceSize.y <= 0)
            {
                Native.DwmUnregisterThumbnail(thumbnail);
                continue;
            }

            int column = i % columns;
            int row = i / columns;
            int cellLeft = left + gap + column * (cellWidth + gap);
            int cellTop = top + gap + row * (cellHeight + gap);

            double scale = Math.Min(cellWidth / (double)sourceSize.x, cellHeight / (double)sourceSize.y);
            if (count == 1) scale = Math.Min(scale, 0.88);
            int renderWidth = Math.Max(40, (int)Math.Round(sourceSize.x * scale));
            int renderHeight = Math.Max(40, (int)Math.Round(sourceSize.y * scale));
            int renderLeft = cellLeft + (cellWidth - renderWidth) / 2;
            int renderTop = cellTop + (cellHeight - renderHeight) / 2;

            RECT rect = new RECT {
                left = renderLeft, top = renderTop,
                right = renderLeft + renderWidth, bottom = renderTop + renderHeight
            };
            DWM_THUMBNAIL_PROPERTIES props = new DWM_THUMBNAIL_PROPERTIES {
                dwFlags = DWM_TNP_RECTDESTINATION | DWM_TNP_OPACITY | DWM_TNP_VISIBLE | DWM_TNP_SOURCECLIENTAREAONLY,
                rcDestination = rect,
                opacity = 255,
                fVisible = true,
                fSourceClientAreaOnly = false
            };

            if (Native.DwmUpdateThumbnailProperties(thumbnail, ref props) != 0)
            {
                Native.DwmUnregisterThumbnail(thumbnail);
                continue;
            }

            entries.Add(new Entry { Source = windows[i], Thumbnail = thumbnail, Rect = rect });
        }

        return entries.Count;
    }

    public void SetVisible(bool visible)
    {
        if (disposed) return;
        foreach (Entry entry in entries)
        {
            DWM_THUMBNAIL_PROPERTIES props = new DWM_THUMBNAIL_PROPERTIES {
                dwFlags = DWM_TNP_VISIBLE,
                fVisible = visible
            };
            try { Native.DwmUpdateThumbnailProperties(entry.Thumbnail, ref props); } catch { }
        }
    }

    public bool ActivateAt(int x, int y)
    {
        if (disposed) return false;
        for (int i = entries.Count - 1; i >= 0; i--)
        {
            Entry entry = entries[i];
            if (x < entry.Rect.left || x > entry.Rect.right || y < entry.Rect.top || y > entry.Rect.bottom) continue;
            if (!Native.IsWindow(entry.Source)) return false;
            Native.ShowWindow(entry.Source, SW_RESTORE);
            Native.SetForegroundWindow(entry.Source);
            return true;
        }
        return false;
    }

    bool IsEligible(IntPtr hwnd)
    {
        if (hwnd == IntPtr.Zero || !Native.IsWindow(hwnd) || !Native.IsWindowVisible(hwnd)) return false;
        if (Native.GetWindow(hwnd, GW_OWNER) != IntPtr.Zero) return false;

        int pid;
        Native.GetWindowThreadProcessId(hwnd, out pid);
        if (pid == 0 || pid == ownPid) return false;

        long style = Native.GetWindowStyle(hwnd);
        long exStyle = Native.GetWindowExStyle(hwnd);
        if ((style & WS_CHILD) != 0 || (style & WS_CAPTION) != WS_CAPTION) return false;
        if ((exStyle & WS_EX_TOOLWINDOW) != 0 || (exStyle & WS_EX_NOACTIVATE) != 0) return false;

        int cloaked = 0;
        if (Native.DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, out cloaked, sizeof(int)) == 0 && cloaked != 0) return false;

        StringBuilder title = new StringBuilder(512);
        Native.GetWindowText(hwnd, title, title.Capacity);
        if (String.IsNullOrWhiteSpace(title.ToString())) return false;

        try
        {
            Process process = Process.GetProcessById(pid);
            if (excluded.Contains(process.ProcessName)) return false;
        }
        catch { return false; }

        return true;
    }

    void Clear()
    {
        foreach (Entry entry in entries)
        {
            try { if (entry.Thumbnail != IntPtr.Zero) Native.DwmUnregisterThumbnail(entry.Thumbnail); } catch { }
        }
        entries.Clear();
    }

    public void Dispose()
    {
        if (disposed) return;
        disposed = true;
        Clear();
    }

    static class Native
    {
        public delegate bool EnumWindowsProc(IntPtr hwnd, IntPtr lParam);

        [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc callback, IntPtr lParam);
        [DllImport("user32.dll")] [return: MarshalAs(UnmanagedType.Bool)] public static extern bool IsWindow(IntPtr hwnd);
        [DllImport("user32.dll")] [return: MarshalAs(UnmanagedType.Bool)] public static extern bool IsWindowVisible(IntPtr hwnd);
        [DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr hwnd, int command);
        [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out int processId);
        [DllImport("user32.dll", CharSet = CharSet.Auto)] public static extern int GetWindowText(IntPtr hwnd, StringBuilder text, int maxCount);
        [DllImport("user32.dll")] [return: MarshalAs(UnmanagedType.Bool)] public static extern bool ShowWindow(IntPtr hwnd, int command);
        [DllImport("user32.dll")] [return: MarshalAs(UnmanagedType.Bool)] public static extern bool SetForegroundWindow(IntPtr hwnd);
        [DllImport("user32.dll", EntryPoint = "GetWindowLongPtr")] static extern IntPtr GetWindowLongPtr64(IntPtr hwnd, int index);
        [DllImport("user32.dll", EntryPoint = "GetWindowLong")] static extern int GetWindowLong32(IntPtr hwnd, int index);
        [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr hwnd, int attribute, out int value, int size);
        [DllImport("dwmapi.dll")] public static extern int DwmRegisterThumbnail(IntPtr destination, IntPtr source, out IntPtr thumbnail);
        [DllImport("dwmapi.dll")] public static extern int DwmUnregisterThumbnail(IntPtr thumbnail);
        [DllImport("dwmapi.dll")] public static extern int DwmQueryThumbnailSourceSize(IntPtr thumbnail, out PSIZE size);
        [DllImport("dwmapi.dll")] public static extern int DwmUpdateThumbnailProperties(IntPtr thumbnail, ref DWM_THUMBNAIL_PROPERTIES properties);

        static long GetWindowLongValue(IntPtr hwnd, int index)
        {
            return IntPtr.Size == 8 ? GetWindowLongPtr64(hwnd, index).ToInt64() : GetWindowLong32(hwnd, index);
        }

        public static long GetWindowStyle(IntPtr hwnd) { return GetWindowLongValue(hwnd, GWL_STYLE); }
        public static long GetWindowExStyle(IntPtr hwnd) { return GetWindowLongValue(hwnd, GWL_EXSTYLE); }
    }
}

public static class FedoraWinPowerMode
{
    public static readonly Guid BestEfficiency = new Guid("961cc777-2547-4f9d-8174-7d86181b8a7a");
    public static readonly Guid Balanced = Guid.Empty;
    public static readonly Guid BestPerformance = new Guid("ded574b5-45a0-4f42-8737-46345c09c238");

    [DllImport("powrprof.dll", EntryPoint = "PowerGetUserConfiguredACPowerMode")]
    static extern uint GetAc(out Guid mode);

    [DllImport("powrprof.dll", EntryPoint = "PowerGetUserConfiguredDCPowerMode")]
    static extern uint GetDc(out Guid mode);

    [DllImport("powrprof.dll", EntryPoint = "PowerSetUserConfiguredACPowerMode")]
    static extern uint SetAc(ref Guid mode);

    [DllImport("powrprof.dll", EntryPoint = "PowerSetUserConfiguredDCPowerMode")]
    static extern uint SetDc(ref Guid mode);

    public static string Get(bool onAcPower)
    {
        Guid mode;
        uint code = onAcPower ? GetAc(out mode) : GetDc(out mode);
        if (code != 0) return "Unavailable";
        if (mode == BestEfficiency) return "Power Saver";
        if (mode == BestPerformance) return "Performance";
        return "Balanced";
    }

    public static bool Set(string name)
    {
        Guid mode;
        if (String.Equals(name, "Power Saver", StringComparison.OrdinalIgnoreCase)) mode = BestEfficiency;
        else if (String.Equals(name, "Performance", StringComparison.OrdinalIgnoreCase)) mode = BestPerformance;
        else mode = Balanced;

        uint ac = SetAc(ref mode);
        uint dc = SetDc(ref mode);
        return ac == 0 || dc == 0;
    }
}

public static class FedoraWinSession
{
    [DllImport("user32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    static extern bool LockWorkStation();

    [DllImport("powrprof.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.U1)]
    static extern bool SetSuspendState([MarshalAs(UnmanagedType.U1)] bool hibernate, [MarshalAs(UnmanagedType.U1)] bool forceCritical, [MarshalAs(UnmanagedType.U1)] bool disableWakeEvent);

    public static bool Lock() { return LockWorkStation(); }
    public static bool Suspend() { return SetSuspendState(false, false, false); }
}
