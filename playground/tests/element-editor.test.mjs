// Node tests of the property editor's model (js/element-editor.js): typed values to form nodes
// and back, type changes without guessing, raw JSON with kept types, the schema mapped to
// fields, compression rule paths, and the diff of a form into the engine's write request.
// Run: node --test playground/tests/*.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  nodeFromTyped, typedFromNode, setType, addItem, addEntry, scalarError, wantsTextarea,
  nodeToJson, parseJson, nodeFromJsonText, typesOf, defaultType, schemaFields, childSchema,
  schemaHint, enumOptions, sameScalar, setScalar, pathNames, compressionPaths, compressedAt,
  formFromElement, newRow, diffForm, engineErrors, formAsNode, applyObjectToForm, summary,
  nodeWithValue, rowChanged,
} from '../js/element-editor.js';

const T = {
  s: (v) => ({ type: 'string', value: v }),
  i: (v) => ({ type: 'int64', value: String(v) }),
  f: (v) => ({ type: 'float64', value: v }),
  u: (v) => ({ type: 'uuid', value: v }),
};
const UUID = '550e8400-e29b-41d4-a716-446655440000';

const NESTED = {
  type: 'object',
  value: [
    ['name', T.s('marko')],
    ['big', T.i('9007199254740993')],
    ['w', T.f(1)],
    ['nan', T.f('NaN')],
    ['ok', { type: 'boolean', value: true }],
    ['none', { type: 'null' }],
    ['id', T.u(UUID)],
    ['tags', { type: 'array', value: [T.s('a'), T.i(2)] }],
  ],
};

test('typed values round trip through form nodes exactly', () => {
  const node = nodeFromTyped(NESTED);
  assert.equal(node.entries[2].node.text, '1.0'); // a float keeps its point
  assert.deepEqual(typedFromNode(node).typed, NESTED);
  assert.equal(summary(node), '{8 keys}');
});

test('a scalar that does not parse is an error with its path', () => {
  const node = nodeFromTyped(NESTED);
  node.entries[7].node.items[1].text = '2.5';
  const r = typedFromNode(node);
  assert.match(r.error, /whole number/);
  assert.deepEqual(r.path, ['tags', 1]);
  assert.equal(scalarError('int64', '9223372036854775808'), 'out of the int64 range');
  assert.equal(scalarError('uuid', UUID.toUpperCase()) !== null, true);
  assert.equal(scalarError('float64', 'Infinity'), null);
});

test('a string stays a string whatever it holds; the user changes the type', () => {
  const n = nodeFromTyped(T.s('42'));
  assert.deepEqual(typedFromNode(n).typed, T.s('42'));
  setType(n, 'int64');
  assert.deepEqual(typedFromNode(n).typed, T.i(42)); // the text is kept, the user chose int64
  setType(n, 'array');
  assert.deepEqual(typedFromNode(n).typed, { type: 'array', value: [] });
  addItem(n, 'boolean');
  const e = addEntry(setType(nodeFromTyped({ type: 'null' }), 'object'), 'k', 'uuid');
  assert.equal(e.node.type, 'uuid');
  assert.deepEqual(typedFromNode(n).typed, { type: 'array', value: [{ type: 'boolean', value: false }] });
});

test('long, multi-line and compressed strings get a textarea', () => {
  assert.equal(wantsTextarea(nodeFromTyped(T.s('short'))), false);
  assert.equal(wantsTextarea(nodeFromTyped(T.s('a\nb'))), true);
  assert.equal(wantsTextarea(nodeFromTyped(T.s('x'.repeat(100)))), true);
  assert.equal(wantsTextarea(nodeFromTyped(T.s('x')), true), true);
  assert.equal(wantsTextarea(nodeFromTyped(T.i(1)), true), false);
});

test('raw JSON keeps number literals and the types of the tree it replaces', () => {
  const node = nodeFromTyped(NESTED);
  const text = nodeToJson(node);
  assert.match(text, /"big": 9007199254740993/);
  assert.match(text, /"w": 1\.0/);
  const back = nodeFromJsonText(text, node);
  assert.deepEqual(typedFromNode(back.node).typed, NESTED); // uuid and NaN kept through the hint
  // Without a hint: integers are int64, other numbers float64, strings strings (never a uuid).
  const fresh = nodeFromJsonText(`{"a": 1, "b": 1.5, "c": 2e3, "d": "${UUID}"}`).node;
  assert.deepEqual(typedFromNode(fresh).typed.value.map(([, t]) => t.type), ['int64', 'float64', 'float64', 'string']);
  assert.match(nodeFromJsonText('{"a": }').error, /Not valid JSON/);
  assert.throws(() => parseJson('[1] x'), /after the value/);
  assert.throws(() => parseJson('['.repeat(200) + ']'.repeat(200)), /deeper/);
  assert.deepEqual(parseJson('"a\\"b"'), { kind: 'string', value: 'a"b' });
});

const SCHEMA = {
  mode: 'open',
  vertices: {
    person: {
      schema: {
        type: 'object',
        properties: {
          name: { type: 'string', description: 'Full name', maxLength: 40 },
          age: { type: ['integer', 'null'], minimum: 0, maximum: 150 },
          uid: { type: 'string', format: 'uuid' },
          role: { type: 'string', enum: ['admin', 'user'] },
          address: { type: 'object', properties: { zip: { type: 'string' } } },
          scores: { type: 'array', items: { type: 'number' } },
        },
        required: ['name'],
      },
    },
    employee: {
      schema: { type: 'object', properties: { name: { type: 'integer' }, badge: { type: 'integer' } }, required: ['badge'], additionalProperties: false },
    },
  },
  edges: { knows: { schema: { type: 'object', properties: { weight: { type: 'number', enum: [0.5, 1] } } } } },
};

test('the schema maps to fields: first label wins, required is the union', () => {
  const f = schemaFields(SCHEMA, 'vertex', ['person', 'employee']);
  assert.equal(f.declared, true);
  assert.equal(f.additional, true); // person admits additional keys (mode open)
  assert.equal(f.fields.get('name').schema.type, 'string');
  assert.equal(f.fields.get('name').required, true);
  assert.equal(f.fields.get('badge').required, true);
  assert.equal(f.fields.get('badge').label, 'employee');
  assert.equal(schemaFields(SCHEMA, 'vertex', ['employee']).additional, false);
  assert.equal(schemaFields(null, 'vertex', ['x']).mode, 'none');
  assert.deepEqual(typesOf(f.fields.get('age').schema), ['int64', 'null']);
  assert.deepEqual(typesOf(f.fields.get('uid').schema), ['uuid']);
  assert.equal(defaultType(f.fields.get('age').schema), 'int64');
  assert.equal(defaultType(null), 'string');
  assert.equal(childSchema(f.fields.get('address').schema, 'zip').type, 'string');
  assert.equal(childSchema(f.fields.get('scores').schema, 0).type, 'number');
  assert.equal(schemaHint(f.fields.get('age').schema), 'int64 | null · ≥ 0, ≤ 150');
});

test('enum options are typed by the declared type', () => {
  const role = enumOptions(SCHEMA.vertices.person.schema.properties.role);
  assert.deepEqual(role.map((o) => o.typed), [T.s('admin'), T.s('user')]);
  const w = enumOptions(SCHEMA.edges.knows.schema.properties.weight);
  assert.deepEqual(w.map((o) => o.typed), [T.f(0.5), T.f(1)]);
  const node = nodeFromTyped(T.s('x'));
  setScalar(node, w[1].typed);
  assert.equal(sameScalar(node, w[1].typed), true);
  assert.equal(enumOptions({ type: 'string' }), null);
});

test('compression rules cover key paths of the element kind and labels', () => {
  assert.deepEqual(pathNames('$.details.career.cv'), ['details', 'career', 'cv']);
  assert.deepEqual(pathNames("$['a b'].c"), ['a b', 'c']);
  assert.deepEqual(pathNames('name'), ['name']);
  assert.equal(pathNames('$.a[0]'), null);
  const rules = [
    { name: 'cv', element: 'vertex', label: 'person', path: '$.details.cv' },
    { name: 'e', element: 'edge', label: 'person', path: '$.note' },
    { name: 'o', element: 'vertex', label: 'other', path: '$.x' },
  ];
  const paths = compressionPaths(rules, 'vertex', ['person']);
  assert.equal(paths.length, 1);
  assert.equal(compressedAt(paths, ['details', 'cv']), 'cv');
  assert.equal(compressedAt(paths, ['details']), null);
  assert.equal(compressedAt(paths, ['details', 0]), null);
});

const ELEMENT = {
  kind: 'vertex', id: '1', label: 'person', labels: ['person'],
  properties: [['name', T.s('marko')], ['age', T.i(29)], ['tags', { type: 'array', value: [T.s('a')] }]],
};

test('an unchanged form writes nothing; changes become set and remove', () => {
  const fields = schemaFields(SCHEMA, 'vertex', ['person']);
  const form = formFromElement(ELEMENT, fields);
  assert.equal(form.rows.filter((r) => r.declared).length, 4); // uid, role, address, scores
  assert.deepEqual(diffForm(form, fields), { set: [], remove: [], errors: new Map(), changed: false });
  form.rows[1].node.text = '30';
  form.rows[2].removed = true;
  form.rows[2].removed = true;
  const role = form.rows.find((r) => r.key === 'role');
  role.node = nodeFromTyped(T.s('admin'));
  const d = diffForm(form, fields);
  assert.deepEqual(d.set, [['age', T.i(30)], ['role', T.s('admin')]]);
  assert.deepEqual(d.remove, ['tags']);
  assert.equal(d.changed, true);
});

test('a renamed key is removed and set; nested changes write the top-level key whole', () => {
  const form = formFromElement(ELEMENT);
  form.rows[0].key = 'fullName';
  addItem(form.rows[2].node, 'int64').text = '7';
  const d = diffForm(form);
  assert.deepEqual(d.remove, ['name']);
  assert.deepEqual(d.set, [['fullName', T.s('marko')], ['tags', { type: 'array', value: [T.s('a'), T.i(7)] }]]);
});

test('local errors: empty or repeated names, bad values, required keys left out', () => {
  const fields = schemaFields(SCHEMA, 'vertex', ['person']);
  const form = formFromElement(ELEMENT, fields);
  const blank = newRow('', 'string');
  const dup = newRow('age', 'int64');
  dup.node.text = '1';
  form.rows.push(blank, dup);
  form.rows[1].node.text = 'old';
  form.rows[0].removed = true; // name is required
  const d = diffForm(form, fields);
  assert.match(d.errors.get(blank.id), /needs a name/);
  assert.match(d.errors.get(dup.id), /set twice/);
  assert.match(d.errors.get(form.rows[1].id), /whole number/);
  assert.match(d.errors.get(form.rows[0].id), /required by the schema/);
  // Mode none enforces nothing: no required error.
  const none = { ...fields, mode: 'none' };
  assert.equal(diffForm(form, none).errors.has(form.rows[0].id), false);
});

test('engine errors land on their rows, the rest is general', () => {
  const form = formFromElement(ELEMENT);
  const { byRow, general } = engineErrors(form, [
    { field: 'age', message: 'too small', help: 'Use 0 or more.' },
    { field: 'gone', message: 'x' },
    { field: 'general', message: 'vetoed' },
  ]);
  assert.deepEqual(byRow.get(form.rows[1].id), ['too small Use 0 or more.']);
  assert.deepEqual(general, ['gone: x', 'vetoed']);
});

test('the whole element as raw JSON: keys kept, added and removed', () => {
  const form = formFromElement(ELEMENT);
  const text = nodeToJson(formAsNode(form));
  const edited = text.replace('"age": 29', '"age": 30').replace(/,\s*"tags": \[[^\]]*\]/, ', "extra": true');
  const { node } = nodeFromJsonText(edited, formAsNode(form));
  applyObjectToForm(form, node);
  const d = diffForm(form);
  assert.deepEqual(d.set, [['age', T.i(30)], ['extra', { type: 'boolean', value: true }]]);
  assert.deepEqual(d.remove, ['tags']);
});

test('the add row: a new row holds the typed value of its type', () => {
  assert.equal(newRow('n', 'int64', { text: '42' }).node.text, '42');
  assert.equal(newRow('b', 'boolean', { bool: true }).node.bool, true);
  assert.equal(newRow('o', 'object', { text: 'x' }).node.text, '');
  assert.equal(newRow('z', 'null', { text: 'x' }).node.text, '');
  assert.equal(nodeWithValue('nope', { text: 'x' }).type, 'string');
  assert.deepEqual(typedFromNode(newRow('w', 'float64', { text: '0.5' }).node).typed, T.f(0.5));
});

test('changed rows: new, renamed, removed or set; untouched and unset declared rows are not', () => {
  const fields = schemaFields(SCHEMA, 'vertex', ['person']);
  const form = formFromElement(ELEMENT, fields);
  const [name, age, tags] = form.rows;
  const declared = form.rows.find((r) => r.declared);
  assert.equal(form.rows.some((r) => rowChanged(r, diffForm(form, fields))), false);
  age.node.text = '30';
  tags.removed = true;
  const added = newRow('x', 'string', { text: 'y' });
  form.rows.push(added);
  const d = diffForm(form, fields);
  assert.equal(rowChanged(name, d), false);
  assert.equal(rowChanged(age, d), true);
  assert.equal(rowChanged(tags, d), true);
  assert.equal(rowChanged(added, d), true);
  assert.equal(rowChanged(declared, d), false);
  name.key = 'fullName';
  assert.equal(rowChanged(name, diffForm(form, fields)), true);
});
