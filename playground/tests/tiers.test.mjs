// Node tests of the tier model (js/graph/tiers.js): thresholds, hysteresis, the manual switch,
// the no-WebGL fallback, confirmations, and capabilities -> banner and disabled controls.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  TIER_TUNING, TIER_CHOICES, chooseTier, normalizeChoice, missingFeatures, controlStates, tierBanner, detectWebGL,
} from '../js/graph/tiers.js';
import { TIER_CAPABILITIES, TIERS, CAPABILITY_KEYS } from '../js/graph/renderer.js';

const T = TIER_TUNING;
const LAYOUT_IDS = ['cose', 'engine', 'circle', 'concentric', 'breadthfirst', 'grid'];

test('the thresholds are ordered: hysteresis band below the Fast threshold, confirmations above', () => {
  assert.ok(T.detailedBelow < T.fastFrom);
  assert.ok(T.fastFrom < T.detailedConfirm);
  assert.ok(T.detailedConfirm < T.noWebglMax && T.noWebglMax < T.fastMax);
  assert.deepEqual(TIER_CHOICES.map((c) => c.id), ['auto', 'detailed', 'fast']);
});

test('auto: Detailed below the threshold, Fast from it', () => {
  assert.equal(chooseTier({ size: 0 }).tier, 'detailed');
  assert.equal(chooseTier({ size: T.fastFrom - 1 }).tier, 'detailed');
  const fast = chooseTier({ size: T.fastFrom });
  assert.equal(fast.tier, 'fast');
  assert.equal(fast.why, 'auto');
  assert.equal(fast.confirm, null, 'no confirmation inside the Fast range');
  assert.equal(chooseTier({ size: T.fastMax }).confirm, null);
  assert.equal(fast.warning, null);
});

test('auto has hysteresis: once Fast, Detailed again only below the lower threshold', () => {
  const band = Math.floor((T.detailedBelow + T.fastFrom) / 2);
  assert.equal(chooseTier({ size: band, previous: 'detailed' }).tier, 'detailed', 'growing into the band stays Detailed');
  assert.equal(chooseTier({ size: band, previous: 'fast' }).tier, 'fast', 'shrinking into the band stays Fast');
  assert.equal(chooseTier({ size: T.detailedBelow, previous: 'fast' }).tier, 'fast');
  assert.equal(chooseTier({ size: T.detailedBelow - 1, previous: 'fast' }).tier, 'detailed');
  // A sequence around the threshold flips once each way, not on every change.
  let previous = null;
  const tiers = [];
  const sizes = [T.detailedBelow - 500, T.fastFrom - 1, T.fastFrom, T.fastFrom - 100, band, T.detailedBelow, T.detailedBelow - 1, T.fastFrom - 1, T.fastFrom + 100];
  for (const size of sizes) {
    previous = chooseTier({ size, previous }).tier;
    tiers.push(previous);
  }
  assert.deepEqual(tiers, ['detailed', 'detailed', 'fast', 'fast', 'fast', 'fast', 'detailed', 'detailed', 'fast']);
});

test('the manual switch overrides the size; forcing Detailed above the threshold warns and asks', () => {
  const small = chooseTier({ size: 10, choice: 'fast' });
  assert.equal(small.tier, 'fast');
  assert.equal(small.why, 'choice');
  const forced = chooseTier({ size: T.fastFrom, choice: 'detailed' });
  assert.equal(forced.tier, 'detailed');
  assert.match(forced.warning, /Detailed is forced/);
  assert.equal(forced.confirm, null, 'a warning only, up to the confirmation size');
  const big = chooseTier({ size: T.detailedConfirm + 1, choice: 'detailed' });
  assert.equal(big.confirm.size, T.detailedConfirm + 1);
  assert.match(big.confirm.reason, /forced/);
  assert.equal(chooseTier({ size: T.fastFrom - 1, choice: 'detailed' }).warning, null);
  assert.equal(chooseTier({ size: 50, choice: 'detailed', previous: 'fast' }).tier, 'detailed');
});

test('above the Fast range even Fast asks first', () => {
  const d = chooseTier({ size: T.fastMax + 1 });
  assert.equal(d.tier, 'fast');
  assert.equal(d.confirm.size, T.fastMax + 1);
  assert.match(d.confirm.reason, /range/);
  assert.equal(chooseTier({ size: T.fastMax + 1, choice: 'fast' }).confirm.size, T.fastMax + 1);
});

test('no WebGL: Detailed with a notice, a hard limit and a confirmation above the Detailed size', () => {
  const small = chooseTier({ size: 100, webgl: false, choice: 'fast' });
  assert.equal(small.tier, 'detailed');
  assert.equal(small.why, 'no-fast');
  assert.match(small.notice, /no WebGL/);
  assert.equal(small.cap, null);
  assert.equal(small.confirm, null);
  const mid = chooseTier({ size: T.detailedConfirm + 1, webgl: false });
  assert.equal(mid.cap, null);
  assert.equal(mid.confirm.size, T.detailedConfirm + 1);
  const huge = chooseTier({ size: 100000, webgl: false });
  assert.equal(huge.cap, T.noWebglMax);
  assert.equal(huge.confirm.size, T.noWebglMax, 'what is drawn after the cut');
  assert.match(huge.confirm.reason, /only the first 10,000 vertices and edges are drawn/);
  assert.doesNotMatch(mid.confirm.reason, /only the first/);
  assert.match(huge.notice, /at most 10,000/);
  // The Fast renderer failing to start (offline) falls back the same way, naming the cause.
  const offline = chooseTier({ size: 50000, fastError: 'could not load sigma.min.js' });
  assert.equal(offline.tier, 'detailed');
  assert.match(offline.notice, /could not start \(could not load sigma\.min\.js\)/);
});

test('the schema drawing is always Detailed; unknown choices count as auto', () => {
  assert.deepEqual(chooseTier({ size: 1e6, kind: 'schema', choice: 'fast' }), {
    tier: 'detailed', why: 'schema', warning: null, notice: null, confirm: null, cap: null,
  });
  assert.equal(normalizeChoice('fast'), 'fast');
  assert.equal(normalizeChoice('webgl'), 'auto');
  assert.equal(normalizeChoice(undefined), 'auto');
  assert.equal(chooseTier({ size: T.fastFrom, choice: 'bogus' }).tier, 'fast');
});

test('every tier declares every capability; Fast draws no schema and has only the cheap layouts', () => {
  assert.deepEqual(TIERS, ['detailed', 'fast']);
  for (const tier of TIERS) {
    for (const key of CAPABILITY_KEYS) assert.ok(key in TIER_CAPABILITIES[tier], `${tier}.${key}`);
    assert.equal(TIER_CAPABILITIES[tier].tier, tier);
  }
  assert.equal(TIER_CAPABILITIES.fast.schema, false);
  assert.equal(TIER_CAPABILITIES.fast.edgeLabels, false);
  assert.deepEqual([...TIER_CAPABILITIES.fast.layouts], ['circle', 'grid']);
  assert.ok(TIER_CAPABILITIES.fast.png && TIER_CAPABILITIES.detailed.png, 'PNG in both tiers');
});

test('the banner lists what Fast lacks; Detailed lacks nothing', () => {
  assert.deepEqual(missingFeatures(TIER_CAPABILITIES.detailed), []);
  const lacks = missingFeatures(TIER_CAPABILITIES.fast);
  assert.deepEqual(lacks, ['edge labels (hover a vertex to see the labels of its edges)', 'curved edges and loops', 'Concentric or Tree layout']);
  assert.equal(tierBanner(TIER_CAPABILITIES.detailed), null);
  assert.equal(
    tierBanner(TIER_CAPABILITIES.fast, { why: 'auto', size: 150000 }),
    'Fast tier (WebGL) for 150,000 vertices and edges: no edge labels (hover a vertex to see the labels of its edges), no curved edges and loops, no Concentric or Tree layout.',
  );
  assert.match(tierBanner(TIER_CAPABILITIES.fast, { why: 'choice' }), /^Fast tier \(WebGL\): no edge labels/);
  // A renderer that also lacked arrows and SVG (against a reference that has them) says so.
  const ref = { ...TIER_CAPABILITIES.detailed, svg: true };
  assert.deepEqual(missingFeatures({ ...TIER_CAPABILITIES.fast, arrows: false }, ref).slice(2), ['arrow heads', 'Concentric or Tree layout', 'SVG export']);
});

test('capabilities -> disabled controls with the reason as tooltip', () => {
  const detailed = controlStates(TIER_CAPABILITIES.detailed, LAYOUT_IDS);
  assert.equal(detailed.edgeLabels.disabled, false);
  assert.equal(detailed.edgeIds.disabled, false);
  assert.ok(LAYOUT_IDS.every((id) => detailed.layouts[id].disabled === false));
  assert.equal(detailed.svg.disabled, true, 'no SVG export exists yet');
  assert.match(detailed.svg.reason, /no SVG export yet/);

  const fast = controlStates(TIER_CAPABILITIES.fast, LAYOUT_IDS);
  assert.equal(fast.edgeLabels.disabled, true);
  assert.match(fast.edgeLabels.reason, /Fast tier draws no edge labels: hover a vertex/);
  assert.deepEqual(fast.edgeIds, fast.edgeLabels);
  assert.deepEqual(LAYOUT_IDS.filter((id) => fast.layouts[id].disabled), ['concentric', 'breadthfirst']);
  assert.match(fast.layouts.breadthfirst.reason, /no Tree layout: switch to Detailed/);
  assert.equal(fast.layouts.cose.disabled, false, 'Force stays: in Fast it is the engine layout');
  assert.match(fast.svg.reason, /only available in the Detailed tier/);
});

test('detectWebGL: false without a document, true when a context exists, false when getContext throws', () => {
  assert.equal(detectWebGL(undefined), false);
  const doc = (ctx) => ({ createElement: () => ({ getContext: ctx }) });
  assert.equal(detectWebGL(doc((kind) => (kind === 'webgl' ? {} : null))), true);
  assert.equal(detectWebGL(doc(() => null)), false);
  assert.equal(detectWebGL(doc(() => { throw new Error('blocked'); })), false);
});
