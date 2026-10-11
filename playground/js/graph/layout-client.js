// The engine's force layout from the page (pure: no DOM; tested under Node with a fake engine).
//
// The layout runs in the engine (crates/graphersal-session/src/layout.rs: Barnes-Hut,
// ForceAtlas2-style), in the playground's Web Worker or on the dev server, in SLICES: the page
// starts a job (`engine.layoutStart`) and asks for one slice after the other
// (`engine.layoutStep`), so it can show progress and stop it (`engine.layoutCancel`, which keeps
// the positions reached so far). Between slices the worker / session answers other requests
// (an inspector click). Every answer is the job's status; one that is no longer running carries
// `positions`, a Float32Array with `x, y` per vertex. The engine caches finished layouts by
// topology: drawing the same view again comes back at once (`cached: true`).

/** Slice budget in milliseconds: the worker (wasm) answers other requests between slices. */
export const DEFAULT_SLICE_MS = 50;

/**
 * The layout input of a drawing: `{vertexCount, edgeSource, edgeTarget}` from a compact graph
 * view (its index columns as they are) or a schema view (`{nodes, edges}`, edges by node id; an
 * edge to a node that is not drawn is left out).
 * @param {Object} data
 * @param {'data'|'schema'} [kind]
 * @returns {{vertexCount:number, edgeSource:Uint32Array, edgeTarget:Uint32Array}}
 */
export function layoutInputOf(data, kind = 'data') {
  if (!data) return { vertexCount: 0, edgeSource: new Uint32Array(0), edgeTarget: new Uint32Array(0) };
  if (kind === 'schema') {
    const index = new Map(data.nodes.map((n, i) => [n.id, i]));
    const pairs = data.edges.filter((e) => index.has(e.source) && index.has(e.target));
    return {
      vertexCount: data.nodes.length,
      edgeSource: Uint32Array.from(pairs, (e) => index.get(e.source)),
      edgeTarget: Uint32Array.from(pairs, (e) => index.get(e.target)),
    };
  }
  return {
    vertexCount: data.vertexIds.length,
    edgeSource: Uint32Array.from(data.edgeSource),
    edgeTarget: Uint32Array.from(data.edgeTarget),
  };
}

/** Raised by `runEngineLayout` when the engine has no layout (an older worker or server). */
export class LayoutUnavailableError extends Error {
  constructor(message = 'the engine has no force layout') {
    super(message);
    this.name = 'LayoutUnavailableError';
  }
}

/**
 * Runs the engine's force layout of `input` to its end, slice by slice.
 * Resolves to `{positions, state, iterations, totalIterations, elapsedMs, cached}`: `state`
 * `done`, or `cancelled` when `signal` was aborted (the positions reached so far).
 * @param {Object} engine   Has `layoutStart(input, options)`, `layoutStep(job, budgetMs)`,
 *   `layoutCancel(job)` (engine.js).
 * @param {{vertexCount:number, edgeSource:Uint32Array, edgeTarget:Uint32Array}} input
 * @param {{budgetMs?:number, seed?:number, signal?:AbortSignal,
 *          onProgress?:(status:Object)=>void}} [options]
 */
export async function runEngineLayout(engine, input, options = {}) {
  if (!engine || typeof engine.layoutStart !== 'function') throw new LayoutUnavailableError();
  const budgetMs = options.budgetMs ?? DEFAULT_SLICE_MS;
  const signal = options.signal;
  const startOptions = { budgetMs };
  if (options.seed !== undefined) startOptions.seed = options.seed;
  let status = await engine.layoutStart(input, startOptions);
  while (status.state === 'running') {
    if (options.onProgress) options.onProgress(status);
    status = signal && signal.aborted
      ? await engine.layoutCancel(status.job)
      : await engine.layoutStep(status.job, budgetMs);
  }
  if (options.onProgress) options.onProgress(status);
  return {
    positions: status.positions,
    state: status.state,
    iterations: status.iterations,
    totalIterations: status.totalIterations,
    elapsedMs: status.elapsedMs,
    cached: !!status.cached,
  };
}
