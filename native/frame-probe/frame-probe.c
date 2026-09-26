/* Disposable, same-process Win32 Adwaita-style frame proof.
   Not a DLL, not GTK, not an injection into foreign processes. */
/* MSYS2 -municode already defines UNICODE and _UNICODE. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <windowsx.h>
#include <dwmapi.h>
#include <stdio.h>

#define PROBE_CLASS L"FedoraWinNativeFrameProbe"
#define MSG_STATE (WM_APP + 81)
#define MSG_RESTORED (WM_APP + 82)
#define MSG_STYLE (WM_APP + 83)

static WNDPROC previous_proc = NULL;
static LONG_PTR original_style = 0;
static BOOL frame_enabled = FALSE;

static LRESULT CALLBACK regular_proc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp);
static LRESULT CALLBACK frame_proc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp);

static int pixels(HWND hwnd, int value) {
    UINT dpi = GetDpiForWindow(hwnd);
    return MulDiv(value, dpi ? (int)dpi : 96, 96);
}

static void recalculate(HWND hwnd) {
    SetWindowPos(hwnd, NULL, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE |
        SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED);
    RedrawWindow(hwnd, NULL, NULL, RDW_INVALIDATE | RDW_ERASE | RDW_FRAME);
}

static BOOL enable(HWND hwnd) {
    if (frame_enabled || previous_proc ||
        GetCurrentThreadId() != GetWindowThreadProcessId(hwnd, NULL) ||
        (GetWindowLongPtrW(hwnd, GWL_STYLE) & WS_CAPTION) != WS_CAPTION)
        return FALSE;
    /* WM_CREATE predates ShowWindow, which sets WS_VISIBLE. Save the exact
       live style immediately before attachment, not at window creation. */
    original_style = GetWindowLongPtrW(hwnd, GWL_STYLE);
    SetLastError(0);
    LONG_PTR old = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, (LONG_PTR)frame_proc);
    if (!old) return FALSE;
    previous_proc = (WNDPROC)old;
    frame_enabled = TRUE;
    SetWindowTextW(hwnd, L"FedoraWin Frame Probe - ATTACHED");
    recalculate(hwnd);
    return TRUE;
}

static BOOL disable(HWND hwnd) {
    if (!frame_enabled || !previous_proc ||
        GetCurrentThreadId() != GetWindowThreadProcessId(hwnd, NULL) ||
        GetWindowLongPtrW(hwnd, GWLP_WNDPROC) != (LONG_PTR)frame_proc)
        return FALSE;
    LONG_PTR old = SetWindowLongPtrW(hwnd, GWLP_WNDPROC,
                                     (LONG_PTR)previous_proc);
    if (old != (LONG_PTR)frame_proc) return FALSE;
    previous_proc = NULL;
    frame_enabled = FALSE;
    SetWindowTextW(hwnd, L"FedoraWin Frame Probe - ORIGINAL");
    recalculate(hwnd);
    return TRUE;
}

static void paint(HWND hwnd) {
    PAINTSTRUCT ps;
    HDC dc = BeginPaint(hwnd, &ps);
    RECT rect;
    GetClientRect(hwnd, &rect);
    HBRUSH base = CreateSolidBrush(RGB(39, 39, 43));
    FillRect(dc, &rect, base);
    DeleteObject(base);
    int header = pixels(hwnd, 55);
    RECT bar = {0, 0, rect.right, header};
    HBRUSH surface = CreateSolidBrush(RGB(46, 46, 50));
    FillRect(dc, &bar, surface);
    DeleteObject(surface);
    SetBkMode(dc, TRANSPARENT);
    SetTextColor(dc, RGB(249, 249, 250));
    HFONT font = CreateFontW(-pixels(hwnd, 16), 0, 0, 0, FW_SEMIBOLD,
        FALSE, FALSE, FALSE, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS,
        CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, DEFAULT_PITCH, L"Segoe UI");
    HFONT old_font = (HFONT)SelectObject(dc, font);
    RECT title = {pixels(hwnd, 20), 0, rect.right - pixels(hwnd, 165), header};
    DrawTextW(dc, L"FedoraWin    Native frame test", -1, &title,
              DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS);
    SelectObject(dc, old_font);
    DeleteObject(font);

    HBRUSH circle = CreateSolidBrush(RGB(94, 94, 101));
    HPEN pen = CreatePen(PS_SOLID, 2, RGB(250, 250, 251));
    HBRUSH old_brush = (HBRUSH)SelectObject(dc, circle);
    HPEN old_pen = (HPEN)SelectObject(dc, pen);
    int span = pixels(hwnd, 38);
    int cy = header / 2;
    for (int i = 0; i < 3; ++i) {
        int cx = rect.right - pixels(hwnd, 12) - span / 2 - span * i;
        int radius = pixels(hwnd, 13);
        Ellipse(dc, cx - radius, cy - radius, cx + radius, cy + radius);
        int mark = pixels(hwnd, 4);
        if (i == 0) {
            MoveToEx(dc, cx-mark, cy-mark, NULL); LineTo(dc, cx+mark, cy+mark);
            MoveToEx(dc, cx+mark, cy-mark, NULL); LineTo(dc, cx-mark, cy+mark);
        } else if (i == 1) {
            Rectangle(dc, cx-mark, cy-mark, cx+mark+1, cy+mark+1);
        } else {
            MoveToEx(dc, cx-mark, cy, NULL); LineTo(dc, cx+mark+1, cy);
        }
    }
    SelectObject(dc, old_pen);
    SelectObject(dc, old_brush);
    DeleteObject(pen);
    DeleteObject(circle);
    SetTextColor(dc, RGB(220, 220, 226));
    RECT info = {pixels(hwnd, 30), header + pixels(hwnd, 30),
                 rect.right - pixels(hwnd, 30), rect.bottom};
    DrawTextW(dc,
      L"F8: restore the original Windows frame without reboot or app restart.\r\n"
      L"F8 again: attach a real native Win32 headerbar on the same GUI thread.\r\n\r\n"
      L"No foreign window or operating system frame is modified.",
      -1, &info, DT_LEFT | DT_TOP | DT_WORDBREAK);
    EndPaint(hwnd, &ps);
}

static LRESULT frame_hit(HWND hwnd, LPARAM lp) {
    RECT r;
    if (!GetWindowRect(hwnd, &r)) return HTNOWHERE;
    int x = GET_X_LPARAM(lp) - r.left;
    int y = GET_Y_LPARAM(lp) - r.top;
    int w = r.right - r.left;
    int h = r.bottom - r.top;
    int edge = IsZoomed(hwnd) ? 0 : pixels(hwnd, 8);
    if (edge && x < edge && y < edge) return HTTOPLEFT;
    if (edge && x >= w-edge && y < edge) return HTTOPRIGHT;
    if (edge && x < edge && y >= h-edge) return HTBOTTOMLEFT;
    if (edge && x >= w-edge && y >= h-edge) return HTBOTTOMRIGHT;
    if (edge && x < edge) return HTLEFT;
    if (edge && x >= w-edge) return HTRIGHT;
    if (edge && y < edge) return HTTOP;
    if (edge && y >= h-edge) return HTBOTTOM;
    if (y < pixels(hwnd, 55) + edge) {
        int size = pixels(hwnd, 38);
        int offset = w-x-pixels(hwnd, 12);
        if (offset >= 0 && offset < size) return HTCLOSE;
        if (offset >= size && offset < size*2) return HTMAXBUTTON;
        if (offset >= size*2 && offset < size*3) return HTMINBUTTON;
        return HTCAPTION;
    }
    return HTCLIENT;
}

static LRESULT CALLBACK frame_proc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp) {
    switch (msg) {
    case MSG_STATE: return frame_enabled ? 1 : 0;
    case MSG_RESTORED: return 0;
    case MSG_STYLE:
        return GetWindowLongPtrW(hwnd, GWL_STYLE) == original_style ? 1 : 0;
    case WM_KEYDOWN:
        if (wp == VK_F8) return disable(hwnd) ? 0 : 1;
        break;
    case WM_NCCALCSIZE:
        if (wp) {
            NCCALCSIZE_PARAMS *p = (NCCALCSIZE_PARAMS *)lp;
            LONG top = p->rgrc[0].top;
            LRESULT result = CallWindowProcW(previous_proc, hwnd, msg, wp, lp);
            p->rgrc[0].top = top + (IsZoomed(hwnd) ? 0 : pixels(hwnd, 8));
            return result;
        }
        break;
    case WM_NCHITTEST: {
        LRESULT result = 0;
        if (DwmDefWindowProc(hwnd, msg, wp, lp, &result) &&
            (result == HTMAXBUTTON || result == HTMINBUTTON ||
             result == HTCLOSE)) return result;
        return frame_hit(hwnd, lp);
    }
    case WM_NCLBUTTONDOWN:
        if (wp == HTCLOSE || wp == HTMINBUTTON || wp == HTMAXBUTTON) {
            WPARAM action = wp == HTCLOSE ? SC_CLOSE :
                wp == HTMINBUTTON ? SC_MINIMIZE :
                IsZoomed(hwnd) ? SC_RESTORE : SC_MAXIMIZE;
            PostMessageW(hwnd, WM_SYSCOMMAND, action, 0);
            return 0;
        }
        break;
    case WM_PAINT: paint(hwnd); return 0;
    case WM_NCDESTROY: {
        WNDPROC old = previous_proc;
        previous_proc = NULL;
        frame_enabled = FALSE;
        return old ? CallWindowProcW(old, hwnd, msg, wp, lp) : 0;
    }
    }
    return CallWindowProcW(previous_proc, hwnd, msg, wp, lp);
}

static LRESULT CALLBACK regular_proc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp) {
    switch (msg) {
    case WM_CREATE:
        original_style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        return 0;
    case MSG_STATE: return frame_enabled ? 1 : 0;
    case MSG_RESTORED:
        return !frame_enabled && !previous_proc &&
            GetWindowLongPtrW(hwnd, GWLP_WNDPROC) == (LONG_PTR)regular_proc;
    case MSG_STYLE:
        return GetWindowLongPtrW(hwnd, GWL_STYLE) == original_style ? 1 : 0;
    case WM_KEYDOWN:
        if (wp == VK_F8) return enable(hwnd) ? 0 : 1;
        break;
    case WM_PAINT: {
        PAINTSTRUCT ps;
        HDC dc = BeginPaint(hwnd, &ps);
        RECT r;
        GetClientRect(hwnd, &r);
        FillRect(dc, &r, (HBRUSH)(COLOR_WINDOW+1));
        SetBkMode(dc, TRANSPARENT);
        DrawTextW(dc, L"Original Windows frame.\r\nPress F8 to attach "
          L"the native replacement; press F8 again to restore.",
          -1, &r, DT_CENTER | DT_VCENTER | DT_WORDBREAK);
        EndPaint(hwnd, &ps);
        return 0;
    }
    case WM_DESTROY: PostQuitMessage(0); return 0;
    }
    return DefWindowProcW(hwnd, msg, wp, lp);
}

int WINAPI wWinMain(HINSTANCE instance, HINSTANCE unused,
                    LPWSTR command_line, int show) {
    (void)unused;
    (void)command_line;
    WNDCLASSEXW cls = {0};
    cls.cbSize = sizeof(cls);
    cls.lpfnWndProc = regular_proc;
    cls.hInstance = instance;
    cls.hCursor = LoadCursorW(NULL, IDC_ARROW);
    cls.hbrBackground = (HBRUSH)(COLOR_WINDOW+1);
    cls.lpszClassName = PROBE_CLASS;
    fprintf(stderr, "WinMain started PID=%lu\n", (unsigned long)GetCurrentProcessId());
    fflush(stderr);
    if (!RegisterClassExW(&cls)) {
        fprintf(stderr, "RegisterClassExW failed=%lu\n", (unsigned long)GetLastError());
        fflush(stderr);
        return 2;
    }
    HWND hwnd = CreateWindowExW(0, PROBE_CLASS,
        L"FedoraWin Frame Probe - ORIGINAL", WS_OVERLAPPEDWINDOW,
        CW_USEDEFAULT, CW_USEDEFAULT, 850, 570,
        NULL, NULL, instance, NULL);
    if (!hwnd) {
        fprintf(stderr, "CreateWindowExW failed=%lu\n", (unsigned long)GetLastError());
        fflush(stderr);
        return 3;
    }
    fprintf(stderr, "Created actual HWND=%p, show=%d\n", (void *)hwnd, show);
    fflush(stderr);
    ShowWindow(hwnd, SW_SHOWNORMAL);
    UpdateWindow(hwnd);
    MSG msg;
    while (GetMessageW(&msg, NULL, 0, 0) > 0) {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    return (int)msg.wParam;
}
