// Hash routing without a library: `#/` is the start page, `#/play` the workspace.
//
// With the dev server (`graphersal --server`) there is no start page: the server serves one
// graph, so every route is the workspace on that graph.
//
// A playground link (`#/play?graph=..&q=..&run=1`, js/link.js) loads its sample graph, puts its
// query into the editor and, with run=1, runs it; the address then goes back to `#/play`, so a
// reload does not apply the link again. With the dev server only the query is opened: the served
// graph is the user's own data, so a link neither replaces it nor runs anything on it.

import { state, loadGraph, engineReady, isServerMode, attachServerGraph, openScript, run, toast } from './store.js';
import { storage } from './util.js';
import { parseLink } from './link.js';

function routeFromHash() {
  const h = window.location.hash.replace(/^#/, '');
  return h.startsWith('/play') ? 'play' : 'start';
}

export function navigate(route) {
  const hash = route === 'play' ? '#/play' : '#/';
  if (window.location.hash !== hash) window.location.hash = hash;
  else applyRoute();
}

const SAMPLE_NAMES = { modern: 'Modern (TinkerPop)', empty: 'empty', large: 'large generated', tree: 'file tree' };

/** Puts a link's query into the editor (a new tab when the current one holds the user's text). */
function openLinkScript(link) {
  if (link.script === null) return null;
  return openScript({ title: link.title || 'Linked query', script: link.script });
}

/** Applies a link in the browser playground. Returns true when the workspace is shown. */
async function applyLink(link) {
  // A graph is open in this page (the link was pasted into it): replacing it loses its changes.
  if (state.graph) {
    const ok = window.confirm(`This link loads the ${SAMPLE_NAMES[link.graph]} sample graph and replaces the graph open now. Continue?`);
    if (!ok) {
      openLinkScript(link);
      state.route = 'play';
      toast('The current graph stays; the linked query is in the editor (not run).', 'info');
      return true;
    }
  }
  if (!(await loadGraph({ kind: link.graph }))) return false;
  state.route = 'play';
  const tab = openLinkScript(link);
  if (tab && link.run) await run(link.script, tab.id);
  return true;
}

export async function applyRoute() {
  // A hash change can come before the engine exists (the WebAssembly module still loads).
  await engineReady();
  if (!state.engine) return;
  const link = parseLink(window.location.hash);
  if (link) history.replaceState(null, '', '#/play');
  if (link && link.error) toast(link.error, 'error', 8000);
  if (isServerMode()) {
    if (!state.graph) await attachServerGraph();
    if (window.location.hash !== '#/play') history.replaceState(null, '', '#/play');
    state.route = 'play';
    if (link && !link.error && openLinkScript(link)) {
      toast('The linked query is in the editor. It was not run: the dev server serves your own graph.', 'info', 6000);
    }
    return;
  }
  if (link && !link.error && (await applyLink(link))) return;
  const route = routeFromHash();
  if (route === 'play' && !state.graph) {
    // A reload on #/play: bring back the last sample graph (files are not stored; save the graph
    // with "Save graph" and load the file again).
    const last = storage.get('lastGraph', null);
    if (['modern', 'empty', 'large', 'tree'].includes(last) && (await loadGraph({ kind: last }))) {
      state.route = 'play';
      return;
    }
    if (window.location.hash !== '#/') window.location.hash = '#/';
    state.route = 'start';
    return;
  }
  state.route = route;
}
