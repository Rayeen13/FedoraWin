import assert from 'node:assert/strict';
import fs from 'node:fs';

const main = fs.readFileSync(new URL('../src-tauri/src/main.rs', import.meta.url), 'utf8');
const layout = fs.readFileSync(new URL('../src-tauri/src/layout.rs', import.meta.url), 'utf8');
const osd = fs.readFileSync(new URL('../src-tauri/src/osd.rs', import.meta.url), 'utf8');
const html = fs.readFileSync(new URL('../ui/osd.html', import.meta.url), 'utf8');
const js = fs.readFileSync(new URL('../ui/osd.js', import.meta.url), 'utf8');
const css = fs.readFileSync(new URL('../ui/osd.css', import.meta.url), 'utf8');

assert.match(main, /manage\(osd::OsdState::default\(\)\)/);
assert.match(main, /set_master_volume[\s\S]*osd::show[\s\S]*"volume"/);
assert.match(main, /set_power_mode[\s\S]*osd::show[\s\S]*"power"/);
assert.match(main, /show_control_osd/);

assert.match(layout, /pub osd:\s*SurfaceGeometry/);
assert.match(layout, /osd_bottom_margin_px/);
assert.match(layout, /display\.bounds\.bottom - osd_height_px/);

assert.match(osd, /OSD_TIMEOUT_MS:\s*u64\s*=\s*1600/);
assert.match(osd, /WebviewUrl::App\(payload\.initial_url\(\)\.into\(\)\)/);
assert.match(osd, /\.transparent\(true\)/);
assert.match(osd, /get_webview_window\(OSD_LABEL\)/);
assert.match(osd, /window\.close\(\)/);
assert.match(osd, /"volume" \| "brightness" \| "power" \| "media"/);

assert.match(html, /osd\.css/);
assert.match(html, /osd\.js/);
assert.match(js, /fedorawin:\/\/osd/);
assert.match(js, /bestEfficiency[\s\S]*Power Saver/);
assert.match(js, /bestPerformance[\s\S]*Performance/);
assert.match(css, /\.control-osd/);
assert.match(css, /border-radius:\s*22px/);
assert.match(css, /prefers-reduced-motion:\s*reduce/);
assert.doesNotMatch(osd + js + css, /explorer\.exe|reg(?:\.exe)?\s+add|powercfg/i);

console.log('GNOME control OSD contract checks passed');
