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
#define HEADER_HEIGHT_DIP 46

static WNDPROC previous_proc = NULL;
static LONG_PTR original_style = 0;
static BOOL frame_enabled = FALSE;
static int hovered_control = HTNOWHERE;

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

static void repaint_header(HWND hwnd) {
    RECT client;
    if (!GetClientRect(hwnd, &client)) return;
    RECT header = {0, 0, client.right, pixels(hwnd, HEADER_HEIGHT_DIP)};
    RedrawWindow(hwnd, &header, NULL, RDW_INVALIDATE | RDW_UPDATENOW);
}

static int control_for_index(int index) {
    if (index == 0) return HTCLOSE;
    if (index == 1) return HTMAXBUTTON;
    return HTMINBUTTON;
}

static void set_hovered_control(HWND hwnd, int hit) {
    int next = (hit == HTCLOSE || hit == HTMAXBUTTON || hit == HTMINBUTTON)
        ? hit : HTNOWHERE;
    if (hovered_control == next) return;
    hovered_control = next;
    repaint_header(hwnd);
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
    hovered_control = HTNOWHERE;
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
    hovered_control = HTNOWHERE;
    SetWindowTextW(hwnd, L"FedoraWin Frame Probe - ORIGINAL");
    recalculate(hwnd);
    return TRUE;
}

static void paint(HWND hwnd) {
    PAINTSTRUCT ps;
    HDC dc = BeginPaint(hwnd, &ps);
    RECT rect;
    GetClientRect(hwnd, &rect);
    HBRUSH base = CreateSolidBrush(RGB(34, 34, 38));
    FillRect(dc, &rect, base);
    DeleteObject(base);
    int header = pixels(hwnd, HEADER_HEIGHT_DIP);
    RECT bar = {0, 0, rect.right, header};
    HBRUSH surface = CreateSolidBrush(RGB(34, 34, 38));
    FillRect(dc, &bar, surface);
    DeleteObject(surface);
    SetBkMode(dc, TRANSPARENT);
    SetTextColor(dc, RGB(249, 249, 250));

    WCHAR title_text[256] = L"FedoraWin";
    GetWindowTextW(hwnd, title_text,
                   (int)(sizeof(title_text) / sizeof(title_text[0])));
    HFONT font = CreateFontW(-pixels(hwnd, 13), 0, 0, 0, FW_SEMIBOLD,
        FALSE, FALSE, FALSE, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS,
        CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, DEFAULT_PITCH, L"Segoe UI");
    HFONT old_font = (HFONT)SelectObject(dc, font);
    RECT title = {pixels(hwnd, 132), 0, rect.right - pixels(hwnd, 132), header};
    DrawTextW(dc, title_text, -1, &title,
              DT_SINGLELINE | DT_VCENTER | DT_CENTER | DT_END_ELLIPSIS);
    SelectObject(dc, old_font);
    DeleteObject(font);

    HBRUSH old_brush = (HBRUSH)SelectObject(dc, GetStockObject(NULL_BRUSH));
    HPEN old_pen = (HPEN)SelectObject(dc, GetStockObject(NULL_PEN));
    int span = pixels(hwnd, 38);
    int cy = header / 2;
    for (int i = 0; i < 3; ++i) {
        int cx = rect.right - pixels(hwnd, 12) - span / 2 - span * i;
        int radius = pixels(hwnd, 12);
        COLORREF fill = hovered_control == control_for_index(i)
            ? RGB(76, 76, 81) : RGB(56, 56, 59);
        HBRUSH circle = CreateSolidBrush(fill);
        HBRUSH prior = (HBRUSH)SelectObject(dc, circle);
        Ellipse(dc, cx - radius, cy - radius, cx + radius, cy + radius);
        SelectObject(dc, prior);
        DeleteObject(circle);
    }
    SelectObject(dc, old_pen);

    HPEN icon_pen = CreatePen(PS_SOLID, pixels(hwnd, 1), RGB(255, 255, 255));
    old_pen = (HPEN)SelectObject(dc, icon_pen);
    for (int i = 0; i < 3; ++i) {
        int cx = rect.right - pixels(hwnd, 12) - span / 2 - span * i;
        int mark = pixels(hwnd, 4);
        if (i == 0) {
            MoveToEx(dc, cx-mark, cy-mark, NULL); LineTo(dc, cx+mark, cy+mark);
            MoveToEx(dc, cx+mark, cy-mark, NULL); LineTo(dc, cx-mark, cy+mark);
        } else if (i == 1) {
            MoveToEx(dc, cx-mark, cy-mark, NULL); LineTo(dc, cx+mark, cy-mark);
            LineTo(dc, cx+mark, cy+mark); LineTo(dc, cx-mark, cy+mark);
            LineTo(dc, cx-mark, cy-mark);
        } else {
            MoveToEx(dc, cx-mark, cy, NULL); LineTo(dc, cx+mark+1, cy);
        }
    }
    SelectObject(dc, old_pen);
    SelectObject(dc, old_brush);
    DeleteObject(icon_pen);

    SetTextColor(dc, RGB(220, 220, 226));
    HFONT body_font = CreateFontW(-pixels(hwnd, 14), 0, 0, 0, FW_NORMAL,
        FALSE, FALSE, FALSE, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS,
        CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, DEFAULT_PITCH, L"Segoe UI");
    old_font = (HFONT)SelectObject(dc, body_font);
    RECT info = {pixels(hwnd, 30), header + pixels(hwnd, 30),
                 rect.right - pixels(hwnd, 30), rect.bottom};
    DrawTextW(dc,
      L"F8: restore the original Windows frame without reboot or app restart.\r\n"
      L"F8 again: attach a real native Win32 headerbar on the same GUI thread.\r\n\r\n"
      L"No foreign window or operating system frame is modified.",
      -1, &info, DT_LEFT | DT_TOP | DT_WORDBREAK);
    SelectObject(dc, old_font);
    DeleteObject(body_font);
    EndPaint(hwnd, &ps);
}

static void show_caption_system_menu(HWND hwnd, LPARAM lp) {
    HMENU menu = GetSystemMenu(hwnd, FALSE);
    if (!menu) return;

    int x = GET_X_LPARAM(lp);
    int y = GET_Y_LPARAM(lp);
    UINT command = TrackPopupMenu(
        menu,
        TPM_RETURNCMD | TPM_RIGHTBUTTON,
        x,
        y,
        0,
        hwnd,
        NULL);

    if (command != 0) {
        PostMessageW(hwnd, WM_SYSCOMMAND, (WPARAM)command, 0);
    }
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
    if (y < pixels(hwnd, HEADER_HEIGHT_DIP) + edge) {
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
            /* Paint through the top resize strip so the attached frame has no
               visible Windows caption sliver. frame_hit() still returns HTTOP
               and corner resize codes for the first 8 DIPs. */
            p->rgrc[0].top = top;
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
    case WM_NCMOUSEMOVE: {
        set_hovered_control(hwnd, (int)wp);
        if (hovered_control != HTNOWHERE) {
            TRACKMOUSEEVENT tracking = {0};
            tracking.cbSize = sizeof(tracking);
            tracking.dwFlags = TME_LEAVE | TME_NONCLIENT;
            tracking.hwndTrack = hwnd;
            TrackMouseEvent(&tracking);
        }
        break;
    }
    case WM_NCMOUSELEAVE:
        set_hovered_control(hwnd, HTNOWHERE);
        break;
    case WM_ACTIVATE:
        if (LOWORD(wp) == WA_INACTIVE) set_hovered_control(hwnd, HTNOWHERE);
        break;
    case WM_NCRBUTTONUP:
        if (wp == HTCAPTION) {
            show_caption_system_menu(hwnd, lp);
            return 0;
        }
        break;
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
        hovered_control = HTNOWHERE;
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
