// Node tests of vectors in the playground (book "Vectors"): the result table's vector cell
// (js/util.js; the engine puts `{"@type": "graphersal:Vector", element, dimension, preview}` into
// a table), the property editor's model (js/element-editor.js: the typed form
// `{type: 'vector', element, value: [numbers]}`, a JSON list for a declared vector) and the
// schema editor's model (js/schema/draft.js: `{"type": "array", "format": "vector", ..}`).
// Run: node --test playground/tests/*.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { formatValue, isVectorCell, vectorPreview, vectorSummary } from '../js/util.js';
import {
  TYPES, TEXT_TYPES, VECTOR_ELEMENTS, placeholderOf, nodeFromTyped, typedFromNode, scalarError,
  vectorValues, nodeToJson, nodeFromJsonText, typesOf, defaultType, setType, emptyNode,
} from '../js/element-editor.js';
import { BASE_TYPES, describe, typeText, setBaseType } from '../js/schema/draft.js';

const CELL = { '@type': 'graphersal:Vector', element: 'f32', dimension: 768, preview: [0.5, -1, 0.25] };

test('a vector cell shows its kind and its first values', () => {
  assert.ok(isVectorCell(CELL));
  assert.ok(!isVectorCell([0.5, 1]));
  assert.ok(!isVectorCell({ element: 'f32' }));
  assert.ok(!isVectorCell(null));
  assert.equal(vectorSummary(CELL), 'vector[768 f32]');
  assert.equal(vectorPreview(CELL), '[0.5, -1, 0.25, …]');
  assert.equal(vectorPreview({ ...CELL, dimension: 3 }), '[0.5, -1, 0.25]');
  assert.equal(formatValue(CELL), 'vector[768 f32] [0.5, -1, 0.25, …]');
});

test('the property editor edits a vector as a JSON list and keeps its element', () => {
  assert.deepEqual(VECTOR_ELEMENTS, ['f32', 'f16', 'i8', 'bit']);
  assert.ok(TYPES.includes('vector'));
  assert.ok(TEXT_TYPES.includes('vector'));
  assert.equal(scalarError('vector', placeholderOf('vector')), null);
  const node = nodeFromTyped({ type: 'vector', element: 'i8', value: [0.5, -1, 2] });
  assert.equal(node.text, '[0.5, -1.0, 2.0]');
  assert.equal(node.element, 'i8');
  assert.deepEqual(typedFromNode(node).typed, { type: 'vector', element: 'i8', value: [0.5, -1, 2] });
  // A new vector is f32.
  const fresh = setType(emptyNode('string'), 'vector');
  fresh.text = '[1, 2]';
  assert.deepEqual(typedFromNode(fresh).typed, { type: 'vector', element: 'f32', value: [1, 2] });
});

test('a vector text that is not a list of finite numbers is refused', () => {
  for (const bad of ['', '[]', '1', '[1, "x"]', '{"a": 1}', '[1, null]', 'not json']) {
    assert.ok(scalarError('vector', bad), bad);
    assert.ok(typedFromNode({ ...emptyNode('vector'), text: bad }).error, bad);
  }
  assert.deepEqual(vectorValues(' [0.1, 2e3] ').values, [0.1, 2000]);
  assert.match(vectorValues(JSON.stringify(new Array(65537).fill(0))).error, /65536/);
});

test('raw JSON of a vector is its list, read back as a vector', () => {
  const node = nodeFromTyped({ type: 'vector', element: 'f16', value: [1, 0.5] });
  const text = nodeToJson(node);
  assert.equal(text, '[1.0, 0.5]');
  const back = nodeFromJsonText(text, node).node;
  assert.equal(back.type, 'vector');
  assert.equal(back.element, 'f16');
  assert.deepEqual(typedFromNode(back).typed, { type: 'vector', element: 'f16', value: [1, 0.5] });
  // Without a vector there before, a list stays a list.
  assert.equal(nodeFromJsonText(text).node.type, 'array');
});

test('a declared vector property offers the vector type', () => {
  const schema = { type: 'array', format: 'vector', element: 'i8', dimension: 3 };
  assert.deepEqual(typesOf(schema), ['vector']);
  assert.equal(defaultType(schema), 'vector');
  assert.deepEqual(typesOf({ type: ['array', 'null'], format: 'vector' }), ['vector', 'null']);
  assert.deepEqual(typesOf({ type: 'array', items: { type: 'number' } }), ['array']);
});

test('the schema editor knows the vector base type', () => {
  assert.ok(BASE_TYPES.includes('vector'));
  const doc = { p: { type: 'string', minLength: 1 } };
  setBaseType(doc, ['p'], 'vector');
  assert.deepEqual(doc.p, { type: 'array', format: 'vector', element: 'f32' });
  assert.equal(describe(doc.p).base, 'vector');
  assert.equal(typeText(doc.p), 'vector<f32>');
  doc.p.dimension = 768;
  doc.p.element = 'i8';
  assert.equal(typeText(doc.p), 'vector<i8, 768>');
  // Back to an array: the vector keywords go.
  setBaseType(doc, ['p'], 'array');
  assert.deepEqual(doc.p, { type: 'array', items: { type: 'string' } });
});
