from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
errors=[]

def strip_ps(text: str) -> str:
    text = re.sub(r"@'\n.*?\n'@", "", text, flags=re.S)
    text = re.sub(r'@"\n.*?\n"@', "", text, flags=re.S)
    lines = []
    for line in text.splitlines():
        line = re.sub(r"'(?:''|[^'\r\n])*'", "''", line)
        line = re.sub(r'"(?:`.|[^"`\r\n])*"', '""', line)
        line = re.sub(r'#.*$', '', line)
        lines.append(line)
    return "\n".join(lines)

for ps in ROOT.glob('*.ps1'):
    t=strip_ps(ps.read_text(encoding='utf-8'))
    for a,b,name in [('{','}','braces'),('(',')','parentheses'),('[',']','brackets')]:
        depth=0
        for ch in t:
            if ch==a: depth+=1
            elif ch==b:
                depth-=1
                if depth<0:
                    errors.append(f'{ps.name}: closing {name} before opening')
                    break
        if depth!=0:
            errors.append(f'{ps.name}: unbalanced {name}: depth={depth}')

main=(ROOT/'FedoraWin.ps1').read_text(encoding='utf-8')
startup=main[main.find('$taskbarHandle = Get-TaskbarHandle'):]
for token in ['Save-State','Hide-WindowsTaskbar','FedoraWinAppBar','FedoraWinDwmFrameManager','Apply-FedoraWinTheme','Toggle-AppearancePopover','Toggle-PowerPopover']:
    if token not in main:
        errors.append(f'missing runtime feature: {token}')

if startup.find('Save-State') < 0 or startup.find('Hide-WindowsTaskbar') < 0 or startup.find('Save-State') > startup.find('Hide-WindowsTaskbar'):
    errors.append('state backup does not precede taskbar modification')

restore=(ROOT/'Restore-Windows.ps1').read_text(encoding='utf-8')
for token in ['SW_SHOW','SetWorkArea','originalWallpaper','desktopIconsWereVisible','GetDesktopListView','FedoraWin.ps1']:
    if token not in restore:
        errors.append(f'Restore-Windows.ps1 missing recovery token: {token}')

names=set()
for x in (ROOT/'ui').glob('*.xaml'):
    names.update(re.findall(r'x:Name="([A-Za-z0-9_]+)"', x.read_text(encoding='utf-8')))
for ref in set(re.findall(r"FindName\('([^']+)'\)", main)):
    if ref not in names:
        errors.append(f'FindName references missing XAML element: {ref}')

native=(ROOT/'native'/'FedoraWinNativeUi.cs').read_text(encoding='utf-8')
for token in ['SHAppBarMessage','ABM_SETPOS','FedoraWinDwmFrameManager','SetWinEventHook','UnhookWinEvent','DwmSetWindowAttribute','DwmGetWindowAttribute','FedoraWinPowerMode','FedoraWinSession']:
    if token not in native:
        errors.append(f'native bridge missing: {token}')
if 'FedoraWinChromeManager' in native or 'GnomeChromeForm' in native:
    errors.append('legacy fake frame overlay still present')
if re.search(r'SetWindowLong(?:Ptr)?\s*\(', native):
    errors.append('native bridge mutates target window styles')

if errors:
    print('STRUCTURE TESTS FAILED')
    for e in errors:
        print(' -',e)
    sys.exit(1)
print('STRUCTURE TESTS PASSED')
