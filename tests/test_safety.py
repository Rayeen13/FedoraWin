from pathlib import Path
import json
import re
import sys
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]

FORBIDDEN = {
    r"HKLM:\\": "machine-wide registry access",
    r"\\System32\\": "System32 access",
    r"takeown(?:\.exe)?": "ownership takeover",
    r"icacls(?:\.exe)?": "ACL modification",
    r"bcdedit(?:\.exe)?": "boot configuration modification",
    r"reg(?:\.exe)?\s+delete": "registry deletion",
    r"sc(?:\.exe)?\s+(?:create|delete|config)": "service modification",
    r"uxtheme\.dll|uxinit\.dll": "theme DLL patching",
    r"CurrentVersion\\Winlogon\\Shell": "Explorer shell replacement",
    r"#Requires\s+-RunAsAdministrator": "administrator requirement",
    r"-Verb\s+RunAs": "administrator elevation",
    r"New-Service|Set-Service": "service modification",
    r"schtasks(?:\.exe)?": "scheduled task modification",
    r"diskpart(?:\.exe)?|Format-Volume|Remove-Partition": "disk/volume modification",
    r"Remove-Item[^\r\n]*-Recurse": "recursive deletion",
    r"SetWindowsHookEx": "global injected hook",
    r"SetWindowLong(?:Ptr)?\s*\(": "foreign window-style mutation",
}

files = [p for p in ROOT.rglob('*') if p.is_file() and p.suffix.lower() in {'.ps1', '.cmd', '.json', '.xaml', '.cs', '.yml', '.yaml'}]
errors = []
for path in files:
    text = path.read_text(encoding='utf-8', errors='ignore')
    for pattern, reason in FORBIDDEN.items():
        if path.name == 'test_safety.py':
            continue
        if re.search(pattern, text, flags=re.I):
            errors.append(f"{path.relative_to(ROOT)}: forbidden pattern ({reason}): {pattern}")

for xaml in ROOT.rglob('*.xaml'):
    try:
        ET.parse(xaml)
    except Exception as exc:
        errors.append(f"{xaml.relative_to(ROOT)}: invalid XAML/XML: {exc}")

try:
    cfg = json.loads((ROOT / 'config.json').read_text(encoding='utf-8'))
    for key in ['hideTaskbarWhileRunning','applyWallpaperWhileRunning','hideDesktopIconsWhileRunning','reservePanelInSafePreview','windowFrameEnabled']:
        assert isinstance(cfg[key], bool)
    assert 24 <= int(cfg['panelHeight']) <= 60
    assert 10 <= int(cfg['maxLauncherApps']) <= 250
    assert cfg['theme'] in {'light','dark','system'}
    assert cfg['accent'] in {'blue','teal','green','yellow','orange','red','pink','purple','slate'}
    assert cfg['dockMode'] in {'overview','desktop','both'}
    assert cfg['dockPosition'] in {'bottom','left','right'}
    assert isinstance(cfg['dockTopmost'], bool)
    assert 32 <= int(cfg['dockIconSize']) <= 64
    assert isinstance(cfg['dockFavorites'], list) and len(cfg['dockFavorites']) >= 1
except Exception as exc:
    errors.append(f"config.json validation failed: {exc}")

for required in [
    'Restore-Windows.cmd','Restore-Windows.ps1','Safe-Preview.cmd','Start-FedoraWin.cmd',
    'native/FedoraWinNativeUi.cs','ui/Activities.xaml','ui/QuickSettings.xaml',
    'ui/Appearance.xaml','ui/PowerMenu.xaml','ui/Dock.xaml','native/FedoraWinLauncher.cs','tests/build-exe.ps1'
]:
    if not (ROOT / required).exists():
        errors.append(f"missing required file: {required}")

main=(ROOT/'FedoraWin.ps1').read_text(encoding='utf-8')
startup=main[main.rfind('$taskbarHandle = Get-TaskbarHandle'):]
if startup.find('Save-State') == -1 or startup.find('Hide-WindowsTaskbar') == -1 or startup.find('Save-State') > startup.find('Hide-WindowsTaskbar'):
    errors.append('state is not saved before taskbar hiding')

native=(ROOT/'native'/'FedoraWinNativeUi.cs').read_text(encoding='utf-8')
for token in ['DwmGetWindowAttribute','DwmSetWindowAttribute','FedoraWinDwmFrameManager','RestoreAttribute','Dispose()']:
    if token not in native:
        errors.append(f'native frame theming missing reversible DWM token: {token}')
for token in ['System.Windows.Forms.Form','GnomeChromeForm','CreateWindowEx']:
    if token in native:
        errors.append(f'native bridge contains fake/extra window chrome token: {token}')

if errors:
    print('SAFETY TESTS FAILED')
    for e in errors:
        print(' -',e)
    sys.exit(1)
print(f'SAFETY TESTS PASSED ({len(files)} text/config/UI/native/CI files checked)')
