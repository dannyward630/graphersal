// Node tests of the schema editor's model (js/schema/draft.js) and flow (js/schema/flow.js):
// edits -> draft -> merge patch, JSON <-> form sync, the compact type/constraint texts, the
// queries a violation opens, and the draft -> diff -> check -> apply flow against a fake api.
// Run: node --test playground/tests/*.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  SchemaDraft, applyMergePatch, createMergePatch, describe, typeText, constraintSummary,
  setBaseType, setNullable, setRequired, addProperty, renameProperty, removeProperty, setKeyword,
  addLabel, renameLabel, removeLabel, addConnection, setConnection, removeConnection, nestedObjectPath,
  parseSchemaText, violationQuery, backfillScript, rhaiLiteral, getAt,
} from '../js/schema/draft.js';
import { SchemaFlow } from '../js/schema/flow.js';

const PERSON = ['vertices', 'person', 'schema'];

function stored() {
  return {
    $schema: 'https://graphersal.dev/schema/v1',
    mode: 'open',
    vertices: {
      person: {
        schema: {
          type: 'object',
          properties: {
            name: { type: 'string', minLength: 1, maxLength: 80 },
            age: { type: ['integer', 'null'], minimum: 0 },
            address: {
              type: 'object',
              properties: { zip: { type: 'string', pattern: '^\\d{5}$' }, city: { type: 'string' } },
              required: ['zip'],
            },
          },
          required: ['name'],
        },
      },
    },
    edges: {
      knows: { connections: [{ from: 'person', to: 'person' }], schema: { type: 'object', properties: { weight: { type: 'number', maximum: 10 } } } },
    },
  };
}

test('merge patch: RFC 7396 apply, and the patch from base to target round-trips', () => {
  assert.deepEqual(applyMergePatch({ a: 1, b: { c: 2, d: 3 } }, { a: null, b: { c: 5 }, e: [1] }), { b: { c: 5, d: 3 }, e: [1] });
  assert.deepEqual(applyMergePatch({ a: 1 }, [1, 2]), [1, 2]);
  const base = stored();
  const target = stored();
  delete target.vertices.person.schema.properties.age;
  target.vertices.person.schema.properties.email = { type: 'string' };
  target.mode = 'closed';
  const { patch, exact } = createMergePatch(base, target);
  assert.equal(exact, true);
  assert.deepEqual(patch, {
    mode: 'closed',
    vertices: { person: { schema: { properties: { age: null, email: { type: 'string' } } } } },
  });
  assert.deepEqual(applyMergePatch(base, patch), target);
  assert.deepEqual(createMergePatch(base, stored()).patch, {}, 'no change, empty patch');
  // Arrays are replaced as a whole.
  const req = stored();
  req.vertices.person.schema.required = ['name', 'age'];
  assert.deepEqual(createMergePatch(base, req).patch, { vertices: { person: { schema: { required: ['name', 'age'] } } } });
});

test('merge patch: a member set TO null cannot be expressed (exact: false)', () => {
  const target = stored();
  target.vertices.person.schema.properties.age.default = null;
  assert.equal(createMergePatch(stored(), target).exact, false);
  const nested = { a: { b: null } };
  assert.equal(createMergePatch({}, nested).exact, false);
});

test('describe and typeText: plain, nullable, anyOf, uuid, arrays, objects, unions, any', () => {
  assert.deepEqual(describe({ type: 'string' }), { base: 'string', nullable: false, value: [] });
  assert.deepEqual(describe({ type: ['integer', 'null'] }), { base: 'integer', nullable: true, value: [] });
  assert.deepEqual(describe({ anyOf: [{ type: 'object', properties: {} }, { type: 'null' }], description: 'x' }),
    { base: 'object', nullable: true, value: ['anyOf', 0] });
  assert.equal(describe({ type: 'string', format: 'uuid' }).base, 'uuid');
  assert.equal(describe({}).base, 'any');
  assert.equal(describe({ type: 'null' }).base, 'null');
  assert.equal(describe({ type: ['integer', 'string'] }).base, 'union');
  assert.equal(typeText({ type: ['integer', 'string'] }), 'integer | string');
  assert.equal(typeText({ anyOf: [{ type: 'string', format: 'uuid' }, { type: 'integer' }] }), 'anyOf<uuid | integer>');
  assert.equal(typeText({ type: 'array', items: { type: 'integer' } }), 'array of integer');
  assert.equal(typeText({ type: 'array', items: { type: 'object', properties: { a: {}, b: {} } } }), 'array of object {2 fields}');
  assert.equal(typeText({ type: 'object', properties: { a: {} } }, { collapsed: true }), 'object {1 field}');
  assert.equal(typeText({ type: 'array' }), 'array');
});

test('constraintSummary: the compact column', () => {
  assert.deepEqual(constraintSummary({ type: 'integer', minimum: 0, maximum: 150 }), ['0..150']);
  assert.deepEqual(constraintSummary({ type: 'number', maximum: 10 }), ['<= 10']);
  assert.deepEqual(constraintSummary({ type: 'number', exclusiveMinimum: 0, maximum: 1 }), ['> 0, <= 1']);
  assert.deepEqual(constraintSummary({ type: 'string', minLength: 1, maxLength: 80 }), ['length 1..80']);
  assert.deepEqual(constraintSummary({ type: 'string', pattern: '^\\d{5}$' }), ['^\\d{5}$']);
  assert.deepEqual(constraintSummary({ type: 'array', minItems: 1 }), ['items >= 1']);
  assert.deepEqual(constraintSummary({ enum: ['a', 'b'] }), ['one of a, b']);
  assert.deepEqual(constraintSummary({ const: 3 }), ['= 3']);
  assert.deepEqual(constraintSummary({ anyOf: [{ type: 'integer', minimum: 1 }, { type: 'null' }] }), ['>= 1']);
});

test('setBaseType keeps nullability and annotations, drops keywords of another kind', () => {
  const doc = { p: { type: ['string', 'null'], minLength: 2, description: 'd', enum: ['a'] } };
  setBaseType(doc, ['p'], 'integer');
  assert.deepEqual(doc.p, { type: ['integer', 'null'], description: 'd' });
  setBaseType(doc, ['p'], 'uuid');
  assert.deepEqual(doc.p, { type: ['string', 'null'], format: 'uuid', description: 'd' });
  setBaseType(doc, ['p'], 'array');
  assert.deepEqual(doc.p, { type: ['array', 'null'], description: 'd', items: { type: 'string' } });
  setBaseType(doc, ['p', 'items'], 'object');
  assert.deepEqual(doc.p.items, { type: 'object', properties: {} });
  setBaseType(doc, ['p'], 'any');
  assert.deepEqual(doc.p, { description: 'd' });
  // A nullable anyOf (Pydantic's Optional[X]) becomes the plain form.
  const o = { q: { anyOf: [{ type: 'integer', minimum: 1 }, { type: 'null' }], title: 'Q' } };
  setBaseType(o, ['q'], 'number');
  assert.deepEqual(o.q, { type: ['number', 'null'], title: 'Q', minimum: 1 });
});

test('setNullable on the type array and on the anyOf form', () => {
  const doc = { a: { type: 'string' }, b: { anyOf: [{ type: 'integer' }, { type: 'null' }], description: 'b' }, c: {} };
  setNullable(doc, ['a'], true);
  assert.deepEqual(doc.a.type, ['string', 'null']);
  setNullable(doc, ['a'], false);
  assert.equal(doc.a.type, 'string');
  setNullable(doc, ['b'], false);
  assert.deepEqual(doc.b, { type: 'integer', description: 'b' });
  setNullable(doc, ['c'], false);
  assert.deepEqual(doc.c, {}, 'any admits null; nothing to toggle');
});

test('properties: add, rename (order and required kept), remove, required toggle', () => {
  const doc = stored();
  addProperty(doc, PERSON, 'email');
  assert.deepEqual(Object.keys(doc.vertices.person.schema.properties), ['name', 'age', 'address', 'email']);
  assert.throws(() => addProperty(doc, PERSON, 'email'), /already exists/);
  assert.throws(() => addProperty(doc, PERSON, '  '), /needs a name/);
  renameProperty(doc, PERSON, 'name', 'fullName');
  assert.deepEqual(Object.keys(doc.vertices.person.schema.properties), ['fullName', 'age', 'address', 'email']);
  assert.deepEqual(doc.vertices.person.schema.required, ['fullName']);
  setRequired(doc, PERSON, 'email', true);
  assert.deepEqual(doc.vertices.person.schema.required, ['fullName', 'email']);
  removeProperty(doc, PERSON, 'fullName');
  removeProperty(doc, PERSON, 'email');
  assert.equal(doc.vertices.person.schema.required, undefined, 'an empty required list is dropped');
  // A label without a schema gets one with its first property.
  const bare = { vertices: { thing: {} } };
  addProperty(bare, ['vertices', 'thing', 'schema'], 'x');
  assert.deepEqual(bare.vertices.thing, { schema: { type: 'object', properties: { x: { type: 'string' } } } });
});

test('nested tables: the object path of objects, arrays of objects and nullable anyOf objects', () => {
  const doc = stored();
  doc.vertices.person.schema.properties.roles = { type: 'array', items: { type: 'object', properties: { n: { type: 'string' } } } };
  doc.vertices.person.schema.properties.boss = { anyOf: [{ type: 'object', properties: {} }, { type: 'null' }] };
  const p = [...PERSON, 'properties'];
  assert.deepEqual(nestedObjectPath(doc, [...p, 'address']), [...p, 'address']);
  assert.deepEqual(nestedObjectPath(doc, [...p, 'roles']), [...p, 'roles', 'items']);
  assert.deepEqual(nestedObjectPath(doc, [...p, 'boss']), [...p, 'boss', 'anyOf', 0]);
  assert.equal(nestedObjectPath(doc, [...p, 'name']), null);
  addProperty(doc, nestedObjectPath(doc, [...p, 'roles']), 'since', { type: 'integer' });
  assert.deepEqual(Object.keys(doc.vertices.person.schema.properties.roles.items.properties), ['n', 'since']);
});

test('labels and connections: add, rename (connections follow), remove', () => {
  const doc = stored();
  addLabel(doc, 'vertex', 'company');
  addLabel(doc, 'edge', 'works_at');
  addConnection(doc, 'works_at');
  setConnection(doc, 'works_at', 0, 'from', 'person');
  setConnection(doc, 'works_at', 0, 'to', 'company');
  assert.deepEqual(doc.edges.works_at.connections, [{ from: 'person', to: 'company' }]);
  assert.throws(() => addLabel(doc, 'vertex', 'person'), /already exists/);
  renameLabel(doc, 'vertex', 'person', 'human');
  assert.deepEqual(Object.keys(doc.vertices), ['human', 'company']);
  assert.deepEqual(doc.edges.knows.connections, [{ from: 'human', to: 'human' }]);
  assert.deepEqual(doc.edges.works_at.connections, [{ from: 'human', to: 'company' }]);
  removeConnection(doc, 'works_at', 0);
  assert.deepEqual(doc.edges.works_at.connections, []);
  removeLabel(doc, 'edge', 'works_at');
  assert.equal(doc.edges.works_at, undefined);
});

test('SchemaDraft: edits start a draft from the working schema; the patch follows the edits', () => {
  const ed = new SchemaDraft();
  ed.setEngineState({ stored: stored(), inferred: null });
  assert.equal(ed.source, 'stored');
  assert.equal(ed.dirty, false);
  const v0 = ed.version;
  ed.edit((d) => setKeyword(d, [...PERSON, 'properties', 'age'], 'maximum', 150));
  ed.edit((d) => { d.mode = 'closed'; });
  assert.equal(ed.source, 'draft');
  assert.ok(ed.version > v0);
  assert.deepEqual(ed.patch, {
    patch: { mode: 'closed', vertices: { person: { schema: { properties: { age: { maximum: 150 } } } } } },
    exact: true,
  });
  assert.equal(ed.stored.mode, 'open', 'the stored schema is never edited');
  ed.discard();
  assert.equal(ed.draft, null);
  assert.equal(ed.source, 'stored');
});

test('SchemaDraft: without a stored schema the patch is the whole draft (base {})', () => {
  const ed = new SchemaDraft();
  const inferred = { $schema: 'https://graphersal.dev/schema/v1', mode: 'none', vertices: { a: {} }, edges: {} };
  ed.setEngineState({ stored: null, inferred });
  assert.equal(ed.source, 'inferred');
  ed.edit((d) => { d.mode = 'open'; });
  assert.deepEqual(ed.patch.patch, { ...inferred, mode: 'open' });
});

test('JSON text: made on first read only, kept until the working schema changes', () => {
  // A schema inferred from a graph with one label per vertex is megabytes of JSON: a new engine
  // state (every reload) must not format it while nobody reads the text.
  let formatted = 0;
  const big = (mode) => ({
    mode, vertices: { a: {} }, edges: {},
    toJSON() { formatted++; return { mode: this.mode, vertices: this.vertices, edges: this.edges }; },
  });
  const ed = new SchemaDraft();
  ed.setEngineState({ stored: null, inferred: big('none') });
  ed.setEngineState({ stored: null, inferred: big('none') });
  assert.equal(formatted, 0, 'a new engine state formats nothing');
  assert.equal(JSON.parse(ed.jsonText).mode, 'none');
  assert.equal(JSON.parse(ed.jsonText).mode, 'none');
  assert.equal(formatted, 1, 'the text is made once and kept');
  ed.setEngineState({ stored: big('open'), inferred: null });
  assert.equal(JSON.parse(ed.jsonText).mode, 'open', 'a new working schema: a new text');
  // An edit in place (the same draft object) makes the text again.
  ed.edit((d) => { d.mode = 'closed'; });
  assert.equal(JSON.parse(ed.jsonText).mode, 'closed');
  ed.edit((d) => { d.mode = 'none'; });
  assert.equal(JSON.parse(ed.jsonText).mode, 'none');
  // The user's own text stays what they typed until a form edit or a discard.
  ed.setJsonText('{"mode": "open",');
  assert.equal(ed.jsonText, '{"mode": "open",');
  ed.discard();
  assert.equal(JSON.parse(ed.jsonText).mode, 'open');
});

test('JSON <-> forms: valid text becomes the draft, invalid text only shows the error', () => {
  const ed = new SchemaDraft();
  ed.setEngineState({ stored: stored(), inferred: null });
  assert.deepEqual(JSON.parse(ed.jsonText), stored(), 'the text shows the working schema');
  // Text -> forms.
  const edited = stored();
  edited.mode = 'closed';
  assert.equal(ed.setJsonText(JSON.stringify(edited)), true);
  assert.equal(ed.draft.mode, 'closed');
  assert.equal(ed.draftOrigin, 'json');
  // Invalid text: the error shows, the forms keep the last valid state.
  assert.equal(ed.setJsonText('{"mode": "open",'), false);
  assert.match(ed.jsonError, /Not valid JSON/);
  assert.equal(ed.draft.mode, 'closed');
  assert.equal(ed.setJsonText('[1]'), false);
  assert.match(ed.jsonError, /JSON object/);
  // Forms -> text: an edit rewrites the text and clears the error.
  ed.edit((d) => { d.mode = 'none'; });
  assert.equal(ed.jsonError, null);
  assert.equal(JSON.parse(ed.jsonText).mode, 'none');
  // Text equal to the working schema starts no draft.
  const fresh = new SchemaDraft();
  fresh.setEngineState({ stored: stored(), inferred: null });
  fresh.setJsonText(JSON.stringify(stored(), null, 4));
  assert.equal(fresh.draft, null);
  assert.ok(parseSchemaText('{}').ok);
});

test('violationQuery: exact filters where a traversal can express the rule, sample ids otherwise', () => {
  const g = (o) => ({ element: 'vertex', label: 'person', field: null, detail: null, count: 2, sample_ids: ['1', '2'], omitted_ids: 0, ...o });
  assert.equal(violationQuery(g({ kind: 'unknown_label', label: 'Alien' })).script,
    '// Alien: unknown label, 2 vertices\ng.V().hasLabel("Alien")');
  assert.equal(violationQuery(g({ kind: 'missing_required', field: 'email' })).script,
    '// person.email: missing required, 2 vertices\ng.V().hasLabel("person").not(__.has("email"))');
  assert.equal(violationQuery(g({ kind: 'undeclared_property', element: 'edge', label: 'knows', field: 'weight' })).script,
    '// knows.weight: undeclared property, 2 edges\ng.E().hasLabel("knows").has("weight")');
  assert.equal(violationQuery(g({ kind: 'constraint_violation', field: 'name', detail: 'length >= 5, found length 4', count: 1, sample_ids: ['4'] })).script,
    '// person.name: constraint violation (length >= 5, found length 4), 1 vertex\ng.V("4")');
  assert.equal(violationQuery(g({ kind: 'missing_required', field: '$.address.zip', count: 7, sample_ids: ['1', '2'], omitted_ids: 5 })).script,
    '// person $.address.zip: missing required, 7 vertices\n// the report keeps 2 sample ids of 7\ng.V("1", "2")');
  assert.equal(violationQuery(g({ kind: 'missing_required', field: '$.addresses[*].zip', example_field: '$.addresses[1].zip', count: 5, sample_ids: ['1', '2'], omitted_ids: 3 })).script,
    '// person $.addresses[*].zip: missing required, 5 vertices (first at $.addresses[1].zip)\n// the report keeps 2 sample ids of 5\ng.V("1", "2")');
  assert.equal(violationQuery(g({ kind: 'invalid_topology', element: 'edge', label: 'knows', field: '(:person)-[:knows]->(:person)', sample_ids: ['0', '2'] })).script,
    '// knows (:person)-[:knows]->(:person): invalid topology, 2 edges\ng.E("0", "2")');
  assert.equal(violationQuery(g({ kind: 'type_mismatch', field: 'age', sample_ids: ['x'.repeat(39) + '…'] })).script,
    '// person.age: type mismatch, 2 vertices\ng.V().hasLabel("person")', 'no usable id: every element of the label');
});

test('backfillScript: relax, backfill (default or placeholder), set the draft', () => {
  const draft = stored();
  draft.mode = 'closed';
  draft.vertices.person.schema.properties.email = { type: 'string', default: 'n/a' };
  draft.vertices.person.schema.properties.score = { type: 'integer' };
  draft.vertices.person.schema.required = ['name', 'email', 'score'];
  const groups = [
    { kind: 'missing_required', element: 'vertex', label: 'person', field: 'email', count: 4 },
    { kind: 'missing_required', element: 'vertex', label: 'person', field: 'score', count: 4 },
    { kind: 'missing_required', element: 'vertex', label: 'person', field: '$.address.zip', count: 1 },
    { kind: 'type_mismatch', element: 'vertex', label: 'person', field: 'age', count: 1 },
  ];
  const { script } = backfillScript(draft, groups);
  const lines = script.split('\n');
  assert.ok(lines.includes('g.V().hasLabel("person").not(__.has("email")).property("email", "n/a").toList();  // the draft\'s default'));
  assert.ok(lines.includes('g.V().hasLabel("person").not(__.has("score")).property("score", 0).toList();  // placeholder: edit it'));
  const sets = lines.filter((l) => l.startsWith('g.set_schema('));
  assert.equal(sets.length, 2);
  assert.equal(JSON.parse(JSON.parse(sets[0].slice('g.set_schema('.length, -2))).mode, 'none');
  assert.deepEqual(JSON.parse(JSON.parse(sets[1].slice('g.set_schema('.length, -2))), draft);
  assert.equal(backfillScript(draft, [groups[3]]), null);
  assert.equal(rhaiLiteral({ a: [1, null, 'x'], 'b c': true }), '#{"a": [1, (), "x"], "b c": true}');
});

/** A fake of the seven methods over an in-memory stored schema, recording the calls. */
function fakeApi({ initial = null, violations = 0 } = {}) {
  const calls = [];
  let current = initial;
  const report = () => ({ mode: 'closed', elements_checked: 6, violating_elements: violations ? 1 : 0, total_violations: violations, omitted_violations: 0, groups: [] });
  return {
    calls,
    setViolations(n) { violations = n; },
    get: async () => { calls.push('get'); return current; },
    set: async (s, mode) => { calls.push(mode ? `set:${mode}` : 'set'); current = mode ? { ...s, mode } : s; },
    patch: async (p) => { calls.push('patch'); current = applyMergePatch(current ?? {}, p); return current; },
    validate: async () => { calls.push('validate'); return { violations: report(), diff: [{ kind: 'mode_changed' }], compatible: false }; },
    validatePatch: async () => { calls.push('validatePatch'); return { violations: report(), diff: [{ kind: 'mode_changed' }], compatible: false }; },
    infer: async () => { calls.push('infer'); return { mode: 'none', vertices: { person: {} }, edges: {} }; },
    diff: async () => { calls.push('diff'); return { changes: [{ kind: 'mode_changed', compatible: false }], compatible: false }; },
  };
}

test('flow: diff -> check -> apply with patch_schema; results go stale on an edit', async () => {
  const api = fakeApi({ initial: stored() });
  const flow = new SchemaFlow(api);
  await flow.load();
  assert.deepEqual(api.calls, ['get'], 'a stored schema needs no inference');
  flow.ed.edit((d) => { d.mode = 'closed'; });
  await flow.whatChanges();
  assert.equal(flow.freshDiff.changes.length, 1);
  await flow.checkAgainstData();
  assert.equal(flow.freshCheck.via, 'patch');
  assert.equal(flow.blocked, false);
  flow.ed.edit((d) => { d.meta = { v: 2 }; });
  assert.equal(flow.freshDiff, null, 'stale after an edit');
  assert.equal(flow.freshCheck, null);
  assert.equal(await flow.apply(), true);
  assert.deepEqual(api.calls, ['get', 'diff', 'validatePatch', 'validatePatch', 'patch', 'get']);
  assert.equal(flow.ed.draft, null);
  assert.equal(flow.ed.stored.mode, 'closed');
  assert.deepEqual(flow.ed.stored.meta, { v: 2 });
});

test('flow: violations block Apply; a draft with a null member is set, not patched', async () => {
  const api = fakeApi({ initial: null, violations: 3 });
  const flow = new SchemaFlow(api);
  await flow.load();
  assert.deepEqual(api.calls, ['get', 'infer']);
  assert.equal(flow.ed.source, 'inferred');
  flow.ed.edit((d) => { d.mode = 'closed'; });
  assert.equal(await flow.apply(), false);
  assert.equal(flow.blocked, true);
  assert.equal(flow.canApply, false);
  assert.ok(!api.calls.includes('patch'));
  api.setViolations(0);
  flow.ed.edit((d) => { d.vertices.person = { schema: { type: 'object', properties: { x: { type: 'string', default: null } } } }; });
  assert.equal(flow.ed.patch.exact, false);
  assert.equal(await flow.apply(), true);
  assert.deepEqual(api.calls.slice(-3), ['validate', 'set', 'get']);
});

test('flow: freeze stores the inferred schema with a mode in one step', async () => {
  const api = fakeApi({ initial: null });
  const flow = new SchemaFlow(api);
  await flow.load();
  assert.equal(flow.canFreeze, true, 'offered without a stored schema');
  assert.equal(await flow.freeze('closed'), true);
  assert.deepEqual(api.calls, ['get', 'infer', 'infer', 'set:closed', 'get']);
  assert.equal(flow.ed.stored.mode, 'closed');
  assert.equal(flow.canFreeze, false, 'not offered over an enforcing schema');
  // A failure (unlabeled elements in closed mode) is shown, nothing changes.
  const failing = fakeApi({ initial: { mode: 'none', vertices: {}, edges: {} } });
  failing.set = async () => { throw new Error('Cannot apply the Closed schema ... 1 vertex without a label'); };
  const f = new SchemaFlow(failing);
  await f.load();
  assert.equal(f.canFreeze, true, 'offered over a mode-none schema');
  assert.equal(await f.freeze('closed'), false);
  assert.match(f.freezeError, /without a label/);
  assert.equal(f.busy, null);
  assert.equal(f.ed.stored.mode, 'none');
  // A draft in progress hides it.
  f.ed.edit((d) => { d.mode = 'open'; });
  assert.equal(f.canFreeze, false);
});

test('flow: a reload of the engine state keeps the draft but drops the review', async () => {
  const api = fakeApi({ initial: stored() });
  const flow = new SchemaFlow(api);
  await flow.load();
  flow.ed.edit((d) => { d.mode = 'closed'; });
  await flow.checkAgainstData();
  assert.ok(flow.freshCheck);
  await flow.load(); // e.g. a query changed the stored schema
  assert.equal(flow.freshCheck, null);
  assert.equal(flow.freshDiff, null);
  assert.equal(flow.ed.draft.mode, 'closed');
});

test('flow: an engine error of the check is shown, not thrown', async () => {
  const api = fakeApi({ initial: stored() });
  api.validatePatch = async () => { throw new Error("Invalid schema at /mdoe: unknown key 'mdoe'"); };
  const flow = new SchemaFlow(api);
  await flow.load();
  flow.ed.edit((d) => { d.mdoe = 'closed'; });
  assert.equal(await flow.apply(), false);
  assert.match(flow.freshCheckError, /unknown key 'mdoe'/);
  await flow.inferDraft();
  assert.equal(flow.ed.draftOrigin, 'inferred');
  assert.deepEqual(getAt(flow.ed.draft, ['vertices', 'person']), {});
});

test('defaultSummary shows a fixed default and a default function of a property', async () => {
  const { defaultSummary } = await import('../js/schema/draft.js');
  assert.equal(defaultSummary({ type: 'string', format: 'date-time', defaultFunction: 'now' }), 'default now()');
  assert.equal(defaultSummary({ type: 'integer', default: 5 }), 'default 5');
  assert.equal(defaultSummary({ type: 'string' }), null);
});

test('default functions are offered by type and only on top-level properties', async () => {
  const { DEFAULT_FUNCTIONS, isTopLevelProperty } = await import('../js/schema/draft.js');
  assert.deepEqual([...DEFAULT_FUNCTIONS.uuid], ['uuidv4', 'uuidv7']);
  assert.deepEqual([...DEFAULT_FUNCTIONS.datetime], ['now']);
  assert.deepEqual([...DEFAULT_FUNCTIONS.date], ['today']);
  assert.equal(DEFAULT_FUNCTIONS.string, undefined);
  assert.equal(isTopLevelProperty(['vertices', 'event', 'schema', 'properties', 'created']), true);
  assert.equal(isTopLevelProperty(['vertices', 'event', 'schema', 'properties', 'a', 'properties', 'b']), false);
});
