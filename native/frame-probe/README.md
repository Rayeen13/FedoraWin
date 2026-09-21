# Native Win32 frame laboratory (disposable process)

Builds a genuine same-process window-procedure replacement. This is NOT a
foreign-app injection engine or libadwaita: the real GTK4/libadwaita window
is under native/adwaita/. Here only native Win32 GDI reproduces a headerbar
within a disposable standard window. F8 attaches; F8 restores the original
procedure and Windows title bar, without changing its WS_CAPTION style.

MSYS2 UCRT64, from repository root:

    pacman -S --needed mingw-w64-ucrt-x86_64-gcc
    mkdir -p native/frame-probe/out
    gcc -std=c11 -Wall -Wextra -Werror -municode -mwindows \
      native/frame-probe/frame-probe.c \
      -o native/frame-probe/out/fedorawin-frame-probe.exe \
      -ldwmapi -lgdi32 -luser32
    ./native/frame-probe/out/fedorawin-frame-probe.exe

CI builds and starts the *actual executable*, sends F8 on the GUI thread,
checks same-PID attach/detach/reattach and restored original WNDPROC and
window styles, verifies headerbar drag, maximize and resize hit tests and
actual dark pixels, and compares the original and restored screenshots
pixel-for-pixel. The first passing run is 35592649463; the expanded gate
must pass separately before its additional checks count.

This does not prove foreign-app injection, 32-bit compatibility, Windows 11
Snap Layout hover, accessibility, data safety, DPI/multimonitor robustness,
or suitability for release. Do not ship this proof as an app-wide patcher.

See ../../docs/FRAME_ENGINE_RFC.md for opt-in and rollback requirements.
