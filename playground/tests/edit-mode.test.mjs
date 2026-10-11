// The Graph panel's edit mode, its pure part (js/graph/edit-mode.js), the creation form of the
// element editor (js/element-editor.js `formForNew`, `splitHeadErrors`), and how tiers express
// that the Fast tier adds nothing (js/graph/tiers.js `controlStates`).
import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  editModeState, ConnectGesture, applyEdit, carryEditPositions, labelOptions, labelProblem, noEdgeLabelText,
  deleteConfirmText, EDIT_TUNING,
} from '../js/graph/edit-mode.js';
import { formForNew, splitHeadErrors, schemaFields, diffForm } from '../js/element-editor.js';
import { TIER_CAPABILITIES } from '../js/graph/renderer.js';
import { controlStates } from '../js/graph/tiers.js';

/** A path a -> b -> c (vertex ids 1, 2, 3; edge ids 10, 11). */
function path() {
  return {
    vertexIds: ['1', '2', '3'],
    vertexLabel: Uint32Array.from([0, 0, 1]),
    vertexName: ['a', 'b', null],
    edgeIds: ['10', '11'],
    edgeSource: Uint32Array.from([0, 1]),
    edgeTarget: Uint32Array.from([1, 2]),
    edgeLabel: Uint32Array.from([2, 2]),
    labels: ['person', 'software', 'knows'],
    truncated: false,
    totalNodes: 3,
    totalEdges: 2,
  };
}

test('the edit mode switch: off for the label overview, read-only graphs and held drawings; adding needs the tier', () => {
  const on = editModeState({ caps: TIER_CAPABILITIES.detailed, on: true });
  assert.equal(on.active, true);
  assert.equal(on.canAdd, true);
  assert.match(on.hint, /double-click empty space to add a vertex/);
  assert.equal(editModeState({ caps: TIER_CAPABILITIES.detailed, on: false }).active, false);
  assert.equal(editModeState({ caps: TIER_CAPABILITIES.detailed, on: false }).hint, null);

  const fast = editModeState({ caps: TIER_CAPABILITIES.fast, on: true });
  assert.equal(fast.active, true, 'the Fast tier edits and deletes');
  assert.equal(fast.canAdd, false);
  assert.match(fast.hint, /adding needs the Detailed tier/);

  const schema = editModeState({ caps: TIER_CAPABILITIES.detailed, kind: 'schema', on: true });
  assert.equal(schema.disabled, true);
  assert.equal(schema.active, false);
  assert.match(schema.reason, /label overview/);

  const ro = editModeState({ caps: TIER_CAPABILITIES.detailed, readOnly: 'a read-only view of a past state is open', on: true });
  assert.equal(ro.disabled, true);
  assert.equal(ro.active, false);
  assert.equal(ro.reason, 'Read-only: a read-only view of a past state is open');

  assert.equal(editModeState({ caps: TIER_CAPABILITIES.detailed, held: true, on: true }).active, false);
  assert.match(editModeState({ caps: TIER_CAPABILITIES.detailed, running: true, on: true }).hint, /query is running/);
});

test('tiers: the Fast tier\'s add control is disabled with a reason, Detailed\'s is on', () => {
  assert.equal(TIER_CAPABILITIES.detailed.addElements, true);
  assert.equal(TIER_CAPABILITIES.fast.addElements, false);
  const fast = controlStates(TIER_CAPABILITIES.fast, []).addElements;
  assert.equal(fast.disabled, true);
  assert.match(fast.reason, /cannot add vertices or edges with the mouse: switch to Detailed/);
  assert.deepEqual(controlStates(TIER_CAPABILITIES.detailed, []).addElements, { disabled: false, reason: null });
});

test('the connect gesture: a click, a drag to another vertex, a loop, a release elsewhere', () => {
  const g = new ConnectGesture(5);
  assert.equal(g.active, false);
  assert.deepEqual(g.end(1), { type: 'cancel' });

  g.start(0, { x: 0, y: 0 });
  assert.equal(g.move({ x: 2, y: 1 }, 0), null, 'below the threshold: no band');
  assert.deepEqual(g.end(0), { type: 'click', index: 0 });

  g.start(0, { x: 0, y: 0 });
  assert.deepEqual(g.move({ x: 10, y: 0 }, 0), { from: 0, to: { x: 10, y: 0 }, target: null }, 'still on the source: no target yet');
  assert.deepEqual(g.move({ x: 40, y: 0 }, 2), { from: 0, to: { x: 40, y: 0 }, target: 2 });
  assert.deepEqual(g.end(2), { type: 'connect', from: 0, to: 2 });
  assert.equal(g.active, false);

  g.start(1, { x: 0, y: 0 });
  g.move({ x: 20, y: 0 }, 1);
  assert.deepEqual(g.end(1), { type: 'cancel' }, 'released on the source without leaving it');

  g.start(1, { x: 0, y: 0 });
  g.move({ x: 50, y: 0 }, null);
  assert.equal(g.move({ x: 1, y: 0 }, 1).target, 1, 'back on the source after leaving it: a loop');
  assert.deepEqual(g.end(1), { type: 'connect', from: 1, to: 1 });

  g.start(1, { x: 0, y: 0 });
  g.move({ x: 50, y: 0 }, null);
  assert.deepEqual(g.end(null), { type: 'cancel' }, 'released on empty space');
});

test('applyEdit adds a vertex and an edge, removes an edge, and a vertex with its edges', () => {
  const v = path();
  const added = applyEdit(v, { type: 'add-vertex', drawn: { id: '7', label: 'robot', caption: 'r2' } });
  assert.deepEqual(added.vertexIds, ['1', '2', '3', '7']);
  assert.equal(added.labels[added.vertexLabel[3]], 'robot');
  assert.equal(added.vertexName[3], 'r2');
  assert.equal(added.totalNodes, 4);
  assert.deepEqual(v.vertexIds, ['1', '2', '3'], 'the input is not changed');
  assert.equal(applyEdit(added, { type: 'add-vertex', drawn: { id: '7', label: 'robot' } }), added, 'not twice');

  const linked = applyEdit(added, { type: 'add-edge', drawn: { id: '12', label: 'knows', source: '7', target: '1' } });
  assert.deepEqual(linked.edgeIds, ['10', '11', '12']);
  assert.equal(linked.edgeSource[2], 3);
  assert.equal(linked.edgeTarget[2], 0);
  assert.equal(linked.labels[linked.edgeLabel[2]], 'knows');
  assert.equal(linked.labels.length, 4, 'an existing label is reused');
  assert.equal(applyEdit(linked, { type: 'add-edge', drawn: { id: '13', label: 'x', source: '7', target: '99' } }), linked, 'an endpoint not drawn');

  const noEdge = applyEdit(linked, { type: 'remove', kind: 'edge', id: '11' });
  assert.deepEqual(noEdge.edgeIds, ['10', '12']);
  assert.deepEqual(Array.from(noEdge.edgeSource), [0, 3]);

  // Removing vertex 2 takes edges 10 and 11; the indices after it move down.
  const gone = applyEdit(linked, { type: 'remove', kind: 'vertex', id: '2' });
  assert.deepEqual(gone.vertexIds, ['1', '3', '7']);
  assert.deepEqual(gone.vertexName, ['a', null, 'r2']);
  assert.deepEqual(gone.edgeIds, ['12']);
  assert.deepEqual([gone.edgeSource[0], gone.edgeTarget[0]], [2, 0]);
  assert.equal(gone.totalNodes, 3);
  assert.equal(gone.totalEdges, 1);
  assert.equal(applyEdit(gone, { type: 'remove', kind: 'vertex', id: 'nope' }), gone);

  // A first vertex in an empty drawing (no view yet).
  const first = applyEdit(null, { type: 'add-vertex', drawn: { id: '1', label: 'p', caption: null } });
  assert.deepEqual(first.vertexIds, ['1']);
  assert.equal(first.edgeIds.length, 0);
});

test('carryEditPositions keeps every drawn vertex, puts an added one where it was asked for, refuses another drawing', () => {
  const prev = path();
  const xy = Float32Array.from([0, 0, 100, 0, 200, 0]);
  const next = applyEdit(prev, { type: 'add-vertex', drawn: { id: '7', label: 'p' } });
  const out = carryEditPositions(prev, xy, next, new Map([['7', [50, 80]]]));
  assert.deepEqual(Array.from(out), [0, 0, 100, 0, 200, 0, 50, 80]);

  // Without a place: next to its first drawn neighbour.
  const linked = applyEdit(next, { type: 'add-edge', drawn: { id: '12', label: 'k', source: '7', target: '3' } });
  const near = carryEditPositions(prev, xy, linked);
  assert.ok(Math.hypot(near[6] - 200, near[7] - 0) < 100, 'near vertex 3');

  // A removal: the rest stays.
  const gone = applyEdit(prev, { type: 'remove', kind: 'vertex', id: '2' });
  assert.deepEqual(Array.from(carryEditPositions(prev, xy, gone)), [0, 0, 200, 0]);

  // Another drawing (less than half of it drawn before) gets its own layout.
  const other = { ...path(), vertexIds: ['1', '8', '9'] };
  assert.equal(carryEditPositions(prev, xy, other), null);
  assert.equal(carryEditPositions(prev, Float32Array.from([1, 2]), next), null, 'positions of another drawing');

  // The first vertex of an empty drawing goes where it was asked for.
  const empty = applyEdit(null, { type: 'add-vertex', drawn: { id: '1', label: 'p' } });
  const blank = { ...empty, vertexIds: [], vertexLabel: new Uint32Array(0), vertexName: [] };
  assert.deepEqual(Array.from(carryEditPositions(blank, null, empty, new Map([['1', [3, 4]]]))), [3, 4]);
  assert.ok(EDIT_TUNING.carryMs > 0);
});

test('label options and checks from the engine\'s choices', () => {
  const open = {
    kind: 'edge', mode: 'open', free: true,
    labels: [
      { label: 'created', declared: true, count: 4, connects: true },
      { label: 'likes', declared: false, count: 1, connects: null },
      { label: 'knows', declared: true, count: 0, connects: false },
    ],
  };
  assert.deepEqual(labelOptions(open), [
    { label: 'created', note: 'declared, 4 in the graph' },
    { label: 'likes', note: '1 in the graph' },
    { label: 'knows', note: 'schema: connects other labels' },
  ]);
  assert.equal(labelProblem(open, 'anything'), null, 'free text outside closed');
  assert.equal(labelProblem(open, ' '), 'Choose a label.');
  assert.match(labelProblem(open, 'a::b'), /::/);
  const closed = { ...open, mode: 'closed', free: false, labels: [open.labels[0]] };
  assert.equal(labelProblem(closed, 'created'), null);
  assert.match(labelProblem(closed, 'likes'), /closed schema allows no "likes" edge/);
  assert.match(labelProblem({ kind: 'vertex', free: false, labels: [] }, 'x'), /declares no vertex label "x"/);
  assert.equal(noEdgeLabelText(closed, 'person', 'software'), null);
  assert.match(noEdgeLabelText({ ...closed, labels: [] }, 'software', 'person'), /no edge from software to person/);
  assert.deepEqual(labelOptions(null), []);
});

test('the delete confirmation says how many edges go with a vertex', () => {
  assert.deepEqual(deleteConfirmText('vertex', '1', 'marko', 3), { title: 'Delete vertex 1 (marko)?', text: 'Its 3 edges will be deleted with it. One commit.' });
  assert.match(deleteConfirmText('vertex', '9', '', 1).text, /Its 1 edge will/);
  assert.match(deleteConfirmText('vertex', '9', '', 0).text, /no edges/);
  assert.equal(deleteConfirmText('edge', '7', 'knows').title, 'Delete edge 7 (knows)?');
});

test('the creation form follows the label\'s schema and keeps typed values across a label change', () => {
  const schema = {
    mode: 'closed',
    vertices: {
      person: { schema: { type: 'object', properties: { name: { type: 'string' }, age: { type: 'integer' } }, required: ['name'] } },
      software: { schema: { type: 'object', properties: { name: { type: 'string' }, lang: { type: 'string' } } } },
    },
  };
  const person = schemaFields(schema, 'vertex', ['person']);
  const form = formForNew(person);
  assert.deepEqual(form.rows.map((r) => [r.key, r.node ? r.node.type : null]), [['name', 'string'], ['age', null]]);
  form.rows[0].node.text = 'ann';
  form.rows[1].node = { id: 99, type: 'int64', text: '41', bool: false, items: [], entries: [], expanded: true };
  assert.deepEqual(diffForm(form, person).set, [['name', { type: 'string', value: 'ann' }], ['age', { type: 'int64', value: '41' }]]);

  // Required and removed: an error at the row.
  const empty = formForNew(person);
  empty.rows[0].node = null;
  assert.match(diffForm(empty, person).errors.get(empty.rows[0].id), /required/);

  // Another label: name stays (declared there too), age stays as an undeclared row.
  const software = formForNew(schemaFields(schema, 'vertex', ['software']), form);
  assert.deepEqual(software.rows.map((r) => [r.key, r.declared, r.node ? r.node.text : null]), [['name', true, 'ann'], ['lang', true, null], ['age', false, '41']]);
  // No schema: only what was typed.
  assert.deepEqual(formForNew(null, form).rows.map((r) => r.key), ['name', 'age']);
  assert.deepEqual(formForNew(null).rows, []);
});

test('engine errors of a creation: label, id and endpoints apart from the properties', () => {
  const { head, rest } = splitHeadErrors([
    { field: 'label', message: 'Unknown vertex label', help: 'Declare it\nmore' },
    { field: 'id', message: 'taken' },
    { field: 'age', message: 'bad' },
    { field: 'general', message: 'no' },
  ]);
  assert.deepEqual(head, { label: ['Unknown vertex label Declare it'], id: ['taken'] });
  assert.deepEqual(rest.map((e) => e.field), ['age', 'general']);
});
