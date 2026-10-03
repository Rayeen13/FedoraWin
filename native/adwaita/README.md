# Real native Adwaita proof on Windows

This directory contains **FedoraWin-owned GTK4/libadwaita windows**, not
DWM recoloring or HTML/CSS imitations. GTK's Win32 backend owns each HWND.
The baseline proof remains deliberately small, and the Preferences candidate
uses real AdwApplicationWindow, AdwToolbarView, AdwHeaderBar,
AdwPreferencesPage, AdwPreferencesGroup and AdwActionRow widgets.

In an MSYS2 UCRT64 shell, from the FedoraWin repo root:

```sh
pacman -S --needed mingw-w64-ucrt-x86_64-gcc mingw-w64-ucrt-x86_64-pkgconf mingw-w64-ucrt-x86_64-libadwaita
mkdir -p native/adwaita/out
gcc -std=c11 -Wall -Wextra -Werror native/adwaita/adwaita-window.c \
  -o native/adwaita/out/fedorawin-adwaita.exe \
  $(pkg-config --cflags --libs libadwaita-1)
gcc -std=c11 -Wall -Wextra -Werror native/adwaita/preferences-window.c \
  -o native/adwaita/out/fedorawin-preferences.exe \
  $(pkg-config --cflags --libs libadwaita-1)
FEDORAWIN_ADWAITA_THEME=dark ./native/adwaita/out/fedorawin-preferences.exe
```

Use FEDORAWIN_ADWAITA_THEME=light or omit it for system mode.
The production shell can launch this Preferences surface on demand. The beta
installer/runtime package is still gated on a self-contained Windows runtime:
`package-runtime.ps1` stages `preferences-runtime/` with the recursive
UCRT64 DLL closure and required GLib/icon data, while
`check-portable-runtime.ps1` launches that staged executable with MSYS2
removed from PATH and rejects any loaded non-Windows module outside the staged
runtime. This is CI staging evidence, not a distributable beta package.

Windows CI must still build and capture both dark/light HWNDs, record the
temporary process working set, and pass the portable-runtime isolation check
before packaging progress is treated as verified.

The temporary GTK surface has its own 300 MB CI ceiling. That measurement is
separate from the stricter idle-shell process-tree budget and does not include
future installer/dependency disk-size validation.

The existing lightweight Rust/Win32 panel stays GTK-free at idle.
Foreign applications keep their original Win32 caption buttons and
hit testing; libadwaita cannot be applied to them by setting DWM attributes.
No injection, registry patches, Explorer replacement, or fake second caption.
