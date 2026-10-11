// Node tests of when the Schema panel reads the schema again (js/schema/panel-sync.js over
// js/schema/flow.js): inference only on first show, refresh and a replaced graph; a change of the
// graph reads the stored schema only and marks an inferred one "may be outdated".
// Run: node --test playground/tests/*.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { SchemaFlow } from '../js/schema/flow.js';
import { SchemaPanelSync } from '../js/schema/panel-sync.js';

const INFERRED = { mode: 'none', vertices: { person: { schema: { type: 'object' } } } };
const STORED = { mode: 'open', vertices: { person: { schema: { type: 'object', properties: { name: { type: 'string' } } } } } };

/** A fake schema api counting its calls; `stored` is what `get` answers. */
function fakeApi() {
  const calls = { get: 0, infer: 0, set: 0 };
  const api = {
    stored: null,
    calls,
    get: async () => { calls.get++; return api.stored; },
    infer: async () => { calls.infer++; return structuredClone(INFERRED); },
    set: async (schema) => { calls.set++; api.stored = schema; },
  };
  return api;
}

/** The drawings: records `load(force)` and `drop()`. */
function fakeViews() {
  const log = [];
  return { log, load: async (force) => { log.push(force ? 'load!' : 'load'); }, drop: () => { log.push('drop'); } };
}

function setup() {
  const api = fakeApi();
  const flow = new SchemaFlow(api);
  const views = fakeViews();
  const sync = new SchemaPanelSync(flow, views, 0);
  return { api, flow, views, sync };
}

test('a hidden or collapsed panel reads nothing; the first show infers once', async () => {
  const { api, flow, sync } = setup();
  await sync.setActive(false);
  await sync.graphChanged(0);
  assert.deepEqual(api.calls, { get: 0, infer: 0, set: 0 });
  await sync.setActive(true);
  assert.equal(api.calls.infer, 1);
  assert.equal(flow.ed.source, 'inferred');
  assert.equal(flow.outdated, false);
  await sync.setActive(false);
  await sync.setActive(true); // shown again: nothing new to read
  assert.equal(api.calls.infer, 1);
});

test('a change of the graph does not infer: the inferred schema stays and is marked outdated', async () => {
  const { api, flow, views, sync } = setup();
  await sync.setActive(true);
  const shown = flow.ed.working;
  views.log.length = 0;
  await sync.graphChanged(0);
  await sync.graphChanged(0);
  assert.equal(api.calls.infer, 1, 'no inference after a version bump');
  assert.equal(api.calls.get, 3, 'the stored schema is read on every change');
  assert.equal(flow.outdated, true, 'the badge');
  assert.equal(flow.ed.source, 'inferred');
  assert.deepEqual(flow.ed.working, shown, 'the last inferred schema stays shown');
  assert.deepEqual(views.log, ['load', 'load'], 'the drawings are not forced');
});

test('refresh infers once and clears the badge', async () => {
  const { api, flow, views, sync } = setup();
  await sync.setActive(true);
  await sync.graphChanged(0);
  assert.equal(flow.outdated, true);
  views.log.length = 0;
  await sync.refresh();
  assert.equal(api.calls.infer, 2);
  assert.equal(flow.outdated, false);
  assert.deepEqual(views.log, ['load!']);
});

test('a stored schema still reloads on every change and replaces the inferred one', async () => {
  const { api, flow, sync } = setup();
  await sync.setActive(true);
  await sync.graphChanged(0);
  assert.equal(flow.outdated, true);
  api.stored = STORED; // e.g. a query ran set_schema(...)
  await sync.graphChanged(0);
  assert.equal(flow.ed.source, 'stored');
  assert.deepEqual(flow.ed.stored, STORED);
  assert.equal(flow.outdated, false, 'a stored schema is authoritative: no badge');
  const changed = structuredClone(STORED);
  changed.mode = 'closed';
  api.stored = changed;
  await sync.graphChanged(0);
  assert.equal(flow.ed.stored.mode, 'closed');
  assert.equal(api.calls.infer, 1, 'never inferred while a schema is stored');
});

test('dropping the stored schema shows the badge, without inferring', async () => {
  const { api, flow, sync } = setup();
  api.stored = STORED;
  await sync.setActive(true);
  assert.equal(api.calls.infer, 0);
  api.stored = null;
  await sync.graphChanged(0);
  assert.equal(api.calls.infer, 0);
  assert.equal(flow.ed.stored, null);
  assert.equal(flow.outdated, true);
  await sync.refresh();
  assert.equal(api.calls.infer, 1);
  assert.equal(flow.ed.source, 'inferred');
  assert.equal(flow.outdated, false);
});

test('a replaced graph infers once: now when shown, else on the next show', async () => {
  const { api, flow, views, sync } = setup();
  await sync.setActive(true);
  await sync.graphChanged(1); // a load / view / rollback while shown
  assert.equal(api.calls.infer, 2);
  assert.equal(flow.outdated, false);
  await sync.setActive(false);
  views.log.length = 0;
  await sync.graphChanged(2); // replaced while collapsed: the old drawings are dropped
  assert.equal(api.calls.infer, 2);
  assert.deepEqual(views.log, ['drop']);
  await sync.graphChanged(2); // a plain change before the show reads nothing
  assert.equal(api.calls.infer, 2);
  await sync.setActive(true);
  assert.equal(api.calls.infer, 3);
  assert.equal(flow.outdated, false);
});

test('apply and freeze reload without inferring', async () => {
  const { api, flow, sync } = setup();
  await sync.setActive(true);
  assert.equal(await flow.freeze('open'), true);
  assert.equal(api.calls.infer, 2, 'freeze infers what it stores, nothing more');
  assert.equal(flow.ed.source, 'stored');
  assert.equal(flow.outdated, false);
});
