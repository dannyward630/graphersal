// Node tests of the saved-query models (js/saved-queries.js): the folder tree, the editor's
// draft (source split and composed, parameters to schemas and back, field errors) and the run
// form (fields from the parameter schemas, defaults and remembered values, typed arguments).
// Run: node --test playground/tests/*.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  queryTree, signatureText, splitSource, composeSource, newParam, paramFromEntry, paramDeclaration,
  parseDefault, newDraft, draftFromQuery, draftToSpec, errorsByField, formFields, initialValues,
  formArgs, rememberValues, queryNameContext,
} from '../js/saved-queries.js';

const TOP = {
  name: 'top_customers',
  folder: 'reports/sales',
  description: 'Customers of a country by revenue.',
  params: [
    { name: 'country', schema: { type: 'string', enum: ['DE', 'FR'], description: 'ISO code' }, required: true, description: 'ISO code' },
    { name: 'limit', schema: { type: 'integer', minimum: 1, maximum: 1000 }, default: 20, required: false },
  ],
  status: 'ok',
  body: 'fn top_customers(country, limit) {\n  g.v().has("country", country)\n   .limit(limit)\n}',
};

test('tree: root first, folders by path, queries by name', () => {
  const tree = queryTree([
    { name: 'b', folder: 'z' }, { name: 'a', folder: null }, { name: 'c', folder: 'a/b' }, { name: 'a2', folder: 'z' },
  ]);
  assert.deepEqual(tree.map((g) => g.folder), ['', 'a/b', 'z']);
  assert.deepEqual(tree[2].queries.map((q) => q.name), ['a2', 'b']);
  assert.equal(tree[1].depth, 2);
  assert.deepEqual(queryTree(null), []);
  assert.equal(signatureText(TOP), 'top_customers(country, limit = 20)');
});

test('source: split into parameters and body, composed back', () => {
  const split = splitSource(TOP.body, 'top_customers');
  assert.deepEqual(split.params, ['country', 'limit']);
  assert.equal(split.body, '  g.v().has("country", country)\n   .limit(limit)');
  assert.equal(splitSource('fn x() { g.v() }', 'x').body, 'g.v()');
  assert.equal(splitSource('/// doc\nfn x() { 1 }', 'x'), null);
  assert.equal(splitSource('fn y() { 1 }', 'x'), null);
  const draft = draftFromQuery(TOP);
  assert.equal(draft.raw, false);
  assert.equal(composeSource(draft), TOP.body);
  // A doc comment before `fn`: the whole text is edited.
  const raw = draftFromQuery({ ...TOP, body: '// c\n' + TOP.body });
  assert.equal(raw.raw, true);
  assert.equal(composeSource(raw), '// c\n' + TOP.body);
});

test('parameters: schema fields round-trip, unknown keywords become a custom schema', () => {
  const country = paramFromEntry(TOP.params[0]);
  assert.equal(country.type, 'string');
  assert.equal(country.enumText, 'DE, FR');
  assert.equal(country.description, 'ISO code');
  assert.deepEqual(paramDeclaration(country).declaration, {
    name: 'country', schema: { type: 'string', enum: ['DE', 'FR'], description: 'ISO code' },
  });
  const limit = paramFromEntry(TOP.params[1]);
  assert.equal(limit.minimum, '1');
  assert.equal(limit.defaultText, '20');
  assert.deepEqual(paramDeclaration(limit).declaration, {
    name: 'limit', schema: { type: 'integer', minimum: 1, maximum: 1000 }, default: 20,
  });
  const id = paramFromEntry({ name: 'id', schema: { type: 'string', format: 'uuid' } });
  assert.equal(id.type, 'uuid');
  const ids = paramFromEntry({ name: 'ids', schema: { type: 'array', items: { type: 'integer' } }, default: [1, 2] });
  assert.equal(ids.type, 'array');
  assert.equal(ids.itemsType, 'integer');
  assert.equal(ids.defaultText, '[1,2]');
  assert.deepEqual(paramDeclaration(ids).declaration.default, [1, 2]);
  const custom = paramFromEntry({ name: 'age', schema: { type: ['integer', 'null'], exclusiveMinimum: 0 } });
  assert.equal(custom.type, 'custom');
  assert.deepEqual(paramDeclaration(custom).declaration.schema, { type: ['integer', 'null'], exclusiveMinimum: 0 });

  const p = { ...newParam('n'), type: 'integer', minimum: 'x' };
  assert.match(paramDeclaration(p).error, /minimum/);
  assert.match(paramDeclaration({ ...newParam('n'), type: 'integer', defaultText: '1.5' }).error, /integer/);
  assert.deepEqual(parseDefault('boolean', 'true'), { value: true });
  assert.deepEqual(parseDefault('string', ''), { value: undefined });
  assert.ok(parseDefault('uuid', 'nope').error);
  assert.deepEqual(paramDeclaration({ ...newParam('n'), type: 'number', enumText: '1, 2.5' }).declaration.schema.enum, [1, 2.5]);
});

test('draft: spec for define, local field errors, errors by field', () => {
  const draft = newDraft('demo');
  draft.name = 'people';
  draft.params.push({ ...newParam('limit'), type: 'integer', defaultText: '3' });
  draft.body = '  g.v().limit(limit)';
  const { spec } = draftToSpec(draft);
  assert.deepEqual(spec, {
    name: 'people', folder: 'demo', description: null,
    params: [{ name: 'limit', schema: { type: 'integer' }, default: 3 }],
    body: 'fn people(limit) {\n  g.v().limit(limit)\n}',
  });
  const edit = draftFromQuery(TOP);
  edit.name = 'renamed';
  assert.equal(draftToSpec(edit).spec.replaces, 'top_customers');
  assert.match(draftToSpec(edit).spec.body, /^fn renamed\(country, limit\)/);

  const bad = newDraft();
  bad.name = '1x';
  bad.params.push(newParam(''), newParam('a'), newParam('a'));
  const { errors } = draftToSpec(bad);
  assert.deepEqual(errors.map((e) => e.field), ['name', 'params.0', 'params.a']);
  assert.deepEqual(errorsByField([{ field: 'body', message: 'm', help: 'h' }]), { body: ['m h'] });
});

test('run form: fields by schema, defaults prefilled, last values win, typed arguments', () => {
  const entry = {
    name: 'q',
    params: [
      { name: 'country', schema: { type: 'string', enum: ['DE', 'FR'] }, required: true },
      { name: 'limit', schema: { type: 'integer', minimum: 1, maximum: 10 }, default: 5, required: false },
      { name: 'ratio', schema: { type: ['number', 'null'] }, required: true, description: 'share' },
      { name: 'active', schema: { type: 'boolean' }, default: true, required: false },
      { name: 'id', schema: { type: 'string', format: 'uuid' }, required: false, default: '00000000-0000-0000-0000-000000000001' },
      { name: 'tags', schema: { type: 'array', items: { type: 'string' } }, required: true },
      { name: 'opts', schema: { type: 'object' }, required: false, default: { a: 1 } },
      { name: 'text', schema: { type: 'string' }, required: true },
    ],
  };
  const fields = formFields(entry);
  assert.deepEqual(fields.map((f) => f.kind), ['select', 'integer', 'number', 'checkbox', 'uuid', 'list', 'json', 'text']);
  assert.equal(fields[2].hint, 'share');
  assert.equal(fields[1].max, 10);
  const values = initialValues(fields, { country: '"FR"', limit: 7, text: 'kept' });
  assert.equal(values.country, '"FR"');
  assert.equal(values.limit, '5'); // a remembered value of the wrong shape is ignored
  assert.equal(values.active, true);
  assert.equal(values.opts, '{\n  "a": 1\n}');
  assert.equal(values.text, 'kept');

  values.ratio = '0.5';
  values.tags = 'a\n\nb ';
  const { args, errors } = formArgs(fields, values);
  assert.deepEqual(errors, {});
  assert.deepEqual(args, {
    country: 'FR', limit: 5, ratio: 0.5, active: true, id: '00000000-0000-0000-0000-000000000001',
    tags: ['a', 'b'], opts: { a: 1 }, text: 'kept',
  });

  const empty = formArgs(fields, { ...values, limit: '', tags: '', ratio: 'x', id: 'nope', opts: '{', country: '' });
  assert.equal(empty.args.limit, undefined); // left out: the default applies
  assert.equal(empty.errors.tags, 'required');
  assert.equal(empty.errors.country, 'required');
  assert.match(empty.errors.ratio, /not a number/);
  assert.match(empty.errors.id, /UUID/);
  assert.match(empty.errors.opts, /JSON/);
  assert.equal(formArgs(fields, { ...values, limit: '11' }).errors.limit, 'at most 10');
});

test('remembered values: newest last, bounded', () => {
  let all = rememberValues({}, 'a', { x: '1' });
  all = rememberValues(all, 'b', { y: '2' });
  all = rememberValues(all, 'a', { x: '3' });
  assert.deepEqual(Object.keys(all), ['b', 'a']);
  assert.deepEqual(rememberValues(all, 'c', {}, 2), { a: { x: '3' }, c: {} });
});

test('completion context: the name typed in g.query("', () => {
  assert.deepEqual(queryNameContext('let r = g.query("top_'), { quote: '"', prefix: 'top_' });
  assert.deepEqual(queryNameContext('g.query( "'), { quote: '"', prefix: '' });
  assert.equal(queryNameContext('g.query("a", #{x: "'), null);
  assert.equal(queryNameContext('g.v().has("'), null);
});
