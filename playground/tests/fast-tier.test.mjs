// Node tests of the Fast tier's pure parts: the controller's renderer swap (positions survive a
// tier switch) and capability-aware layouts (js/graph/controller.js), the drill-down helpers
// (neighbourhood, sub-view, element cap, built-in positions of js/graph-data.js), and the registry
// of running layouts that the toolbar's Stop cancels (js/graph/layout-jobs.js).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { GraphController, layoutIn } from '../js/graph/controller.js';
import { LayoutJobs } from '../js/graph/layout-jobs.js';
import { TIER_CAPABILITIES, RENDERER_METHODS } from '../js/graph/renderer.js';
import { LabelPalette } from '../js/graph/palette.js';
import {
  adjacencyOf, neighbourhood, subView, idsOfIndices, capToElements, circlePositions, gridPositions, elementCount, layoutFor,
} from '../js/graph-data.js';

/** A renderer that records its calls; `caps` picks the tier. */
class FakeRenderer {
  constructor(name, caps) {
    this.name = name;
    this.capabilities = caps;
    this.calls = [];
    this.positions = null;
  }
  names() { return this.calls.map((c) => c[0]); }
  setData(data, options) { this.calls.push(['setData', data, options]); this.positions = null; }
  setPositions(xy) { this.calls.push(['setPositions', xy]); this.positions = xy; }
  getPositions() { return this.positions; }
  async setLayout(name, options) {
    this.calls.push(['setLayout', name, options]);
    const n = this.calls.find((c) => c[0] === 'setData')[1].vertexIds.length;
    this.positions = Float32Array.from({ length: 2 * n }, (_, k) => k + 0.5);
  }
  setOptions(o) { this.calls.push(['setOptions', o]); }
  fit() { this.calls.push(['fit']); }
  resize() {}
  select() {}
  onClick() {}
  onDoubleClick() {}
  onUserMove() {}
  setEditMode() {}
  onConnect() {}
  async exportPng() { return null; }
  setTheme() {}
  destroy() { this.calls.push(['destroy']); }
}

function fakeEngine(slices = 3) {
  const jobs = new Map();
  let next = 0;
  const calls = [];
  const status = (job) => {
    const j = jobs.get(job);
    const out = { job, state: j.state, iterations: j.done, totalIterations: slices, progress: j.done / slices };
    if (j.state !== 'running') out.positions = Float32Array.from({ length: 2 * j.n }, (_, k) => -k);
    return out;
  };
  return {
    calls,
    async layoutStart(input) { calls.push('start'); const job = ++next; jobs.set(job, { n: input.vertexCount, done: 1, state: 'running' }); return status(job); },
    async layoutStep(job) { calls.push('step'); const j = jobs.get(job); if (++j.done >= slices) j.state = 'done'; return status(job); },
    async layoutCancel(job) { calls.push('cancel'); const j = jobs.get(job); j.state = 'cancelled'; return status(job); },
  };
}

/** n vertices in a chain 0-1-2-..., two labels. */
function chain(n) {
  const m = Math.max(0, n - 1);
  return {
    vertexIds: Array.from({ length: n }, (_, i) => `v${i}`),
    vertexLabel: Uint32Array.from({ length: n }, (_, i) => i % 2),
    vertexName: Array.from({ length: n }, (_, i) => `n${i}`),
    edgeIds: Array.from({ length: m }, (_, j) => `e${j}`),
    edgeSource: Uint32Array.from({ length: m }, (_, j) => j),
    edgeTarget: Uint32Array.from({ length: m }, (_, j) => j + 1),
    edgeLabel: new Uint32Array(m).fill(2),
    labels: ['a', 'b', 'next'],
    truncated: false,
    totalNodes: n,
    totalEdges: m,
  };
}

const palette = new LabelPalette();
const detailed = () => new FakeRenderer('detailed', TIER_CAPABILITIES.detailed);
const fast = () => new FakeRenderer('fast', TIER_CAPABILITIES.fast);

test('both adapters implement the whole renderer interface; the Fast libraries are pinned with SRI', async () => {
  const { SigmaRenderer, FAST_LIBRARIES } = await import('../js/graph/sigma-renderer.js');
  const { CytoscapeRenderer } = await import('../js/graph/cytoscape-renderer.js');
  for (const Adapter of [SigmaRenderer, CytoscapeRenderer]) {
    assert.deepEqual(RENDERER_METHODS.filter((m) => typeof Adapter.prototype[m] !== 'function'), [], Adapter.name);
  }
  for (const lib of FAST_LIBRARIES) {
    assert.match(lib.src, /^https:\/\/cdnjs\.cloudflare\.com\/ajax\/libs\/[a-z.]+\/\d+\.\d+\.\d+\//, 'cdnjs, exact version');
    assert.match(lib.integrity, /^sha512-[A-Za-z0-9+/]{86}==$/);
  }
  assert.deepEqual(FAST_LIBRARIES.map((l) => l.global), ['graphology', 'Sigma']);
});

test('a tier switch keeps the positions: the new renderer gets them as a preset, the old one is destroyed', async () => {
  const a = detailed();
  const c = new GraphController({ renderer: a, engine: fakeEngine() });
  const data = chain(20);
  await c.show(data, { kind: 'data', layout: 'circle', palette, showIds: true });
  const before = a.getPositions();
  const b = fast();
  await c.replaceRenderer(() => {
    // sigma empties the shared container when killed: the old renderer goes first.
    assert.equal(a.names().at(-1), 'destroy', 'destroyed before the new one is created');
    return b;
  });
  assert.equal(a.names().at(-1), 'destroy');
  assert.deepEqual(b.names(), ['setData', 'setPositions', 'fit']);
  assert.equal(b.calls[0][1], data, 'the same data');
  assert.equal(b.calls[0][2].showIds, true, 'with the same caption options');
  assert.deepEqual([...b.calls[1][1]], [...before]);
  assert.equal(c.renderer, b);
  // And back: Fast's positions go to a new Detailed renderer.
  const a2 = detailed();
  await c.replaceRenderer(() => a2);
  assert.deepEqual(a2.names(), ['setData', 'setPositions', 'fit']);
});

test('a switch before anything was drawn runs the layout in the new renderer', async () => {
  let hidden = true;
  const a = detailed();
  const c = new GraphController({ renderer: a, engine: fakeEngine(), isHidden: () => hidden });
  await c.show(chain(5), { kind: 'data', layout: 'grid', palette });
  assert.equal(c.pending, 'grid', 'skipped while hidden: nothing drawn');
  hidden = false;
  const b = fast();
  await c.replaceRenderer(() => b);
  assert.deepEqual(b.names(), ['setData', 'setLayout', 'fit']);
  assert.equal(b.calls[1][1], 'grid');
});

test('redraw: false only swaps (new data follows), and a running engine layout lands in the new renderer', async () => {
  const a = detailed();
  const c = new GraphController({ renderer: a, engine: fakeEngine() });
  const b = fast();
  await c.replaceRenderer(() => b, { redraw: false });
  assert.deepEqual(a.names(), ['destroy']);
  assert.deepEqual(b.names(), []);

  const engine = fakeEngine(4);
  const r1 = detailed();
  const c2 = new GraphController({ renderer: r1, engine });
  const running = c2.show(chain(300), { kind: 'data', layout: 'engine', palette });
  const r2 = fast();
  await c2.replaceRenderer(() => r2);
  await running;
  assert.deepEqual(r2.names(), ['setData', 'setPositions', 'fit'], 'the layout finished into the new renderer');
  assert.ok(!r1.names().includes('setPositions'));
  assert.ok(!engine.calls.includes('cancel'), 'the layout was not restarted or cancelled');
});

test('layouts the renderer lacks run as the engine layout: cose silently, Tree/Concentric with a note', async () => {
  assert.deepEqual(layoutIn(TIER_CAPABILITIES.fast, layoutFor('cose', 10), 'cose'), { name: 'engine', note: null, iterations: 0 });
  // Above the cose cap Detailed explains the engine layout; in Fast the engine IS the force layout.
  assert.match(layoutIn(TIER_CAPABILITIES.detailed, layoutFor('cose', 5000), 'cose').note, /at most 200 vertices/);
  assert.equal(layoutIn(TIER_CAPABILITIES.fast, layoutFor('cose', 5000), 'cose').note, null);
  const tree = layoutIn(TIER_CAPABILITIES.fast, layoutFor('breadthfirst', 10), 'breadthfirst');
  assert.equal(tree.name, 'engine');
  assert.match(tree.note, /Tree is not available in the Fast tier/);
  assert.equal(layoutIn(TIER_CAPABILITIES.fast, layoutFor('circle', 10), 'circle').name, 'circle');
  assert.equal(layoutIn(TIER_CAPABILITIES.detailed, layoutFor('concentric', 10), 'concentric').name, 'concentric');

  const states = [];
  const r = fast();
  const c = new GraphController({ renderer: r, engine: fakeEngine(1), onState: (s) => states.push(s) });
  await c.show(chain(10), { kind: 'data', layout: 'concentric', palette });
  assert.deepEqual(r.names(), ['setData', 'setPositions', 'fit']);
  assert.match(states.at(-1).note, /Concentric is not available in the Fast tier/);
  await c.applyLayout('grid');
  assert.equal(r.calls.at(-2)[1], 'grid', 'a built-in layout of the Fast renderer');
});

test('a layout ended by the engine restart (Stop during a query) says so', async () => {
  const states = [];
  const engine = {
    async layoutStart() {
      const err = new Error('Stopped: the engine was restarted');
      err.name = 'StoppedError';
      throw err;
    },
  };
  const r = detailed();
  const c = new GraphController({ renderer: r, engine, onState: (s) => states.push(s) });
  await c.show(chain(300), { kind: 'data', layout: 'engine', palette });
  assert.match(states.at(-1).note, /ended with the engine restart \(Stop\)/);
  assert.equal(r.calls.find((x) => x[0] === 'setLayout')[1], 'grid');
});

test('setOptions reaches the renderer and a replacement renderer', async () => {
  const a = detailed();
  const c = new GraphController({ renderer: a, engine: fakeEngine() });
  await c.show(chain(3), { kind: 'data', layout: 'grid', palette, showIds: false });
  c.setOptions({ showIds: true });
  assert.deepEqual(a.calls.at(-1), ['setOptions', { showIds: true }]);
  const b = fast();
  await c.replaceRenderer(() => b);
  assert.equal(b.calls[0][2].showIds, true);
});

test('the toolbar Stop registry: add/remove report the count, cancelAll stops every layout', () => {
  const counts = [];
  const jobs = new LayoutJobs((n) => counts.push(n));
  let stopped = 0;
  const t1 = jobs.add(() => { stopped++; });
  const t2 = jobs.add(() => { throw new Error('gone'); });
  assert.equal(jobs.size, 2);
  assert.equal(jobs.cancelAll(), 2, 'a throwing cancel does not stop the others');
  assert.equal(stopped, 1);
  jobs.remove(t1);
  jobs.remove(t1); // twice: ignored
  jobs.remove(t2);
  assert.equal(jobs.size, 0);
  assert.deepEqual(counts, [1, 2, 1, 0]);
  assert.equal(jobs.cancelAll(), 0);
});

test('the registry with a controller: cancelling keeps the positions reached so far', async () => {
  const jobs = new LayoutJobs();
  const engine = fakeEngine(1000);
  const r = fast();
  let token = null;
  const c = new GraphController({
    renderer: r,
    engine,
    onState: (s) => {
      const running = !!(s.layout && s.layout.running);
      if (running && token === null) token = jobs.add(() => c.cancelLayout());
      if (!running && token !== null) { jobs.remove(token); token = null; }
      if (s.layout && s.layout.iterations === 2) jobs.cancelAll();
    },
  });
  await c.show(chain(400), { kind: 'data', layout: 'engine', palette });
  assert.ok(engine.calls.includes('cancel'));
  assert.deepEqual(r.names(), ['setData', 'setPositions', 'fit']);
  assert.equal(jobs.size, 0, 'unregistered when it ended');
  assert.match(c.state.note, /stopped after 2 of 1000/);
});

test('neighbourhood: breadth first in both directions, bounded by hops and size', () => {
  const v = chain(10);
  const { offsets, neighbours, edges } = adjacencyOf(v);
  assert.equal(offsets.length, 11);
  assert.deepEqual([...neighbours.slice(offsets[3], offsets[4])].sort(), [2, 4]);
  assert.deepEqual([...edges.slice(offsets[3], offsets[4])].sort(), [2, 3]);
  assert.equal(adjacencyOf(v), adjacencyOf(v), 'cached per view');
  assert.deepEqual(neighbourhood(v, [5], 1), [5, 4, 6]);
  assert.deepEqual(neighbourhood(v, [5], 2).sort((a, b) => a - b), [3, 4, 5, 6, 7]);
  assert.deepEqual(neighbourhood(v, [0], 20).length, 10);
  assert.deepEqual(neighbourhood(v, [5], 3, 4), [5, 4, 6, 3]);
  assert.deepEqual(neighbourhood(v, [99], 1), [], 'an unknown vertex');
  assert.deepEqual(neighbourhood(v, [2, 2, 7], 0), [2, 7]);
});

test('subView keeps the chosen vertices and the edges between them, re-indexed; idsOfIndices names them', () => {
  const v = chain(10);
  const s = subView(v, [5, 4, 6]);
  assert.deepEqual(s.vertexIds, ['v5', 'v4', 'v6']);
  assert.deepEqual([...s.vertexLabel], [1, 0, 0]);
  assert.deepEqual(s.vertexName, ['n5', 'n4', 'n6']);
  assert.deepEqual(s.edgeIds, ['e4', 'e5']);
  assert.deepEqual([...s.edgeSource], [1, 0]);
  assert.deepEqual([...s.edgeTarget], [0, 2]);
  assert.deepEqual([...s.of], [5, 4, 6]);
  assert.equal(s.labels, v.labels);
  assert.equal(s.truncated, false);
  assert.deepEqual(idsOfIndices(v, [5, 4, 6]), { nodes: ['v5', 'v4', 'v6'], edges: ['e4', 'e5'] });
});

test('capToElements keeps the longest vertex prefix whose elements fit', () => {
  const v = chain(10); // 10 + 9 = 19 elements
  assert.equal(capToElements(v, 19), v);
  const c = capToElements(v, 10);
  assert.equal(c.vertexIds.length, 5, '5 vertices + 4 edges = 9; 6 + 5 = 11 does not fit');
  assert.equal(elementCount(c), 9);
  assert.equal(c.truncated, true);
  assert.equal(capToElements(null, 5), null);
});

test('built-in positions of the Fast tier: circle and grid, one x/y pair per vertex', () => {
  const circle = circlePositions(8, 40);
  assert.equal(circle.length, 16);
  const r = Math.hypot(circle[0], circle[1]);
  for (let i = 0; i < 8; i++) assert.ok(Math.abs(Math.hypot(circle[2 * i], circle[2 * i + 1]) - r) < 1e-3);
  assert.deepEqual([...circlePositions(1)], [0, 0]);
  assert.deepEqual([...gridPositions(5, 10)], [0, 0, 10, 0, 20, 0, 0, 10, 10, 10]);
  assert.equal(gridPositions(0).length, 0);
});
