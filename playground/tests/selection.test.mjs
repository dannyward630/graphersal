// Node tests of the Graph panel's view selector (js/graph/selection.js): the whole-graph plan,
// the label meta-graph drawing and its sample requests, neighbourhood and expansion requests,
// positions that survive an expansion, the notices; and the controller drawing a view at given
// positions without a layout (js/graph/controller.js).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  VIEW_SOURCES, SELECTION_TUNING, normalizeSource, wholeGraphPlan, labelName, metaScale, metaGraphDrawing,
  sampleRequestFor, metaNotice, DIRECTIONS, normalizeDirection, normalizeDepth, neighbourhoodRequest, expandRequest, carryPositions, seedCaption, selectionNotice,
  resultNotice,
} from '../js/graph/selection.js';
import { GraphController } from '../js/graph/controller.js';
import { TIER_CAPABILITIES } from '../js/graph/renderer.js';
import { TIER_TUNING } from '../js/graph/tiers.js';

/** A compact view of `ids` (labels by index into `labels`) with edges `[s, t]` by vertex index. */
function view(ids, edges = [], extra = {}) {
  return {
    vertexIds: ids.map(String),
    vertexLabel: Uint32Array.from(ids, () => 0),
    vertexName: ids.map((id) => `v${id}`),
    edgeIds: edges.map((_, j) => `e${j}`),
    edgeSource: Uint32Array.from(edges, (e) => e[0]),
    edgeTarget: Uint32Array.from(edges, (e) => e[1]),
    edgeLabel: Uint32Array.from(edges, () => 1),
    labels: ['person', 'knows'],
    truncated: false,
    totalNodes: ids.length,
    totalEdges: edges.length,
    ...extra,
  };
}

const META = {
  labels: [{ label: 'person', count: 1000 }, { label: 'software', count: 10 }, { label: '', count: 1 }],
  other: { labels: 5, count: 50 },
  edges: [
    { label: 'created', from: 0, to: 1, count: 400 },
    { label: 'knows', from: 0, to: 0, count: 300 },
    { label: 'likes', from: 3, to: 0, count: 2 },
  ],
  otherPairs: 2,
  otherEdges: 7,
  labelTotal: 8,
  vertexCount: 1061,
  edgeCount: 709,
  scannedEdges: 709,
  complete: true,
};
const LIMITS = { maxNodes: 10000, maxEdges: 20000 };

test('sources: four tabs; an unknown stored source is the whole graph', () => {
  assert.deepEqual(VIEW_SOURCES.map((s) => s.id), ['graph', 'labels', 'neighbourhood', 'result']);
  assert.ok(VIEW_SOURCES.every((s) => s.title));
  assert.equal(normalizeSource('labels'), 'labels');
  assert.equal(normalizeSource('nonsense'), 'graph');
  assert.equal(normalizeSource(undefined), 'graph');
});

test('whole graph: drawn when it fits, else the labels, the first-N slice only on request', () => {
  assert.equal(wholeGraphPlan({ vertexCount: 10, edgeCount: 20, maxNodes: 10, maxEdges: 20 }).mode, 'whole');
  const big = wholeGraphPlan({ vertexCount: 111110, edgeCount: 111100, maxNodes: 10000, maxEdges: 20000 });
  assert.equal(big.mode, 'overview');
  assert.match(big.notice, /111,110 vertices and 111,100 edges/);
  assert.match(big.notice, /labels are shown instead/);
  // Too many edges alone is too large as well.
  assert.equal(wholeGraphPlan({ vertexCount: 5, edgeCount: 21, maxNodes: 10, maxEdges: 20 }).mode, 'overview');
  const slice = wholeGraphPlan({ vertexCount: 111110, edgeCount: 111100, maxNodes: 10000, maxEdges: 20000, sliceRequested: true });
  assert.equal(slice.mode, 'slice');
  assert.match(slice.notice, /first 10,000 vertices in storage order/);
  assert.match(slice.notice, /arbitrary slice/);
});

test('meta-graph drawing: label nodes sized by count, the other labels last, pairs as edges', () => {
  const d = metaGraphDrawing(META);
  assert.deepEqual(d.nodes.map((n) => n.label), ['person', 'software', '(no label)', 'other labels (5)']);
  assert.deepEqual(d.nodes.map((n) => n.count), [1000, 10, 1, 50]);
  assert.equal(d.nodes[0].scale, metaScale(1000, 1000));
  assert.ok(d.nodes[0].scale > d.nodes[1].scale && d.nodes[1].scale > d.nodes[2].scale);
  assert.equal(d.nodes[2].scale, 1);
  assert.deepEqual(d.edges.map((e) => [e.source, e.target, e.count]), [['L0', 'L1', 400], ['L0', 'L0', 300], ['L3', 'L0', 2]]);
  assert.ok(d.nodes.every((n) => n.pick) && d.edges.every((e) => e.pick));
  assert.equal(metaGraphDrawing(null), null);
  assert.equal(labelName(''), '(no label)');
  assert.equal(metaScale(5, 1), 1);
  assert.ok(metaScale(1000, 1000) <= 2.2 + 1e-9);
});

test('meta-graph double click: a sample of a label, the other labels, or a label pair', () => {
  const d = metaGraphDrawing(META);
  const label = sampleRequestFor(META, d.nodes[0].pick, LIMITS);
  assert.deepEqual(label.request, { vertices: { labels: ['person'] }, maxNodes: SELECTION_TUNING.sampleNodes, maxEdges: 20000 });
  assert.equal(label.total, 1000);
  assert.equal(label.title, 'person');
  const other = sampleRequestFor(META, d.nodes[3].pick, LIMITS);
  assert.deepEqual(other.request.vertices, { labels: ['person', 'software', ''], exclude: true });
  assert.equal(other.total, 50);
  const pair = sampleRequestFor(META, d.edges[0].pick, { maxNodes: 300, maxEdges: 600 });
  assert.deepEqual(pair.request, {
    edge: { label: 'created', from: { labels: ['person'] }, to: { labels: ['software'] } }, maxNodes: 300, maxEdges: 600,
  });
  assert.equal(pair.title, 'person –created→ software');
  const fromOther = sampleRequestFor(META, d.edges[2].pick, LIMITS);
  assert.equal(fromOther.request.edge.from.exclude, true);
  assert.equal(fromOther.title, 'other labels –likes→ person');
  assert.equal(sampleRequestFor(META, { kind: 'pair', index: 99 }, LIMITS), null);
  assert.equal(sampleRequestFor(null, d.nodes[0].pick, LIMITS), null);
});

test('meta-graph notice: what is grouped, what is not drawn, how to drill down', () => {
  const text = metaNotice(META);
  assert.match(text, /8 vertex labels of 1,061 vertices \(the 3 largest shown, the other 5 as one node\)/);
  assert.match(text, /5 label pairs of 709 edges \(the 3 largest drawn; 7 edges in 2 smaller pairs are not\)/);
  assert.match(text, /Double-click a label or an edge/);
  assert.doesNotMatch(text, /first/);
  assert.match(metaNotice({ ...META, complete: false, scannedEdges: 5000000 }), /from the first 5,000,000 edges/);
  assert.equal(metaNotice(null), null);
});

test('neighbourhood and expansion requests carry depth, direction and limits; expansion keeps the drawing', () => {
  assert.deepEqual(neighbourhoodRequest([1], 2, LIMITS), { ids: ['1'], hops: 2, direction: 'both', maxNodes: 10000, maxEdges: 20000 });
  assert.equal(neighbourhoodRequest([1], 3, LIMITS, 'in').direction, 'in');
  assert.equal(neighbourhoodRequest([1], 3, LIMITS, 'sideways').direction, 'both');
  const v = view([1, 2, 3], [[0, 1]]);
  assert.deepEqual(expandRequest(v, 2, LIMITS, 'out'), {
    ids: ['3'], hops: 1, direction: 'out', keep: ['1', '2', '3'], maxNodes: 10000, maxEdges: 20000,
  });
});

test('depth and direction controls: bounded depth 1-5, three directions', () => {
  assert.equal(SELECTION_TUNING.maxHops, 5);
  assert.deepEqual(DIRECTIONS.map((d) => d.id), ['both', 'out', 'in']);
  assert.ok(DIRECTIONS.every((d) => d.title));
  assert.equal(normalizeDirection('out'), 'out');
  assert.equal(normalizeDirection(undefined), 'both');
  assert.equal(normalizeDepth(3), 3);
  assert.equal(normalizeDepth('5'), 5);
  for (const bad of [0, 6, 2.5, 'x', null]) assert.equal(normalizeDepth(bad), 1, String(bad));
});

test('carryPositions: kept vertices stay, new ones ring the expanded vertex', () => {
  const prev = view([1, 2]);
  const xy = Float32Array.from([0, 0, 100, 0]);
  const next = view([1, 2, 3, 4]);
  const out = carryPositions(prev, xy, next, '2');
  assert.deepEqual(Array.from(out.slice(0, 4)), [0, 0, 100, 0]);
  for (const i of [2, 3]) {
    const d = Math.hypot(out[2 * i] - 100, out[2 * i + 1]);
    assert.ok(Math.abs(d - 70) < 1e-3, `vertex ${i} at ${d} from the anchor`);
  }
  // Without an anchor: around the centre of the kept vertices.
  const centred = carryPositions(prev, xy, next);
  assert.ok(Math.abs(Math.hypot(centred[4] - 50, centred[5]) - 70) < 1e-3);
  // Many new vertices: further rings, all finite.
  const many = view(Array.from({ length: 40 }, (_, i) => i + 1));
  const ringed = carryPositions(prev, xy, many, '1');
  assert.ok(Array.from(ringed).every(Number.isFinite));
  assert.ok(Math.hypot(ringed[78], ringed[79]) > 70);
  // Nothing to keep: null (the drawing is laid out instead).
  assert.equal(carryPositions(prev, null, next), null);
  assert.equal(carryPositions(prev, Float32Array.from([0, 0]), next), null);
  assert.equal(carryPositions(view([9]), Float32Array.from([0, 0]), next), null);
});

test('notices: a neighbourhood, an expansion, a sample, limits and missing ids', () => {
  const n = view([1, 2, 3], [[0, 1], [0, 2]], { seeds: [0], hiddenEdges: 4, missing: [] });
  assert.equal(seedCaption(n), 'v1');
  assert.equal(selectionNotice(n, { hops: 1 }),
    'Neighbourhood of v1: 3 vertices and 2 edges within 1 hop. 4 more edges lead out of the drawing: double-click a vertex to expand it.');
  assert.match(selectionNotice({ ...n, hiddenEdges: 1 }, { expanded: 2, hops: 2 }), /\(depth 2, expanded 2 times\)\. 1 more edge leads out/);
  assert.match(selectionNotice(n, { hops: 3, direction: 'out' }), /within 3 hops along outgoing edges\./);
  assert.match(selectionNotice(n, { hops: 1, direction: 'in' }), /within 1 hop along incoming edges\./);
  // The limits cut it: how many vertices and edges were left out.
  const cut = { ...n, truncated: true, leftOut: { vertices: 1234, edges: 5, complete: true } };
  assert.match(selectionNotice(cut, { hops: 2, maxNodes: 3, maxEdges: 2 }),
    /The limits \(3 vertices, 2 edges; Settings\) left out 1,234 vertices and 5 edges within this depth\./);
  assert.match(selectionNotice({ ...cut, leftOut: { vertices: 1000000, edges: 0, complete: false } }, {}), /left out at least 1,000,000 vertices within/);
  assert.match(selectionNotice({ ...n, hiddenEdges: 0 }, {}), /Nothing more is connected to it\./);
  assert.match(selectionNotice({ ...n, truncated: true }, {}), /Stopped at the limit \(3 vertices/);
  assert.match(selectionNotice({ ...n, missing: ['a', 'b', 'c', 'd'] }, {}), /Not found: a, b, c, …\./);
  const sample = { ...n, seeds: [], sample: { kind: 'vertices', shown: 3, total: 500, complete: true } };
  assert.match(selectionNotice(sample, { sampleTitle: 'person' }), /^A sample of person: the first 3 of 500 vertices in storage order \(not random\)/);
  const all = { ...sample, sample: { kind: 'edges', shown: 2, total: 2, complete: true } };
  assert.match(selectionNotice(all, { sampleTitle: 'a –x→ b' }), /^All 2 edges of a –x→ b: 3 vertices and 2 edges\./);
  assert.match(selectionNotice(n, { sampleTitle: 'person', expanded: 1 }), /^A sample of person, expanded 1 time: 3 vertices/);
  assert.equal(selectionNotice(null), null);
});

test('result notice: says which limit stopped the subgraph', () => {
  assert.equal(resultNotice(view([1]), 50000), null);
  const cut = view([1, 2], [[0, 1]], { truncated: true, stoppedBy: 'elements' });
  assert.match(resultNotice(cut, 3), /first 2 vertices and 1 edge the result refers to \(the "Result graph" limit of 3/);
  assert.match(resultNotice(cut, 3), new RegExp(TIER_TUNING.fastMax.toLocaleString('en-US')));
  assert.match(resultNotice({ ...cut, stoppedBy: 'bytes' }, 3), /size bound/);
});

test('controller: a view with positions is drawn there, without a layout', async () => {
  const calls = [];
  const renderer = {
    capabilities: TIER_CAPABILITIES.detailed,
    setData: (d) => calls.push(['setData', d.vertexIds.length]),
    setPositions: (xy) => calls.push(['setPositions', Array.from(xy)]),
    getPositions: () => null,
    setLayout: async (name) => calls.push(['setLayout', name]),
    fit: () => calls.push(['fit']),
    setOptions() {}, select() {}, destroy() {},
  };
  const states = [];
  const c = new GraphController({ renderer, onState: (s) => states.push(s) });
  await c.show(view([1, 2]), { layout: 'cose', positions: Float32Array.from([1, 2, 3, 4]) });
  assert.deepEqual(calls, [['setData', 2], ['setPositions', [1, 2, 3, 4]], ['fit']]);
  assert.equal(states.at(-1).layout, null);
  assert.ok(!('positions' in c.options), 'positions are not kept for a later renderer');
  // Positions of the wrong length: laid out as usual.
  calls.length = 0;
  await c.show(view([1, 2, 3]), { layout: 'cose', positions: Float32Array.from([1, 2]) });
  assert.deepEqual(calls.map((x) => x[0]), ['setData', 'setLayout', 'fit']);
});
