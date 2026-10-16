// The Transform choice of the Property indexes form (js/indexes.js): the known transforms, the
// default name's `_to_lower` suffix (the engine's `PropertyIndexDefinition::default_name`), the
// draft and the spec it sends, the transform of a listed entry.
import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  INDEX_TRANSFORMS, defaultIndexName, indexDraftToSpec, indexTransformOf, newTransformIndexDraft,
} from '../js/indexes.js';

test('the transforms offered: none first, then to_lower with a query hint', () => {
  assert.deepEqual(INDEX_TRANSFORMS.map((t) => t.name), ['', 'to_lower']);
  assert.equal(INDEX_TRANSFORMS[0].label, 'none');
  assert.match(INDEX_TRANSFORMS[1].description, /has\(key, __\.to_lower\(\)\.is\(\.\.\)\)/u);
});

test('the default name gets the transform as a suffix, after _edge', () => {
  assert.equal(defaultIndexName('vertex', 'person', 'name', 'to_lower'), 'person_name_to_lower');
  assert.equal(defaultIndexName('edge', 'knows', 'note', 'to_lower'), 'knows_note_edge_to_lower');
  assert.equal(defaultIndexName('vertex', 'person', 'name', ''), 'person_name');
  assert.equal(defaultIndexName('vertex', 'person', 'name', undefined), 'person_name');
  assert.equal(defaultIndexName('vertex', '2nd', 'e-mail', 'to_lower'), 'index_2nd_e_mail_to_lower');
});

test('the draft starts without a transform and sends one only when chosen', () => {
  const draft = newTransformIndexDraft();
  assert.deepEqual(draft, {
    name: '', element: 'vertex', label: '', key: '', type: 'memory-ordered', transform: '',
  });
  assert.deepEqual(indexDraftToSpec({ ...draft, label: 'person', key: 'name' }).spec, {
    element: 'vertex', label: 'person', key: 'name', type: 'memory-ordered',
  });
  assert.deepEqual(
    indexDraftToSpec({ ...draft, label: 'person', key: 'name', transform: 'to_lower' }).spec,
    { element: 'vertex', label: 'person', key: 'name', type: 'memory-ordered', transform: 'to_lower' },
  );
  // A typed name wins over the suggestion (the engine's default is used only without one).
  assert.equal(
    indexDraftToSpec({ ...draft, label: 'person', key: 'name', transform: 'to_lower', name: 'ci' }).spec.name,
    'ci',
  );
});

test('the transform of a listed entry', () => {
  assert.equal(indexTransformOf({ name: 'a', transform: 'to_lower' }), 'to_lower');
  assert.equal(indexTransformOf({ name: 'a', transform: null }), '');
  assert.equal(indexTransformOf({ name: 'a' }), '');
  assert.equal(indexTransformOf(null), '');
});
