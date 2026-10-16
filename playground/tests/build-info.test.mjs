import { test } from 'node:test';
import assert from 'node:assert/strict';
import { versionLabel } from '../js/build-info.js';

test('a release build shows the version alone', () => {
  const label = versionLabel('0.2.0', { commit: 'abc123def456', describe: 'v0.2.0' });
  assert.equal(label.text, 'v0.2.0');
  assert.equal(label.title, 'Engine 0.2.0, built from commit abc123def456');
});

test('a build after the tag adds its describe text', () => {
  const label = versionLabel('0.2.0', { commit: 'abc123def456', describe: 'v0.1.1-12-gabc123d-dirty' });
  assert.equal(label.text, 'v0.2.0 · v0.1.1-12-gabc123d-dirty');
  assert.match(label.title, /with uncommitted changes$/);
});

test('without a build file only the engine version is shown', () => {
  assert.deepEqual(versionLabel('0.2.0', null), { text: 'v0.2.0', title: 'Engine 0.2.0' });
  assert.equal(versionLabel(null, null), null);
});
