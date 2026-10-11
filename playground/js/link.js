// Playground links: a sample graph and a query in the address, so a page (the book) can open an
// example ready to run.
//
//   https://play.graphersal.dev/#/play?graph=modern&q=g.v().count()&run=1
//
// The parameters follow `#/play?`, in the fragment: the browser never sends them to the server.
// Values are percent-encoded (`encodeURIComponent`; `+` also reads as a space, as in a query
// string). Parameters:
//   graph   a sample graph: modern (the default), empty, large, tree
//   q       the script for the editor
//   title   the tab name (optional)
//   run     1: run the script once it is in the editor (never with the dev server, whose graph is
//           the user's own data)
// Pure functions, tested in tests/link.test.mjs; router.js applies a link.

/** The sample graphs a link may name (the playground's start page offers the same). */
export const LINK_GRAPHS = Object.freeze(['modern', 'empty', 'large', 'tree']);

/** The longest script a link may carry, in characters. */
export const MAX_LINK_SCRIPT = 100000;

const MAX_TITLE = 40;

/**
 * The link in a location hash, or null when the hash carries none (`#/`, `#/play`, a hash without
 * `graph` or `q`). A link that names an unknown graph or is too long comes back with `error` and
 * nothing else to apply.
 * @param {string} hash  `window.location.hash`, with or without the leading `#`
 * @returns {null | {graph: string, script: string|null, title: string, run: boolean} | {error: string}}
 */
export function parseLink(hash) {
  const h = String(hash || '').replace(/^#/, '');
  const at = h.indexOf('?');
  if (!h.startsWith('/play') || at < 0) return null;
  let params;
  try {
    params = new URLSearchParams(h.slice(at + 1));
  } catch (e) {
    return { error: 'The link could not be read.' };
  }
  if (!params.has('graph') && !params.has('q')) return null;
  const graph = params.get('graph') || 'modern';
  if (!LINK_GRAPHS.includes(graph)) {
    return { error: `The link names an unknown graph "${graph.slice(0, 40)}"; the samples are ${LINK_GRAPHS.join(', ')}.` };
  }
  const script = params.has('q') ? params.get('q') : null;
  if (script !== null && script.length > MAX_LINK_SCRIPT) {
    return { error: `The link's query is longer than ${MAX_LINK_SCRIPT} characters.` };
  }
  const title = (params.get('title') || '').trim().slice(0, MAX_TITLE);
  return { graph, script, title, run: params.get('run') === '1' && script !== null && script.trim() !== '' };
}

/**
 * The location hash of a link (the inverse of `parseLink`).
 * @param {{graph?: string, script?: string, title?: string, run?: boolean}} link
 */
export function linkHash({ graph = 'modern', script, title, run = false }) {
  const parts = [`graph=${encodeURIComponent(graph)}`];
  if (script !== undefined && script !== null) parts.push(`q=${encodeURIComponent(script)}`);
  if (title) parts.push(`title=${encodeURIComponent(title)}`);
  if (run) parts.push('run=1');
  return `#/play?${parts.join('&')}`;
}
