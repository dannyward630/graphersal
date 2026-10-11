// Node tests of the completion tooltip's doc text split (js/editor/completion-doc.js).
// Run: node --test playground/tests/*.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { splitExample } from '../js/editor/completion-doc.js';

test('a doc text splits into its description and its example with the result', () => {
  const doc = 'Keeps elements whose label is one of the given labels.\n\n'
    + 'Example: `g.v().has_label("software").values("name").to_list()` returns `["lop", "ripple"]`';
  assert.deepEqual(splitExample(doc), [
    'Keeps elements whose label is one of the given labels.',
    'g.v().has_label("software").values("name").to_list()  → ["lop", "ripple"]',
  ]);
});

test('an example that fails or shows no result keeps its query', () => {
  assert.deepEqual(
    splitExample('Stops.\n\nExample: `g.v(1).fail("stop here").to_list()` fails with `stop here`'),
    ['Stops.', 'g.v(1).fail("stop here").to_list()  → error: stop here'],
  );
  assert.deepEqual(splitExample('Renders.\n\nExample: `g.v(1).to_table()`'), ['Renders.', 'g.v(1).to_table()']);
});

test('a text without an example stays whole', () => {
  assert.deepEqual(splitExample('Only a sentence.'), ['Only a sentence.', null]);
});
