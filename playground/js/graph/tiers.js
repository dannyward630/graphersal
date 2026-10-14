// Which renderer tier draws a drawing, and what the panel offers in it (pure: no DOM, no Vue;
// tested under Node).
//
// Two tiers (`playground/README.md`): DETAILED (Cytoscape.js, Canvas 2D: every
// style, edge labels, curved edges, every layout, the schema graph) and FAST (sigma.js + graphology,
// WebGL: vertex labels by zoom level, hover, click, straight edges, the engine's force layout).
// `chooseTier` picks one by the number of elements drawn (vertices + edges), with HYSTERESIS so a
// drawing that grows and shrinks around the threshold does not flip back and forth, honours the
// user's manual choice ('auto' | 'detailed' | 'fast', kept in the settings), and falls back to
// Detailed with a hard limit when the browser has no WebGL. The schema drawing is always Detailed.
//
// The thresholds were measured (playground/README.md, "Graph drawing", GR3 table).

import { TIER_CAPABILITIES } from './renderer.js';

/**
 * The tier thresholds, in elements (vertices + edges). Calibrated in GR3 (README "Graph
 * drawing"): Detailed pans without a hitch up to ~2000 elements (longest frame 17-25 ms), but from
 * 2000 its reduced plan (edges hidden behind a texture while moving) costs a 108 ms frame at 2342
 * elements and 142-441 ms at 3000-10 000, while Fast stays within one 120 Hz frame (longest
 * <= 17.5 ms up to 100 000 elements) and draws its first frame 4-9x sooner.
 */
export const TIER_TUNING = Object.freeze({
  /** Automatic choice: Fast from this many elements (where Detailed's reduced plan begins). */
  fastFrom: 2000,
  /** Automatic choice: once Fast, back to Detailed only below this many (hysteresis, -20 %). */
  detailedBelow: 1600,
  /** Detailed above this many asks first ("Draw it"): forced Detailed, or no WebGL. */
  detailedConfirm: 5000,
  /** Without WebGL, Detailed draws at most this many elements; a larger view is cut. */
  noWebglMax: 10000,
  /** Fast above this many asks first (the end of the Fast range). */
  fastMax: 200000,
  /** A double-click selection / "Open in Detailed" neighbourhood has at most this many vertices. */
  drillMaxNodes: 1000,
});

/** The user's choices of the panel's tier switch. */
export const TIER_CHOICES = Object.freeze([
  { id: 'auto', label: 'Auto', title: 'Choose by size: Detailed for small drawings, Fast (WebGL) for large ones' },
  { id: 'detailed', label: 'Detailed', title: 'Always Detailed (Cytoscape): every style, edge labels, curved edges, every layout' },
  { id: 'fast', label: 'Fast', title: 'Always Fast (WebGL): large drawings stay smooth; fewer drawing features' },
]);

/** A known tier choice (`auto` for anything else, e.g. an old or edited setting). */
export function normalizeChoice(choice) {
  return TIER_CHOICES.some((c) => c.id === choice) ? choice : 'auto';
}

const fmt = (n) => n.toLocaleString('en-US');

/**
 * The tier of a drawing.
 * @param {{size:number, kind?:'data'|'schema', choice?:string, previous?:?string, webgl?:boolean,
 *          fastError?:?string}} input
 *   `size` vertices + edges; `choice` the user's switch; `previous` the tier the last automatic
 *   choice gave (hysteresis); `webgl` false when the browser has none; `fastError` why the Fast
 *   renderer failed to start (offline, blocked).
 * @returns {{tier:'detailed'|'fast', why:string, warning:?string, notice:?string,
 *            confirm:?{size:number, reason:string}, cap:?number}}
 *   `why`: 'schema' | 'auto' | 'choice' | 'no-fast' (no WebGL, or the Fast renderer failed).
 *   `warning`: a forced tier the size does not suit. `notice`: the fallback to Detailed.
 *   `confirm`: the drawing waits for an explicit "Draw it". `cap`: draw at most this many
 *   elements (no Fast tier).
 */
export function chooseTier({ size, kind = 'data', choice = 'auto', previous = null, webgl = true, fastError = null }, tuning = TIER_TUNING) {
  const out = { tier: 'detailed', why: 'auto', warning: null, notice: null, confirm: null, cap: null };
  if (kind === 'schema') return { ...out, why: 'schema' };
  const pick = normalizeChoice(choice);
  if (!webgl || fastError) {
    out.why = 'no-fast';
    const cause = fastError ? `The Fast tier could not start (${fastError})` : 'This browser has no WebGL, so the Fast tier is not available';
    out.notice = `${cause}: the drawing uses the Detailed tier` +
      (size > tuning.noWebglMax ? `, which draws at most ${fmt(tuning.noWebglMax)} vertices and edges here.` : '.');
    if (size > tuning.noWebglMax) out.cap = tuning.noWebglMax;
    const drawn = Math.min(size, tuning.noWebglMax);
    if (drawn > tuning.detailedConfirm) {
      const cut = out.cap ? ` (only the first ${fmt(out.cap)} vertices and edges are drawn)` : '';
      out.confirm = { size: drawn, reason: `Without WebGL this drawing uses the Detailed tier${cut}, which can make the page slow to respond at this size.` };
    }
    return out;
  }
  if (pick === 'detailed') {
    out.why = 'choice';
    if (size >= tuning.fastFrom) {
      out.warning = `Detailed is forced for ${fmt(size)} vertices and edges (Fast takes over from ${fmt(tuning.fastFrom)}): ` +
        'zooming and panning can stutter.';
    }
    if (size > tuning.detailedConfirm) {
      out.confirm = { size, reason: 'The Detailed tier is forced: drawing that many in it can make the page slow to respond.' };
    }
    return out;
  }
  if (pick === 'fast') {
    out.tier = 'fast';
    out.why = 'choice';
  } else {
    const fast = size >= tuning.fastFrom || (previous === 'fast' && size >= tuning.detailedBelow);
    out.tier = fast ? 'fast' : 'detailed';
  }
  if (out.tier === 'fast' && size > tuning.fastMax) {
    out.confirm = { size, reason: `This is above the Fast tier's range (${fmt(tuning.fastMax)}): the page can become slow to respond.` };
  }
  return out;
}

/** Display names of the built-in layouts (the reasons below name them). */
const LAYOUT_NAMES = Object.freeze({ concentric: 'Concentric', breadthfirst: 'Tree', circle: 'Circle', grid: 'Grid', cose: 'Force (cose)' });

/**
 * What the current tier lacks compared with Detailed, as short phrases for the banner (empty for
 * Detailed itself).
 * @param {import('./renderer.js').RendererCapabilities} caps
 * @returns {string[]}
 */
export function missingFeatures(caps, reference = TIER_CAPABILITIES.detailed) {
  if (!caps) return [];
  const out = [];
  if (reference.edgeLabels && !caps.edgeLabels) out.push('edge labels (hover a vertex to see the labels of its edges)');
  if (reference.curvedEdges && !caps.curvedEdges) out.push('curved edges and loops');
  if (reference.arrows && !caps.arrows) out.push('arrow heads');
  const layouts = reference.layouts.filter((l) => l !== 'cose' && !caps.layouts.includes(l)).map((l) => LAYOUT_NAMES[l] || l);
  if (layouts.length) out.push(`${layouts.join(' or ')} layout`);
  if (reference.svg && !caps.svg) out.push('SVG export');
  return out;
}

/**
 * The state of the panel's controls in a tier: `{disabled, reason}` per control, the layout menu
 * per layout id. A disabled control keeps its value; its tooltip says why it is off.
 * @param {import('./renderer.js').RendererCapabilities} caps
 * @param {string[]} layoutIds   The layout menu's ids (graph-canvas.js LAYOUTS).
 */
export function controlStates(caps, layoutIds) {
  const tierName = caps && caps.tier === 'fast' ? 'The Fast tier' : 'This drawing';
  const off = (reason) => ({ disabled: true, reason });
  const on = { disabled: false, reason: null };
  const edgeText = caps && !caps.edgeLabels
    ? off(`${tierName} draws no edge labels: hover a vertex to see the labels of its edges, or switch to Detailed.`)
    : on;
  const layouts = {};
  for (const id of layoutIds) {
    // `cose` and `engine` exist in every tier (in Fast both are the engine's force layout).
    if (!caps || id === 'engine' || id === 'cose' || caps.layouts.includes(id)) layouts[id] = on;
    else layouts[id] = off(`${tierName} has no ${LAYOUT_NAMES[id] || id} layout: switch to Detailed to use it.`);
  }
  return {
    edgeLabels: edgeText,
    edgeIds: edgeText,
    svg: caps && caps.svg ? on : off(caps && caps.tier === 'fast'
      ? 'SVG export is only available in the Detailed tier.'
      : 'There is no SVG export yet.'),
    layouts,
    addElements: caps && !caps.addElements
      ? off(`${tierName} cannot add vertices or edges with the mouse: switch to Detailed to add them. Editing properties and deleting work here.`)
      : on,
  };
}

/**
 * The banner of a tier: what it lacks, as one sentence (`null` for Detailed, which lacks
 * nothing). `size` names the drawing's size when the automatic choice picked Fast.
 */
export function tierBanner(caps, { why = 'auto', size = 0 } = {}) {
  if (!caps || caps.tier !== 'fast') return null;
  const lacks = missingFeatures(caps);
  const head = why === 'auto' ? `Fast tier (WebGL) for ${fmt(size)} vertices and edges` : 'Fast tier (WebGL)';
  return lacks.length ? `${head}: no ${lacks.join(', no ')}.` : `${head}.`;
}

/**
 * Whether this browser can draw with WebGL (sigma.js needs it). `doc` is injectable for tests;
 * without a document (Node) the answer is false.
 */
export function detectWebGL(doc = globalThis.document) {
  if (!doc || typeof doc.createElement !== 'function') return false;
  try {
    const canvas = doc.createElement('canvas');
    return !!(canvas.getContext('webgl2') || canvas.getContext('webgl'));
  } catch (err) {
    return false;
  }
}
