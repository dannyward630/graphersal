// Node tests of the playground logic that runs without a browser: the query-tab store (js/tabs.js)
// and the /reset page's inline script (reset/index.html, run against stubbed browser globals).
// Run: node --test playground/tests/*.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';
import * as tabs from '../js/tabs.js';

const fresh = () => tabs.restoreTabs(null);

test('restore: nothing saved gives one default tab; legacy script and options seed it', () => {
  const ts = fresh();
  assert.equal(ts.tabs.length, 1);
  assert.equal(ts.tabs[0].script, tabs.DEFAULT_SCRIPT);
  assert.equal(ts.activeId, ts.tabs[0].id);
  const legacy = tabs.restoreTabs(null, { script: 'g.e()', runOptions: { memory: true } });
  assert.equal(legacy.tabs[0].script, 'g.e()');
  assert.deepEqual(legacy.tabs[0].runOptions, { profile: true, memory: true });
  // Garbage in storage never breaks the page.
  for (const bad of [42, 'x', { tabs: 'no' }, { tabs: [null, { script: 1 }] }]) {
    assert.equal(tabs.restoreTabs(bad).tabs.length, 1);
  }
});

test('create, switch, rename, close; the last tab is emptied, not removed', () => {
  const ts = fresh();
  const first = ts.tabs[0];
  const second = tabs.addTab(ts, { script: 'g.v(1)' });
  assert.equal(ts.tabs.length, 2);
  assert.equal(ts.activeId, second.id);
  assert.equal(second.name, 'Query 2');
  tabs.activateTab(ts, first.id);
  assert.equal(tabs.activeTab(ts), first);
  tabs.activateRelative(ts, -1); // wraps around
  assert.equal(ts.activeId, second.id);
  tabs.activateIndex(ts, 0);
  assert.equal(ts.activeId, first.id);
  assert.ok(tabs.renameTab(ts, second.id, '  people   query '));
  assert.equal(second.name, 'people query');
  assert.equal(second.autoName, false);
  assert.ok(!tabs.renameTab(ts, second.id, '   '), 'a blank name keeps the old one');
  // Closing the active tab activates its neighbour.
  tabs.activateTab(ts, first.id);
  tabs.closeTab(ts, first.id);
  assert.deepEqual(ts.tabs.map((t) => t.id), [second.id]);
  assert.equal(ts.activeId, second.id);
  second.result = { run_id: 1 };
  tabs.closeTab(ts, second.id);
  assert.equal(ts.tabs.length, 1);
  assert.equal(ts.tabs[0].script, '');
  assert.equal(ts.tabs[0].result, null);
  assert.equal(ts.tabs[0].name, 'people query');
});

test('new tabs go after the active one and inherit its run options; duplicate and close others', () => {
  const ts = fresh();
  ts.tabs[0].runOptions = { profile: false, memory: false };
  const b = tabs.addTab(ts);
  assert.equal(b.runOptions.profile, false);
  b.runOptions.memory = true;
  assert.equal(ts.tabs[0].runOptions.memory, false, 'options are copied, not shared');
  tabs.activateTab(ts, ts.tabs[0].id);
  const c = tabs.addTab(ts);
  assert.deepEqual(ts.tabs.map((t) => t.id), [1, c.id, b.id]);
  ts.tabs[0].script = 'g.v().count()';
  const dup = tabs.duplicateTab(ts, 1);
  assert.equal(dup.script, 'g.v().count()');
  assert.equal(dup.name, 'Query 1 (copy)');
  assert.equal(tabs.duplicateTab(ts, 1).name, 'Query 1 (copy 2)');
  assert.equal(ts.tabs[1], tabs.findTab(ts, ts.activeId), 'the copy sits next to its source');
  tabs.closeOtherTabs(ts, b.id);
  assert.deepEqual(ts.tabs.map((t) => t.id), [b.id]);
  assert.equal(ts.activeId, b.id);
  tabs.moveTab(ts, b.id, 5);
  assert.equal(ts.tabs.length, 1);
});

test('persist: tabs, names and the active tab round-trip; results are not persisted', () => {
  const ts = fresh();
  const b = tabs.addTab(ts, { script: 'g.e().count()' });
  tabs.renameTab(ts, b.id, 'edges');
  b.result = { run_id: 3, display: {} };
  b.profile = { metrics: {} };
  const saved = JSON.parse(JSON.stringify(tabs.serializeTabs(ts)));
  assert.ok(!JSON.stringify(saved).includes('run_id'));
  const back = tabs.restoreTabs(saved);
  assert.deepEqual(back.tabs.map((t) => [t.id, t.name, t.script]), ts.tabs.map((t) => [t.id, t.name, t.script]));
  assert.equal(back.activeId, b.id);
  assert.equal(back.tabs[1].result, null);
  assert.equal(back.tabs[1].autoName, false);
  // An unknown active id falls back to the first tab; duplicate ids are renumbered.
  const odd = tabs.restoreTabs({ tabs: [{ id: 4, script: 'a' }, { id: 4, script: 'b' }], active: 99 });
  assert.equal(new Set(odd.tabs.map((t) => t.id)).size, 2);
  assert.equal(odd.activeId, 4);
});

test('examples reuse an empty tab or an unmodified example, otherwise open a new tab', () => {
  const ts = fresh();
  ts.tabs[0].script = '   ';
  const ex1 = { title: 'Count people', script: 'g.v().has_label("person").count()' };
  const r1 = tabs.openExample(ts, ex1);
  assert.equal(r1.reused, true);
  assert.equal(ts.tabs.length, 1);
  assert.equal(ts.tabs[0].script, ex1.script);
  assert.equal(ts.tabs[0].name, 'Count people', 'an auto-named tab takes the example title');
  // Still the unmodified example: the next one replaces it.
  const ex2 = { title: 'Edges', script: 'g.e()' };
  assert.equal(tabs.openExample(ts, ex2).reused, true);
  assert.equal(ts.tabs.length, 1);
  assert.equal(ts.tabs[0].script, 'g.e()');
  // Any edit makes it user text: the next example opens a new tab and leaves the text alone.
  ts.tabs[0].script = 'g.e().count()';
  const r3 = tabs.openExample(ts, ex1);
  assert.equal(r3.reused, false);
  assert.equal(ts.tabs.length, 2);
  assert.equal(ts.tabs[0].script, 'g.e().count()');
  assert.equal(tabs.activeTab(ts).script, ex1.script);
  // A renamed tab keeps its name when an example reuses it.
  tabs.renameTab(ts, r3.tab.id, 'mine');
  tabs.openExample(ts, ex2);
  assert.equal(r3.tab.name, 'mine');
  assert.equal(r3.tab.script, 'g.e()');
  // The default text of a new playground is user text (it is not an example).
  const plain = fresh();
  assert.equal(tabs.openExample(plain, ex1).reused, false);
  // The example provenance survives a reload, so reuse still works afterwards.
  const back = tabs.restoreTabs(JSON.parse(JSON.stringify(tabs.serializeTabs(ts))));
  assert.equal(tabs.openExample(back, ex1).reused, true);
});

test('a run that changed the graph marks the other tabs with a result as possibly stale', () => {
  const ts = fresh();
  const a = ts.tabs[0];
  const b = tabs.addTab(ts);
  const c = tabs.addTab(ts); // never ran: nothing to mark
  tabs.recordRun(ts, a.id, { result: { run_id: 1 }, profile: { run: 1 }, script: 'g.v()', mutated: false });
  assert.equal(a.profileScript, 'g.v()');
  tabs.recordRun(ts, b.id, { result: { run_id: 2 }, profile: null, script: 'g.add_v("x")', mutated: true });
  assert.equal(a.stale, true);
  assert.equal(b.stale, false);
  assert.equal(c.stale, false);
  assert.equal(b.profile, null, 'a run without metrics keeps the previous profile (none here)');
  tabs.recordRun(ts, a.id, { result: { run_id: 3 }, profile: null, script: 'g.v()', mutated: false });
  assert.equal(a.stale, false, 'running again clears the mark');
  assert.equal(a.profile.run, 1, 'the last profile stays when a run returns none');
  tabs.clearResults(ts);
  assert.ok(ts.tabs.every((t) => t.result === null && t.profile === null && !t.stale));
});

test('the tab count is bounded', () => {
  const ts = fresh();
  while (tabs.addTab(ts)) { /* fill up */ }
  assert.equal(ts.tabs.length, tabs.MAX_TABS);
  // The active tab is empty: an example still fits into it.
  assert.equal(tabs.openExample(ts, { title: 'x', script: 'g.v()' }).reused, true);
  tabs.activeTab(ts).script = 'my own text';
  assert.equal(tabs.openExample(ts, { title: 'x', script: 'g.v()' }).tab, null);
  assert.equal(tabs.activeTab(ts).script, 'my own text');
});

// ------------------------------------------------------------------ /reset

/** A Storage stub with the Web Storage API surface the reset script uses. */
function storageStub(entries) {
  const map = new Map(Object.entries(entries));
  return {
    get length() { return map.size; },
    key: (i) => [...map.keys()][i] ?? null,
    getItem: (k) => (map.has(k) ? map.get(k) : null),
    setItem: (k, v) => map.set(k, String(v)),
    removeItem: (k) => map.delete(k),
    dump: () => Object.fromEntries(map),
  };
}

function runResetPage(pathname) {
  const html = readFileSync(fileURLToPath(new URL('../reset/index.html', import.meta.url)), 'utf8');
  const m = /\/\/ reset-script:begin[^\n]*\n([\s\S]*?)\/\/ reset-script:end/.exec(html);
  assert.ok(m, 'the reset script is between its markers');
  const el = () => ({ textContent: '', hidden: true, href: '', appendChild() {} });
  const elements = { status: el(), details: el(), continue: el() };
  const local = storageStub({
    'graphersal.playground.history': '[{"script":"g.v()"}]',
    'graphersal.playground.tabs': '{"tabs":[]}',
    'graphersal.playground.layout': '{}',
    'graphersal.playground.settings': '{"graphMaxNodes":100000}',
    'graphersal.playground.lastGraph': '"large"',
    'other.app.key': 'kept',
  });
  const timers = [];
  let replaced = null;
  const window = {
    localStorage: local,
    sessionStorage: storageStub({ 'graphersal.playground.x': '1' }),
    location: { pathname, origin: 'http://localhost:8000', replace: (u) => { replaced = u; } },
  };
  const sandbox = {
    window,
    document: { getElementById: (id) => elements[id], createElement: () => ({ textContent: '' }) },
    setTimeout: (fn) => { timers.push(fn); return timers.length; },
    Promise,
  };
  vm.runInNewContext(m[1], sandbox);
  return { local, elements, timers, replaced: () => replaced };
}

test('/reset clears every playground key except the history and opens the Modern graph', async () => {
  for (const [path, root] of [['/reset/', '/'], ['/reset', '/'], ['/reset/index.html', '/'], ['/pg/reset/', '/pg/']]) {
    const page = runResetPage(path);
    assert.deepEqual(page.local.dump(), {
      'graphersal.playground.history': '[{"script":"g.v()"}]',
      'graphersal.playground.lastGraph': '"modern"',
      'other.app.key': 'kept',
    });
    const target = `http://localhost:8000${root}#/play`;
    assert.equal(page.elements.continue.href, target, `fallback link for ${path}`);
    await new Promise((r) => setImmediate(r));
    page.timers.forEach((fn) => fn()); // the race's timeout, then the redirect delay
    await new Promise((r) => setImmediate(r));
    page.timers.forEach((fn) => fn());
    assert.equal(page.replaced(), target, `redirect for ${path}`);
  }
});
