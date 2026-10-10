# Research log

## 2026-09-10

- GNOME's current help describes Activities as the windows overview with a bottom dash, search-as-you-type, a nine-dot applications grid, and horizontal workspaces.
- GNOME Quick Settings directly toggles Wi-Fi/Bluetooth and exposes power mode, Night Light, Dark Style and Airplane Mode when available.
- Microsoft documents `DWMWA_CAPTION_COLOR`, `DWMWA_TEXT_COLOR`, border color and corner preference as Windows 11 DWM attributes.
- Microsoft documents `WlanSetInterface` with `wlan_intf_opcode_radio_state` for changing a Wi-Fi interface's software radio state from desktop apps.
- Seelen UI's public architecture confirms the viability of a Rust/Tauri backend with web UI surfaces for a Windows desktop environment. FedoraWin uses this as architectural validation only; implementation is independent.

## 2026-09-16

- GNOME 45 replaced the top-bar **Activities** text label with a dynamic workspace indicator. FedoraWin's native panel now follows that structure with an active-workspace pill and adjacent workspace dot instead of a text button.
- Windows Core Audio exposes the default render endpoint through `IMMDeviceEnumerator` and `IAudioEndpointVolume`. FedoraWin uses scalar 0–1 endpoint volume as the backing state for the GNOME volume slider rather than a shell command or Settings deep-link.
- `GetSystemPowerStatus` provides battery percentage, charging flags and AC state without a service or polling daemon. FedoraWin uses it for Quick Settings and the native top-panel battery glyph.
- Windows documents **Win+Shift+S** as the Snipping Tool overlay shortcut. FedoraWin's Screenshot action sends that chord through `SendInput` after dismissing Quick Settings, preserving the native Windows capture experience.
- `Windows.Devices.Radios.Radio` can enumerate Bluetooth radios and request On/Off state changes, but radio access is permission-, hardware- and policy-gated. The UI must re-read effective state after a request rather than optimistically assuming success.
- Internal-panel brightness can be exposed through `WmiMonitorBrightness` / `WmiSetBrightness`; external displays may require DDC/CI monitor APIs, whose MCCS support is hardware-dependent. FedoraWin will capability-detect brightness instead of showing a fake universal control.
- Windows power-mode overlay APIs are not treated as a stable public contract. FedoraWin will not wire the GNOME Power Mode tile to undocumented behavior.
