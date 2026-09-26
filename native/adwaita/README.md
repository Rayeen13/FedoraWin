# Real native Adwaita proof on Windows

This is an **optional FedoraWin-owned GTK4/libadwaita window**, not
DWM recoloring or an HTML/CSS imitation. GTK's Win32 backend owns the HWND;
the real AdwApplicationWindow, AdwToolbarView, AdwHeaderBar, AdwWindowTitle
and AdwStatusPage own the interface and controls.

In an MSYS2 UCRT64 shell, from the FedoraWin repo root:

```sh
pacman -S --needed mingw-w64-ucrt-x86_64-gcc mingw-w64-ucrt-x86_64-pkgconf mingw-w64-ucrt-x86_64-libadwaita
mkdir -p native/adwaita/out
gcc -std=c11 -Wall -Wextra -Werror native/adwaita/adwaita-window.c \
  -o native/adwaita/out/fedorawin-adwaita.exe \
  $(pkg-config --cflags --libs libadwaita-1)
FEDORAWIN_ADWAITA_THEME=dark ./native/adwaita/out/fedorawin-adwaita.exe
```

Use FEDORAWIN_ADWAITA_THEME=light or omit it for system mode.
Run with UCRT64 DLLs on PATH. This native window is an **isolated proof**,
not yet connected to the production shell or beta installer. It must pass
the separate Windows build-and-HWND test before its screenshots count.
Do not claim GTK4 + dependencies fit the 300 MB shell budget without
measuring the complete process tree.

The existing lightweight Rust/Win32 panel stays GTK-free at idle.
Foreign applications keep their original Win32 caption buttons and
hit testing; libadwaita cannot be applied to them by setting DWM attributes.
No injection, registry patches, Explorer replacement, or fake second caption.
