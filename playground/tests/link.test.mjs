// Node tests of playground links (js/link.js): parsing a location hash, the inverse builder, the
// round trip of scripts with quotes, newlines, `+`, `&`, `#` and non-ASCII text, and the refusals.
// Run: node --test playground/tests/*.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  parseLink, linkHash, LINK_GRAPHS, MAX_LINK_SCRIPT, LINK_FORMAT, MAX_LINK_DATA, MAX_SHARE_LINK,
  encodeLinkData, decodeLinkData, isUnchangedSample, shareLink,
} from '../js/link.js';

test('hashes without a link are not links', () => {
  for (const h of ['', '#', '#/', '#/play', '#/play?', '#/play?foo=1', '#/?q=g.v()', '#/other?q=x']) {
    assert.equal(parseLink(h), null, h);
  }
});

test('a link names a graph, a script, a title and run', () => {
  assert.deepEqual(parseLink('#/play?graph=tree&q=g.v().count()&title=Count&run=1'), {
    graph: 'tree', data: null, schema: null, script: 'g.v().count()', title: 'Count', run: true,
  });
  // the graph defaults to modern; run needs a script and an explicit 1
  assert.deepEqual(parseLink('#/play?q=g.v()'), { graph: 'modern', data: null, schema: null, script: 'g.v()', title: '', run: false });
  assert.deepEqual(parseLink('#/play?graph=empty&run=1'), { graph: 'empty', data: null, schema: null, script: null, title: '', run: false });
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
      assert.deepEqual(link, { graph, data: null, schema: null, script, title: 'T', run: script !== '' }, script);
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

// ---------------------------------------------------------------- links that carry their graph

const GRAPHSON = [
  '{"id":"1","label":"person","properties":{"name":[{"id":0,"value":"marko"}]}}',
  '{"id":"2","label":"person","properties":{"name":[{"id":1,"value":"Zoë 名前 ✓"}]}}',
].join('\n');
const SCHEMA = '{"mode":"open","vertices":{"person":{"schema":{"type":"object"}}}}';

test('graph data survives encoding: compressed, base64url, unicode', async () => {
  for (const text of [GRAPHSON, SCHEMA, '', 'x'.repeat(100000)]) {
    const encoded = await encodeLinkData(text);
    assert.match(encoded, /^[A-Za-z0-9_-]*$/);
    assert.equal(await decodeLinkData(encoded), text);
  }
  // repetitive data compresses: a link holds far more graph than its length
  assert.ok((await encodeLinkData('x'.repeat(100000))).length < 1000);
});

test('a link carries a graph and a schema and opens them', async () => {
  const data = await encodeLinkData(GRAPHSON);
  const schema = await encodeLinkData(SCHEMA);
  const hash = linkHash({ data, schema, script: 'g.v().count()', title: 'Bug', run: true });
  assert.match(hash, new RegExp(`^#/play\\?v=${LINK_FORMAT}&data=`));
  assert.doesNotMatch(hash, /graph=/);
  const link = parseLink(hash);
  assert.deepEqual(link, { graph: null, data, schema, script: 'g.v().count()', title: 'Bug', run: true });
  assert.equal(await decodeLinkData(link.data), GRAPHSON);
  assert.equal(await decodeLinkData(link.schema), SCHEMA);
  // without a schema, and without v (absent means format 1)
  assert.deepEqual(parseLink(`#/play?data=${data}`), { graph: null, data, schema: null, script: null, title: '', run: false });
});

test('unknown parameters are ignored: a later parameter is additive', async () => {
  const data = await encodeLinkData(GRAPHSON);
  assert.equal(parseLink(`#/play?v=1&data=${data}&catalog=abc&q=x`).data, data);
  assert.equal(parseLink('#/play?graph=tree&catalog=abc&q=x').graph, 'tree');
});

test('malformed links that carry a graph are refused with a reason', async () => {
  const data = await encodeLinkData(GRAPHSON);
  assert.match(parseLink(`#/play?v=${LINK_FORMAT + 1}&data=${data}`).error, /newer playground/);
  assert.match(parseLink(`#/play?v=x&data=${data}`).error, /not a version number/);
  assert.match(parseLink(`#/play?graph=modern&data=${data}`).error, /only do one/);
  assert.match(parseLink('#/play?graph=modern&schema=abc').error, /schema without a graph/);
  assert.match(parseLink('#/play?data=a%2Bb').error, /graph is damaged/);
  assert.match(parseLink(`#/play?data=${data}&schema=a.b`).error, /schema is damaged/);
  assert.match(parseLink('#/play?data=').error, /graph is empty/);
  assert.match(parseLink(`#/play?data=${'A'.repeat(MAX_LINK_DATA + 1)}`).error, /longer than/);
});

test('damaged or oversized graph data is refused when it is decoded', async () => {
  const data = await encodeLinkData(GRAPHSON);
  await assert.rejects(decodeLinkData(data.slice(0, data.length - 7)), /damaged/);
  await assert.rejects(decodeLinkData('not base64url!'), /damaged/);
  await assert.rejects(decodeLinkData('AAAA'), /damaged/);
  // a decompression bomb stops at the limit
  const bomb = await encodeLinkData('x'.repeat(200000));
  await assert.rejects(decodeLinkData(bomb, 100000), /larger than/);
  assert.equal((await decodeLinkData(bomb, 200000)).length, 200000);
});

test('only an unchanged sample is shared by its name', () => {
  assert.equal(isUnchangedSample({ kind: 'modern' }, { commitSeq: 0 }, false), true);
  assert.equal(isUnchangedSample({ kind: 'modern' }, { commitSeq: 1 }, false), false); // changed
  assert.equal(isUnchangedSample({ kind: 'modern' }, { commitSeq: 0 }, true), false); // a past view
  assert.equal(isUnchangedSample({ kind: 'modern' }, null, false), false); // no store: unknown
  assert.equal(isUnchangedSample({ kind: 'file' }, { commitSeq: 0 }, false), false);
  assert.equal(isUnchangedSample(null, { commitSeq: 0 }, false), false);
});

test('a shared link replaces the address hash and says whether it fits', () => {
  const small = shareLink('https://play.graphersal.dev/#/play', { graph: 'tree', script: 'g.v()' });
  assert.equal(small.url, 'https://play.graphersal.dev/#/play?graph=tree&q=g.v()');
  assert.equal(small.chars, small.url.length);
  assert.equal(small.fits, true);
  const big = shareLink('https://play.graphersal.dev/', { data: 'A'.repeat(MAX_SHARE_LINK), script: 'g.v()' });
  assert.equal(big.fits, false);
});
