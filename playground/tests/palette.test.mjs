// Node tests of the label palette (js/graph/palette.js): collision-free slots per view, stable
// across redraws and views, the "other" colour past the palette, both themes.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  LabelPalette, PALETTE_LIGHT, PALETTE_DARK, PALETTE_SIZE, OTHER, OTHER_SLOT, slotColor,
  vertexLabelsInTableOrder,
} from '../js/graph/palette.js';

test('the palette has distinct colours per theme and the same number of slots', () => {
  assert.equal(PALETTE_LIGHT.length, PALETTE_SIZE);
  assert.equal(PALETTE_DARK.length, PALETTE_SIZE);
  assert.equal(new Set(PALETTE_LIGHT).size, PALETTE_SIZE, 'no duplicate in light mode');
  assert.equal(new Set(PALETTE_DARK).size, PALETTE_SIZE, 'no duplicate in dark mode');
  for (const c of [...PALETTE_LIGHT, ...PALETTE_DARK, OTHER.light, OTHER.dark]) assert.match(c, /^#[0-9a-f]{6}$/);
  assert.ok(!PALETTE_LIGHT.includes(OTHER.light) && !PALETTE_DARK.includes(OTHER.dark));
  assert.equal(slotColor(0), PALETTE_LIGHT[0]);
  assert.equal(slotColor(0, true), PALETTE_DARK[0]);
  assert.equal(slotColor(OTHER_SLOT), OTHER.light);
  assert.equal(slotColor(-1, true), OTHER.dark);
});

test('"person" and "company" (one hashed slot before) get different colours', () => {
  const p = new LabelPalette().assign(['person', 'company']);
  assert.notEqual(p.color('person'), p.color('company'));
  assert.notEqual(p.color('person', true), p.color('company', true));
});

test('slots follow the label table order and never collide within the palette size', () => {
  const p = new LabelPalette();
  const labels = Array.from({ length: PALETTE_SIZE }, (_, i) => `l${i}`);
  p.assign(labels);
  assert.deepEqual(labels.map((l) => p.slot(l)), labels.map((_, i) => i));
  assert.equal(new Set(labels.map((l) => p.color(l))).size, PALETTE_SIZE);
  // Past the palette: the shared "other" slot.
  p.assign(['extra1', 'extra2']);
  assert.equal(p.slot('extra1'), OTHER_SLOT);
  assert.equal(p.color('extra2'), OTHER.light);
});

test('a label keeps its slot across redraws, other views and orders', () => {
  const p = new LabelPalette();
  p.assign(['person', 'software']);
  p.assign(['software', 'city', 'person']); // another view, another order
  assert.equal(p.slot('person'), 0);
  assert.equal(p.slot('software'), 1);
  assert.equal(p.slot('city'), 2);
  // An unseen label asked directly gets the next slot; the empty label is a label too.
  assert.equal(p.slot(''), 3);
  assert.equal(p.slot(null), 3, 'null is the empty label');
});

test('vertexLabelsInTableOrder: the vertex labels of a view in label table order, each once', () => {
  const view = {
    labels: ['person', 'software', 'knows', 'created', 'city'],
    vertexLabel: Uint32Array.from([1, 0, 0, 4, 1]),
  };
  assert.deepEqual(vertexLabelsInTableOrder(view), ['person', 'software', 'city']);
  assert.deepEqual(vertexLabelsInTableOrder(null), []);
});
