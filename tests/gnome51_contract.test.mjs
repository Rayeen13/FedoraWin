import assert from 'node:assert/strict';
import fs from 'node:fs';

const html = fs.readFileSync(new URL('../ui/index.html', import.meta.url), 'utf8');
const css = fs.readFileSync(new URL('../ui/gnome51.css', import.meta.url), 'utf8');

assert.match(html, /styles\.css[\s\S]*adwaita\.css[\s\S]*gnome51\.css/);
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

console.log('GNOME 51 fidelity contract checks passed');
