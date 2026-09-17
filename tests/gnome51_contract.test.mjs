import assert from 'node:assert/strict';
import fs from 'node:fs';

const html = fs.readFileSync(new URL('../ui/index.html', import.meta.url), 'utf8');
const css = fs.readFileSync(new URL('../ui/gnome51.css', import.meta.url), 'utf8');
const powerModeUi = fs.readFileSync(new URL('../ui/power-mode.js', import.meta.url), 'utf8');
const main = fs.readFileSync(new URL('../src-tauri/src/main.rs', import.meta.url), 'utf8');
const power = fs.readFileSync(new URL('../src-tauri/src/windows/power.rs', import.meta.url), 'utf8');

assert.match(html, /styles\.css[\s\S]*adwaita\.css[\s\S]*gnome51\.css/);
assert.match(html, /app\.js[\s\S]*power-mode\.js/);
assert.match(css, /grid-template-columns:\s*repeat\(6,\s*minmax\(96px,\s*1fr\)\)/);
assert.match(css, /grid-template-rows:\s*repeat\(4,\s*minmax\(86px,\s*1fr\)\)/);
assert.match(css, /--gnome-icon-size:\s*64px/);
assert.match(css, /\.dash\s*\{[\s\S]*height:\s*74px/);
assert.match(css, /\.quick-card\s*\{[\s\S]*gap:\s*10px/);
assert.match(css, /prefers-reduced-motion:\s*reduce/);
assert.match(css, /\.app-tile:focus-visible/);
assert.match(css, /\.workspace-nav:focus-visible/);
assert.doesNotMatch(css, /position:\s*fixed/);
assert.doesNotMatch(css, /frame-overlay|fake-caption|explorer\.exe/i);

assert.match(power, /PowerGetUserConfiguredACPowerMode/);
assert.match(power, /PowerGetUserConfiguredDCPowerMode/);
assert.match(power, /PowerSetUserConfiguredACPowerMode/);
assert.match(power, /PowerSetUserConfiguredDCPowerMode/);
assert.match(power, /GUID_POWER_MODE_BEST_EFFICIENCY/);
assert.match(power, /GUID_POWER_MODE_BEST_PERFORMANCE/);
assert.match(power, /GUID_POWER_MODE_NONE/);
assert.match(power, /pub enum PowerMode/);
assert.match(main, /fn get_power_mode\(\)/);
assert.match(main, /fn set_power_mode\([\s\S]*?mode:\s*String,[\s\S]*?\) -> Result<windows::power::PowerMode, String>/);
assert.match(main, /get_power_mode,[\s\S]*set_power_mode,/);
assert.match(powerModeUi, /get_power_mode/);
assert.match(powerModeUi, /set_power_mode/);
assert.match(powerModeUi, /bestEfficiency/);
assert.match(powerModeUi, /bestPerformance/);
assert.match(powerModeUi, /MutationObserver/);
assert.doesNotMatch(powerModeUi, /powercfg|cmd\.exe|powershell|reg\.exe/i);

console.log('GNOME 51 fidelity contract checks passed');
