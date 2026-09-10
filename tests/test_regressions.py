from pathlib import Path
import re
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
MAIN = (ROOT/'FedoraWin.ps1').read_text(encoding='utf-8')
NATIVE = (ROOT/'native'/'FedoraWinNativeUi.cs').read_text(encoding='utf-8')
ACTIVITIES = (ROOT/'ui'/'Activities.xaml').read_text(encoding='utf-8')
QUICK = (ROOT/'ui'/'QuickSettings.xaml').read_text(encoding='utf-8')
APPEARANCE = (ROOT/'ui'/'Appearance.xaml').read_text(encoding='utf-8')


def test_xaml_is_ascii_safe_and_valid_xml():
    for path in (ROOT/'ui').glob('*.xaml'):
        text=path.read_text(encoding='utf-8')
        assert all(ord(ch) < 128 for ch in text), f'{path.name} contains non-ASCII shell glyphs'
        ET.parse(path)


def test_dispatcher_guard_installed_after_wpf_load():
    load=MAIN.index('Add-Type -AssemblyName PresentationFramework')
    guard=MAIN.index('[System.Windows.Threading.Dispatcher]::CurrentDispatcher.Add_UnhandledException')
    assert load < guard


def test_activities_is_explicit_toggle_without_deactivation_race():
    block=MAIN[MAIN.index('function Toggle-Activities'):MAIN.index('function Hide-TopPopovers')]
    assert 'IsVisible' in block and 'Hide-Activities' in block and 'Show-Activities' in block
    show=MAIN[MAIN.index('function Show-Activities'):MAIN.index('function Toggle-Activities')]
    assert 'ActivitiesWindow.Add_Deactivated' not in show
    assert "Alt+F1 Activities handler failed" in MAIN


def test_launcher_search_supports_terminal_alias_and_packaged_apps():
    assert "'terminal'" in MAIN.lower()
    assert 'Windows Terminal' in MAIN
    assert 'Get-StartApps' in MAIN
    assert 'shell:AppsFolder' in MAIN
    assert 'SearchText' in MAIN
    assert "([string]$_.SearchText).IndexOf" in MAIN


def test_quick_settings_is_compact_and_direct_for_supported_controls():
    assert 'Height="470"' in QUICK
    assert 'Windows.Devices.Radios.Radio' in MAIN
    assert 'Toggle-RadioState' in MAIN
    assert 'FedoraWinPowerMode' in MAIN
    assert 'Set-DisplayBrightness' in MAIN
    assert 'FedoraWinAudio' in MAIN
    assert "ms-settings:network" not in MAIN.lower()
    assert "ms-settings:bluetooth" not in MAIN.lower()
    assert 'Windows API unavailable' in QUICK


def test_accent_propagates_to_interactive_shell_controls():
    assert '{DynamicResource Accent}' in QUICK
    assert 'Set-QuickTileActive' in MAIN
    for color in ['#3584E4','#2190A4','#3A944A','#C88800','#ED5B00','#E62D42','#D56199','#9141AC','#6F8396']:
        assert color in MAIN
    for token in ['ThemeLightButton','ThemeDarkButton','ThemeSystemButton','AccentBlueButton','AccentSlateButton']:
        assert token in APPEARANCE


def test_real_dwm_frame_replaces_fake_overlay():
    assert 'FedoraWinDwmFrameManager' in NATIVE
    assert 'DwmSetWindowAttribute' in NATIVE
    assert 'DwmGetWindowAttribute' in NATIVE
    assert 'DWMWA_CAPTION_COLOR' in NATIVE
    assert 'DWMWA_TEXT_COLOR' in NATIVE
    assert 'DWMWA_WINDOW_CORNER_PREFERENCE' in NATIVE
    assert 'GnomeChromeForm' not in NATIVE
    assert 'FedoraWinChromeManager' not in NATIVE
    assert not re.search(r'SetWindowLong(?:Ptr)?\s*\(', NATIVE)


def test_appbar_and_power_menu_are_reversible_native_integrations():
    assert '[FedoraWinAppBar]::RegisterTop' in MAIN
    assert '[FedoraWinAppBar]::Remove' in MAIN
    assert '$script:FrameManager.Dispose()' in MAIN
    assert 'Toggle-PowerPopover' in MAIN
    assert '[FedoraWinSession]::Lock()' in MAIN
    assert '[FedoraWinSession]::Suspend()' in MAIN
    power=(ROOT/'ui'/'PowerMenu.xaml').read_text(encoding='utf-8')
    for name in ['LockButton','SuspendButton','RestartButton','PowerOffButton','LogOutButton']:
        assert name in power
