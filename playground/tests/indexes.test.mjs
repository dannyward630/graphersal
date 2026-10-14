// Node tests of the Property indexes manager model (js/indexes.js): the Catalog dropdown's
// items with the index kind, the default name, the indexes table, the form's draft and spec,
// and the Statistics tab's index line.
// Run: node --test playground/tests/*.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  INDEX_KIND, catalogMenuItems, defaultIndexName, indexRow, indexTotals, newIndexDraft,
  indexDraftToSpec,
} from '../js/indexes.js';
import { catalogCountsText } from '../js/compression.js';
import { statisticsModel } from '../js/statistics.js';

const INDEX = {
  name: 'person_age',
  element: 'vertex',
  label: 'person',
  key: 'age',
  served: true,
  stats: { entries: 4, distinctValues: 2, bytes: 2048 },
};

test('the Catalog dropdown lists property indexes with their count', () => {
  assert.equal(INDEX_KIND.kind, 'property_index');
  const items = catalogMenuItems({ catalog: { query: 2, compression: 1, property_index: 3 } });
  assert.deepEqual(items.map((i) => [i.id, i.count]), [['queries', 2], ['compression', 1], ['indexes', 3]]);
  assert.equal(items[2].title, 'Property indexes');
  // The session info's own count, and no info at all.
  assert.equal(catalogMenuItems({ propertyIndexes: 2 })[2].count, 2);
  assert.equal(catalogMenuItems(null)[2].count, 0);
});

test('the Store menu line names property indexes', () => {
  assert.equal(
    catalogCountsText({ property_index: 2, query: 1 }, 3),
    '1 saved query, 2 property indexes',
  );
  assert.equal(catalogCountsText({ property_index: 1 }, 1), '1 property index');
});

test('the default name follows the engine', () => {
  assert.equal(defaultIndexName('vertex', 'person', 'age'), 'person_age');
  assert.equal(defaultIndexName('edge', 'knows', 'weight'), 'knows_weight_edge');
  assert.equal(defaultIndexName('vertex', '2nd-tier', 'e-mail'), 'index_2nd_tier_e_mail');
  assert.equal(defaultIndexName('vertex', 'Café', 'x'), 'Caf__x');
});

test('index rows: target, stats as text, not served', () => {
  const row = indexRow(INDEX);
  assert.deepEqual(row, {
    name: 'person_age',
    target: 'vertex person',
    key: 'age',
    served: true,
    entries: 4,
    distinct: 2,
    perValue: '2',
    bytes: '2 KiB',
  });
  assert.equal(indexRow({ ...INDEX, stats: { entries: 5, distinctValues: 2, bytes: 0 } }).perValue, '2.5');
  assert.equal(indexRow({ ...INDEX, stats: { entries: 0, distinctValues: 0, bytes: 0 } }).perValue, '—');
  const plain = indexRow({ ...INDEX, element: 'edge', label: 'knows', served: false, stats: null });
  assert.equal(plain.target, 'edge knows');
  assert.equal(plain.served, false);
  assert.equal(plain.entries, '—');
  assert.equal(plain.bytes, '—');
  assert.deepEqual(
    indexTotals([INDEX, { ...INDEX, name: 'b', stats: { entries: 6, distinctValues: 6, bytes: 1024 } }, { ...INDEX, served: false, stats: null }]),
    { entries: 10, bytes: 3072, notServed: 1 },
  );
  assert.deepEqual(indexTotals(null), { entries: 0, bytes: 0, notServed: 0 });
});

test('the form: draft, spec and local field errors', () => {
  const draft = newIndexDraft();
  assert.deepEqual(draft, { name: '', element: 'vertex', label: '', key: '' });
  const empty = indexDraftToSpec(draft);
  assert.deepEqual(empty.errors.map((e) => e.field), ['label', 'key']);
  assert.deepEqual(indexDraftToSpec({ ...draft, label: ' person ', key: 'age ' }).spec, {
    element: 'vertex', label: 'person', key: 'age',
  });
  assert.deepEqual(indexDraftToSpec({ name: ' w ', element: 'edge', label: 'knows', key: 'weight' }).spec, {
    element: 'edge', label: 'knows', key: 'weight', name: 'w',
  });
  const jpath = indexDraftToSpec({ ...draft, label: 'person', key: '$.address.city' });
  assert.deepEqual(jpath.errors.map((e) => e.field), ['key']);
});

test('the Statistics tab shows index memory only when there is some', () => {
  const memory = {
    structure_bytes: 1024, index_bytes: 512, property_bytes: 2048, total_bytes: 5632,
    property_index_bytes: 2048,
  };
  const names = (m) => statisticsModel({ memory: m }).memory.map((r) => r.name);
  assert.deepEqual(names(memory), ['Structure', 'Indexes', 'Properties', 'Property indexes', 'Total']);
  assert.equal(statisticsModel({ memory }).memory[3].text, '2 KiB');
  assert.deepEqual(names({ ...memory, property_index_bytes: 0 }), ['Structure', 'Indexes', 'Properties', 'Total']);
});
