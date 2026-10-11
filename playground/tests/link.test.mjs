// Node tests of playground links (js/link.js): parsing a location hash, the inverse builder, the
// round trip of scripts with quotes, newlines, `+`, `&`, `#` and non-ASCII text, and the refusals.
// Run: node --test playground/tests/*.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { parseLink, linkHash, LINK_GRAPHS, MAX_LINK_SCRIPT } from '../js/link.js';

test('hashes without a link are not links', () => {
  for (const h of ['', '#', '#/', '#/play', '#/play?', '#/play?foo=1', '#/?q=g.v()', '#/other?q=x']) {
    assert.equal(parseLink(h), null, h);
  }
});

test('a link names a graph, a script, a title and run', () => {
  assert.deepEqual(parseLink('#/play?graph=tree&q=g.v().count()&title=Count&run=1'), {
    graph: 'tree', script: 'g.v().count()', title: 'Count', run: true,
  });
  // the graph defaults to modern; run needs a script and an explicit 1
  assert.deepEqual(parseLink('#/play?q=g.v()'), { graph: 'modern', script: 'g.v()', title: '', run: false });
  assert.deepEqual(parseLink('#/play?graph=empty&run=1'), { graph: 'empty', script: null, title: '', run: false });
  assert.equal(parseLink('#/play?q=g.v()&run=true').run, false);
  assert.equal(parseLink('#/play?q=%20%20&run=1').run, false);
});

test('scripts survive the round trip', () => {
  const scripts = [
    'g.v().has("name", "marko").out("knows").values("name")',
    '// who created software together with marko?\ng.v().has("name", "marko").as("me")\n  .out("created").in("created")\n  .where(P.neq("me"))\n  .path().by("name")',
    'let a = 1 + 2; a & 3 # not a fragment ? = %41 \'x\' `y`',
    'g.v().has("name", "Zoë 名前 ✓")',
    '',
  ];
  for (const script of scripts) {
    for (const graph of LINK_GRAPHS) {
      const link = parseLink(linkHash({ graph, script, title: 'T', run: true }));
      assert.deepEqual(link, { graph, script, title: 'T', run: script !== '' }, script);
    }
  }
});

test('a plus sign in a hand-written link reads as a space, %2B as a plus', () => {
  assert.equal(parseLink('#/play?q=g.v()+.count()').script, 'g.v() .count()');
  assert.equal(parseLink('#/play?q=1%2B2').script, '1+2');
  assert.match(linkHash({ script: '1+2' }), /q=1%2B2/);
});

test('unknown graphs and oversized scripts are refused with a reason', () => {
  assert.match(parseLink('#/play?graph=crew&q=g.v()').error, /unknown graph "crew".*modern, empty, large, tree/);
  assert.match(parseLink(`#/play?q=${'x'.repeat(MAX_LINK_SCRIPT + 1)}`).error, /longer than/);
  assert.equal(parseLink(`#/play?q=${'x'.repeat(MAX_LINK_SCRIPT)}`).script.length, MAX_LINK_SCRIPT);
});

test('a title is trimmed and cut', () => {
  assert.equal(parseLink(`#/play?q=x&title=${encodeURIComponent('  ' + 'a'.repeat(60))}`).title, 'a'.repeat(40));
});
