import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

// A Rust-only pull request must not silently skip either independent Windows
// proof. These checks are automated CI evidence, never manual beta approval.
for (const filename of ['native-adwaita.yml', 'native-frame-probe.yml']) {
  const workflow = readFileSync(new URL(`../.github/workflows/${filename}`, import.meta.url), 'utf8');
  const [events] = workflow.split('\npermissions:\n');

  test(`${filename} runs on every native development push and pull request`, () => {
    assert.match(events, /^on:$/m);
    assert.match(events, /^  push:$/m);
    assert.match(events, /^    branches: \[dev\/gnome-native-20260914\]$/m);
    assert.match(events, /^  pull_request:$/m);
    assert.doesNotMatch(events, /^\s+(?:paths|paths-ignore|branches-ignore):/m);
  });
}
