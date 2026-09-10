import assert from 'node:assert/strict';
import fs from 'node:fs';

const js = fs.readFileSync(new URL('../ui/app.js', import.meta.url), 'utf8');
const css = fs.readFileSync(new URL('../ui/styles.css', import.meta.url), 'utf8');
const frame = fs.readFileSync(new URL('../src-tauri/src/windows/frame.rs', import.meta.url), 'utf8');
const wifi = fs.readFileSync(new URL('../src-tauri/src/windows/wifi.rs', import.meta.url), 'utf8');

assert.match(js, /toggle_activities/);
assert.match(js, /set_wifi_enabled/);
assert.match(js, /terminal|Terminal/);
assert.match(css, /--accent/);
assert.match(css, /grid-template-columns: 1fr 1fr/);
assert.match(frame, /DwmSetWindowAttribute/);
assert.doesNotMatch(frame, /GnomeChromeForm|FormBorderStyle|ShowWithoutActivation/);
assert.match(frame, /DWMWA_CAPTION_COLOR/);
assert.match(frame, /DWMWA_TEXT_COLOR/);
assert.match(wifi, /WlanSetInterface/);
assert.match(wifi, /WLAN_INTF_OPCODE_RADIO_STATE/);
console.log('UI/native contract checks passed');
