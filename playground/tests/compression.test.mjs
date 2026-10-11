// Node tests of the Catalog dropdown and Compression manager models (js/compression.js): the
// dropdown's items and counts, the Store menu's catalog line, the rules table, the form's draft
// and spec, and the label suggestions.
// Run: node --test playground/tests/*.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  CATALOG_KINDS, DEFAULT_MIN_BYTES, catalogItems, catalogTotal, catalogCountsText, ratioText,
  ruleRow, totals, newDraft, draftFromRule, draftToSpec, labelSuggestions,
} from '../js/compression.js';

const RULE = {
  name: 'customer_cv',
  element: 'vertex',
  label: 'customer',
  path: '$.details.career.cv',
  codec: 'lz4',
  minBytes: 512,
  dictionary: true,
  stats: { compressedValues: 3, plainBytes: 3000, storedBytes: 1200, dictionaryBytes: 2048 },
};

test('catalog items: one per kind with its count, from the graph info', () => {
  assert.deepEqual(CATALOG_KINDS.map((k) => k.id), ['queries', 'compression']);
  const items = catalogItems({ catalog: { query: 2, compression: 1, 'kind 77': 4 } });
  assert.deepEqual(items.map((i) => [i.id, i.count]), [['queries', 2], ['compression', 1]]);
  assert.equal(catalogTotal({ catalog: { query: 2, compression: 1, 'kind 77': 4 } }), 7);
  // An older info without `catalog`.
  assert.deepEqual(catalogItems({ savedQueries: 5, compressionRules: 2 }).map((i) => i.count), [5, 2]);
  assert.deepEqual(catalogItems(null).map((i) => i.count), [0, 0]);
});

test('the Store menu line names every kind', () => {
  assert.equal(catalogCountsText({ query: 2, compression: 1 }, 3), '2 saved queries, 1 compression rule');
  assert.equal(catalogCountsText({ query: 1, 'kind 77': 2 }, 3), '1 saved query, 2 kind 77 definitions');
  assert.equal(catalogCountsText({}, 0), 'empty');
  assert.equal(catalogCountsText(undefined, 4), '4 definitions');
});

test('rules table: sizes, ratio, saving and totals', () => {
  const row = ruleRow(RULE);
  assert.equal(row.target, 'vertex customer');
  assert.equal(row.path, '$.details.career.cv');
  assert.equal(row.threshold, '≥ 512 B');
  assert.equal(row.dictionary, '2 KiB');
  assert.equal(row.values, 3);
  assert.equal(row.sizes, '2.9 KiB → 1.2 KiB');
  assert.equal(row.ratio, '40 %');
  assert.equal(row.saved, '1.8 KiB');
  const empty = ruleRow({ ...RULE, element: 'edge', dictionary: true, stats: { compressedValues: 0, plainBytes: 0, storedBytes: 0, dictionaryBytes: 0 } });
  assert.equal(empty.target, 'edge customer');
  assert.equal(empty.dictionary, 'yes (none yet)');
  assert.equal(empty.sizes, '—');
  assert.equal(empty.ratio, '—');
  assert.equal(ruleRow({ ...RULE, dictionary: false }).dictionary, 'no');
  assert.equal(ratioText(null), '—');
  assert.deepEqual(totals([RULE, RULE]), { compressedValues: 6, plainBytes: 6000, storedBytes: 2400, dictionaryBytes: 4096 });
});

test('form: a new draft, an edit draft and the spec', () => {
  const draft = newDraft();
  assert.equal(draft.minBytesText, String(DEFAULT_MIN_BYTES));
  assert.equal(draft.dictionary, true);
  const edit = draftFromRule(RULE);
  assert.equal(edit.path, 'details.career.cv');
  assert.equal(edit.replaces, 'customer_cv');
  assert.equal(draftFromRule({ ...RULE, path: "$['a b'].c" }).path, "$['a b'].c");
  const { spec } = draftToSpec({ ...edit, name: ' renamed ', minBytesText: '1024', dictionary: false });
  assert.deepEqual(spec, {
    name: 'renamed', element: 'vertex', label: 'customer', path: 'details.career.cv', minBytes: 1024,
    dictionary: false, replaces: 'customer_cv',
  });
  assert.equal(draftToSpec({ ...draft, name: 'a', label: 'l', path: 'p', minBytesText: '' }).spec.minBytes, DEFAULT_MIN_BYTES);
});

test('form: local field errors before the engine is asked', () => {
  const { errors } = draftToSpec({ ...newDraft(), minBytesText: '0' });
  assert.deepEqual(errors.map((e) => e.field), ['name', 'label', 'path', 'minBytes']);
  for (const bad of ['-5', '1.5', 'many', String(16 * 1024 * 1024 + 1)]) {
    const res = draftToSpec({ ...newDraft(), name: 'a', label: 'l', path: 'p', minBytesText: bad });
    assert.deepEqual(res.errors.map((e) => e.field), ['minBytes'], bad);
  }
});

test('label suggestions come from the statistics of the chosen element kind', () => {
  const stats = {
    statistics: {
      vertex_labels: [{ label: 'person', count: 4 }, { label: 'customer', count: 2 }, { label: 'person', count: 1 }],
      edge_labels: [{ label: 'knows', count: 2 }],
    },
  };
  assert.deepEqual(labelSuggestions(stats, 'vertex'), ['customer', 'person']);
  assert.deepEqual(labelSuggestions(stats, 'edge'), ['knows']);
  assert.deepEqual(labelSuggestions(null, 'vertex'), []);
});
