// Node tests of the renderer interface (js/graph/renderer.js), the graph controller
// (js/graph/controller.js) with a FAKE renderer, and the engine layout client
// (js/graph/layout-client.js) with a fake engine: the panel's behaviour without a browser.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { RENDERER_METHODS, CAPABILITY_KEYS, rendererProblems } from '../js/graph/renderer.js';
import { GraphController, nodeCountOf } from '../js/graph/controller.js';
import { runEngineLayout, layoutInputOf, LayoutUnavailableError } from '../js/graph/layout-client.js';
import { LabelPalette } from '../js/graph/palette.js';
import { withPositions } from '../js/engine/http-engine.js';
import { DRAW_TUNING } from '../js/graph-data.js';

/** A renderer that records every call (the interface contract, nothing drawn). */
class FakeRenderer {
  constructor() {
    this.capabilities = { tier: 'fake', edgeLabels: false, curvedEdges: false, arrows: true, layouts: ['grid', 'circle', 'cose'], png: false, svg: false, schema: true, addElements: false };
    this.calls = [];
    this.positions = null;
    this.handlers = {};
  }
  record(name, ...args) { this.calls.push([name, ...args]); }
  names() { return this.calls.map((c) => c[0]); }
  setData(data, options) { this.record('setData', data, options); this.positions = null; }
  setPositions(xy) { this.record('setPositions', xy); this.positions = xy; }
  getPositions() { return this.positions; }
  async setLayout(name, options) { this.record('setLayout', name, options); }
  setOptions(options) { this.record('setOptions', options); }
  fit() { this.record('fit'); }
  resize() { this.record('resize'); }
  select(ids) { this.record('select', ids); }
  onClick(h) { this.handlers.click = h; }
  onDoubleClick(h) { this.handlers.dblclick = h; }
  onUserMove(h) { this.handlers.move = h; }
  setEditMode(on) { this.record('setEditMode', on); }
  onConnect(h) { this.handlers.connect = h; }
  async exportPng() { throw new Error('no png'); }
  setTheme(dark) { this.record('setTheme', dark); }
  destroy() { this.record('destroy'); }
}

/**
 * An engine with the layout methods: every job runs `slices` slices; positions are `i, -i` per
 * vertex; `layoutCancel` answers the positions so far. Records the calls.
 */
function fakeEngine({ slices = 3, cached = false } = {}) {
  const jobs = new Map();
  const calls = [];
  let next = 0;
  const status = (job) => {
    const j = jobs.get(job);
    const out = { job, state: j.state, iterations: j.done, totalIterations: slices, progress: j.done / slices, elapsedMs: j.done, vertexCount: j.n, cached: j.cached };
    if (j.state !== 'running') out.positions = Float32Array.from({ length: 2 * j.n }, (_, k) => (k % 2 ? -(k >> 1) : k >> 1));
    return out;
  };
  return {
    calls,
    async layoutStart(input, options) {
      calls.push(['layoutStart', input, options]);
      const job = ++next;
      jobs.set(job, { n: input.vertexCount, done: cached ? slices : 1, state: cached ? 'done' : (slices <= 1 ? 'done' : 'running'), cached });
      return status(job);
    },
    async layoutStep(job, budgetMs) {
      calls.push(['layoutStep', job, budgetMs]);
      const j = jobs.get(job);
      if (!j) throw new Error(`no layout job ${job}`);
      if (j.state === 'running' && ++j.done >= slices) j.state = 'done';
      return status(job);
    },
    async layoutCancel(job) {
      calls.push(['layoutCancel', job]);
      const j = jobs.get(job);
      if (j.state === 'running') j.state = 'cancelled';
      return status(job);
    },
  };
}

/** A compact view of `n` vertices in a chain. */
function chain(n) {
  return {
    vertexIds: Array.from({ length: n }, (_, i) => String(i)),
    vertexLabel: new Uint32Array(n),
    vertexName: new Array(n).fill(null),
    edgeIds: Array.from({ length: Math.max(0, n - 1) }, (_, j) => `e${j}`),
    edgeSource: Uint32Array.from({ length: Math.max(0, n - 1) }, (_, j) => j),
    edgeTarget: Uint32Array.from({ length: Math.max(0, n - 1) }, (_, j) => j + 1),
    edgeLabel: new Uint32Array(Math.max(0, n - 1)).fill(1),
    labels: ['v', 'next'],
    truncated: false,
    totalNodes: n,
    totalEdges: Math.max(0, n - 1),
  };
}

const palette = new LabelPalette();

test('the contract: every method and capability, checked by rendererProblems', () => {
  assert.deepEqual(rendererProblems(new FakeRenderer()), []);
  assert.ok(RENDERER_METHODS.includes('setPositions') && RENDERER_METHODS.includes('capabilities') === false);
  const partial = new FakeRenderer();
  partial.exportPng = undefined;
  delete partial.capabilities.svg;
  assert.deepEqual(rendererProblems(partial), ['method exportPng', 'capability svg']);
  assert.deepEqual(rendererProblems(null), ['not an object']);
  assert.ok(CAPABILITY_KEYS.includes('layouts'));
});

test('a small drawing uses the renderer\'s built-in layout and fits', async () => {
  const r = new FakeRenderer();
  const engine = fakeEngine();
  const states = [];
  const c = new GraphController({ renderer: r, engine, onState: (s) => states.push(s) });
  await c.show(chain(10), { kind: 'data', layout: 'cose', palette });
  assert.deepEqual(r.names(), ['setData', 'setLayout', 'fit']);
  assert.equal(r.calls[1][1], 'cose');
  assert.equal(r.calls[1][2].iterations, 1000);
  assert.equal(engine.calls.length, 0, 'no engine layout for a small drawing');
  assert.equal(states.at(-1).note, null);
});

test('above the cose cap the engine lays out in slices with progress, then setPositions', async () => {
  const r = new FakeRenderer();
  const engine = fakeEngine({ slices: 4 });
  const states = [];
  const n = DRAW_TUNING.coseMaxNodes + 1;
  const c = new GraphController({ renderer: r, engine, budgetMs: 20, onState: (s) => states.push(s) });
  await c.show(chain(n), { kind: 'data', layout: 'cose', palette });
  assert.deepEqual(r.names(), ['setData', 'setPositions', 'fit']);
  const xy = r.calls[1][1];
  assert.ok(xy instanceof Float32Array && xy.length === 2 * n);
  assert.deepEqual(engine.calls.map((x) => x[0]), ['layoutStart', 'layoutStep', 'layoutStep', 'layoutStep']);
  assert.equal(engine.calls[0][1].vertexCount, n);
  assert.equal(engine.calls[0][2].budgetMs, 20);
  assert.equal(engine.calls[1][2], 20, 'every slice gets the budget');
  const running = states.filter((s) => s.layout && s.layout.running);
  assert.ok(running.length >= 3, 'progress reported per slice');
  assert.ok(running.every((s, i) => i === 0 || s.layout.progress >= running[i - 1].layout.progress));
  assert.equal(running.at(-1).layout.vertexCount, n);
  assert.equal(states.at(-1).layout, null, 'finished');
  assert.match(states.at(-1).note, /engine's force layout/);
});

test('"engine" lays out even a small drawing; the schema view gets its topology from node ids', async () => {
  const r = new FakeRenderer();
  const engine = fakeEngine({ slices: 1 });
  const c = new GraphController({ renderer: r, engine });
  const schema = { nodes: [{ id: 'L:a', label: 'a' }, { id: 'L:b', label: 'b' }], edges: [{ id: 'E', source: 'L:a', target: 'L:b' }, { id: 'X', source: 'L:a', target: 'L:*' }] };
  await c.show(schema, { kind: 'schema', layout: 'engine', palette });
  assert.deepEqual(r.names(), ['setData', 'setPositions', 'fit']);
  const input = engine.calls[0][1];
  assert.equal(input.vertexCount, 2);
  assert.deepEqual([...input.edgeSource, ...input.edgeTarget], [0, 1], 'the edge to an undrawn node is left out');
  assert.equal(nodeCountOf(schema, 'schema'), 2);
});

test('Stop cancels the engine layout and draws the positions reached so far', async () => {
  const r = new FakeRenderer();
  const engine = fakeEngine({ slices: 1000 });
  const states = [];
  const c = new GraphController({
    renderer: r,
    engine,
    onState: (s) => {
      states.push(s);
      if (s.layout && s.layout.iterations === 3) c.cancelLayout();
    },
  });
  await c.show(chain(500), { kind: 'data', layout: 'engine', palette });
  assert.ok(engine.calls.some((x) => x[0] === 'layoutCancel'));
  assert.deepEqual(r.names(), ['setData', 'setPositions', 'fit']);
  assert.match(states.at(-1).note, /stopped after 3 of 1000 iterations/i);
});

test('a newer drawing supersedes a running layout: the late positions are dropped', async () => {
  const r = new FakeRenderer();
  const engine = fakeEngine({ slices: 50 });
  const c = new GraphController({ renderer: r, engine });
  const first = c.show(chain(400), { kind: 'data', layout: 'engine', palette });
  const second = c.show(chain(5), { kind: 'data', layout: 'grid', palette });
  await Promise.all([first, second]);
  // Only the second drawing is laid out; the first was cancelled in the engine.
  assert.deepEqual(r.names(), ['setData', 'setData', 'setLayout', 'fit']);
  assert.ok(engine.calls.some((x) => x[0] === 'layoutCancel'));
});

test('switching layouts and re-layout keep working; a cached engine layout answers at once', async () => {
  const r = new FakeRenderer();
  const engine = fakeEngine({ slices: 2, cached: true });
  const c = new GraphController({ renderer: r, engine });
  await c.show(chain(300), { kind: 'data', layout: 'circle', palette });
  await c.applyLayout('engine');
  await c.applyLayout('engine'); // re-layout
  await c.applyLayout('grid');
  assert.deepEqual(r.names(), ['setData', 'setLayout', 'fit', 'setPositions', 'fit', 'setPositions', 'fit', 'setLayout', 'fit']);
  assert.deepEqual(engine.calls.map((x) => x[0]), ['layoutStart', 'layoutStart'], 'no slices: cached');
});

test('a hidden canvas defers a built-in layout until resume(); an empty drawing draws nothing', async () => {
  const r = new FakeRenderer();
  let hidden = true;
  const c = new GraphController({ renderer: r, engine: fakeEngine(), isHidden: () => hidden });
  await c.show(chain(3), { kind: 'data', layout: 'circle', palette });
  assert.deepEqual(r.names(), ['setData']);
  assert.equal(c.pending, 'circle');
  hidden = false;
  await c.resume();
  assert.deepEqual(r.names(), ['setData', 'setLayout', 'fit']);
  assert.equal(c.resume(), null, 'nothing pending any more');
  await c.show(null, { kind: 'data', layout: 'cose', palette });
  assert.deepEqual(r.calls.at(-1)[0], 'setPositions');
  assert.equal(r.calls.at(-1)[1].length, 0);
});

test('an engine without the layout (an older worker) falls back to a grid with a note', async () => {
  const r = new FakeRenderer();
  const states = [];
  const c = new GraphController({ renderer: r, engine: {}, onState: (s) => states.push(s) });
  await c.show(chain(300), { kind: 'data', layout: 'cose', palette });
  assert.deepEqual(r.names(), ['setData', 'setLayout', 'fit']);
  assert.equal(r.calls[1][1], 'grid');
  assert.match(states.at(-1).note, /failed .*grid/);
  await assert.rejects(runEngineLayout(null, layoutInputOf(chain(2))), LayoutUnavailableError);
});

test('destroy abandons a running layout and destroys the renderer', async () => {
  const r = new FakeRenderer();
  const engine = fakeEngine({ slices: 100 });
  const c = new GraphController({ renderer: r, engine });
  const running = c.show(chain(300), { kind: 'data', layout: 'engine', palette });
  c.destroy();
  await running;
  assert.deepEqual(r.names(), ['setData', 'destroy']);
});

test('layoutInputOf copies the index columns; the HTTP engine turns positions into a Float32Array', () => {
  const v = chain(4);
  const input = layoutInputOf(v);
  assert.equal(input.vertexCount, 4);
  assert.deepEqual([...input.edgeSource], [0, 1, 2]);
  assert.notEqual(input.edgeSource, v.edgeSource, 'a copy: transferring it leaves the view intact');
  assert.equal(layoutInputOf(null).vertexCount, 0);
  const status = withPositions({ job: 1, state: 'done', positions: [1.5, -2, 3, 4] });
  assert.ok(status.positions instanceof Float32Array);
  assert.deepEqual([...status.positions], [1.5, -2, 3, 4]);
  assert.equal(withPositions({ job: 1, state: 'running' }).positions, undefined);
});

test('positions with keepView: drawn in place, no layout and no fit (an edit made in edit mode)', async () => {
  const r = new FakeRenderer();
  const c = new GraphController({ renderer: r, engine: fakeEngine() });
  const xy = new Float32Array(2 * 4);
  await c.show(chain(4), { kind: 'data', layout: 'cose', palette, positions: xy, keepView: true });
  assert.deepEqual(r.names(), ['setData', 'setPositions']);
  await c.show(chain(4), { kind: 'data', layout: 'cose', palette, positions: xy });
  assert.deepEqual(r.names().slice(2), ['setData', 'setPositions', 'fit']);
});
