// Node tests of the AI Chat's Markdown renderer (js/markdown.js): GFM tables (alignment, escaped
// pipes, ragged rows), code-block classification, raw HTML staying text, link schemes, and the
// table -> Markdown copy.
// Run: node --test playground/tests/*.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  parseMarkdown, parseInlines, tableToMarkdown, isQueryBlock, safeHref, splitRow, inlineText,
} from '../js/markdown.js';

const only = (text, type) => {
  const blocks = parseMarkdown(text);
  const found = blocks.filter((b) => b.type === type);
  assert.equal(found.length, 1, JSON.stringify(blocks));
  return found[0];
};

/** Every node of a block tree (blocks, items, inlines, cells). */
function walk(nodes, out = []) {
  for (const n of nodes || []) {
    if (Array.isArray(n)) {
      walk(n, out);
      continue;
    }
    out.push(n);
    walk(n.children, out);
    if (n.items) walk(n.items, out);
    if (n.header) walk(n.header, out);
    if (n.rows) for (const r of n.rows) walk(r, out);
  }
  return out;
}

test('a GFM table: header, alignment row, rows; escaped pipes stay in the cell', () => {
  const t = only([
    'Here are the people:',
    '',
    '| name | note | age |',
    '|:-----|:----:|----:|',
    '| marko | a \\| b | 29 |',
    '| `x\\|y` | **bold** |',
    '| vadas | c | 27 | extra |',
    '',
    'after',
  ].join('\n'), 'table');
  assert.deepEqual(t.align, ['left', 'center', 'right']);
  assert.deepEqual(t.header.map((c) => c.text), ['name', 'note', 'age']);
  assert.deepEqual(t.rows.map((r) => r.map((c) => c.text)), [
    ['marko', 'a | b', '29'],
    ['`x|y`', '**bold**', ''], // a short row is padded
    ['vadas', 'c', '27'], // a long row is cut to the header
  ]);
  assert.deepEqual(t.rows[1][0].children, [{ type: 'code', text: 'x|y' }]);
  assert.equal(t.rows[1][1].children[0].type, 'strong');
  // Without an alignment row there is no table; an alignment row of another width neither.
  assert.equal(parseMarkdown('| a | b |\n| c | d |').some((b) => b.type === 'table'), false);
  assert.equal(parseMarkdown('| a | b |\n|---|').some((b) => b.type === 'table'), false);
  assert.deepEqual(only('a | b\n--- | ---\n1 | 2', 'table').rows[0].map((c) => c.text), ['1', '2']);
  assert.deepEqual(splitRow('| a \\| b | c |'), ['a \\| b', 'c']);
});

test('a table copies as normalised GFM with pipes escaped', () => {
  const t = only('|name|note|n|\n|---|:-:|--:|\n| marko | a \\| b | 29 |\n| josh |', 'table');
  assert.equal(tableToMarkdown(t), [
    '| name | note | n |',
    '| --- | :---: | ---: |',
    '| marko | a \\| b | 29 |',
    '| josh |  |  |',
    '',
  ].join('\n'));
  // A round trip parses to the same table.
  const again = only(tableToMarkdown(t), 'table');
  assert.deepEqual(again.rows.map((r) => r.map((c) => c.text)), t.rows.map((r) => r.map((c) => c.text)));
  assert.deepEqual(again.align, t.align);
});

test('code blocks: query languages and untagged g. text are queries; others are not', () => {
  for (const lang of ['graphersal', 'gremlin', 'groovy', 'rhai', 'Graphersal']) {
    const b = only('```' + lang + '\ng.v().count()\n```', 'code');
    assert.equal(b.query, true, lang);
    assert.equal(b.text, 'g.v().count()');
  }
  assert.equal(only('```\ng.v().has_label("person")\n```', 'code').query, true);
  assert.equal(only('```\nls -l\n```', 'code').query, false);
  assert.equal(only('```json\n{"g.": 1}\n```', 'code').query, false);
  assert.equal(only('~~~python\ng.v()\n~~~', 'code').query, false);
  assert.equal(isQueryBlock('', '  g.e()'), true);
  // An unclosed fence runs to the end; the text keeps its lines and inner backticks.
  const open = only('```graphersal\ng.v()\n  .out()\n``', 'code');
  assert.equal(open.text, 'g.v()\n  .out()\n``');
  // A longer closing fence closes, a shorter one does not.
  assert.equal(only('````\na\n```\nb\n`````', 'code').text, 'a\n```\nb');
});

test('raw HTML stays text: no node carries markup', () => {
  const text = '<script>alert(1)</script> and <img src=x onerror=alert(1)>\n\n| <b>x</b> |\n|---|\n| <i>y</i> |';
  const nodes = walk(parseMarkdown(text));
  const texts = nodes.filter((n) => n.type === 'text').map((n) => n.text).join('');
  assert.ok(texts.includes('<script>alert(1)</script>'), texts);
  assert.ok(texts.includes('<img src=x onerror=alert(1)>'), texts);
  assert.ok(texts.includes('<b>x</b>') && texts.includes('<i>y</i>'), texts);
  const types = new Set(nodes.map((n) => n.type).filter(Boolean));
  for (const t of types) {
    assert.ok(['paragraph', 'table', 'text', 'code', 'strong', 'em', 'del', 'link', 'br', 'heading', 'list', 'quote', 'hr'].includes(t), t);
  }
});

test('links: only http and https become links, everything else is text', () => {
  const links = (s) => walk(parseInlines(s)).filter((n) => n.type === 'link');
  assert.equal(links('[book](https://example.org/a)')[0].href, 'https://example.org/a');
  assert.equal(links('[x](http://localhost:8080/)')[0].href, 'http://localhost:8080/');
  for (const bad of ['javascript:alert(1)', 'JAVASCRIPT:alert(1)', 'data:text/html,x', 'vbscript:x', 'file:///etc/passwd', '/relative', '//evil.org']) {
    const nodes = parseInlines(`[click](${bad})`);
    assert.equal(walk(nodes).filter((n) => n.type === 'link').length, 0, bad);
    assert.equal(inlineText(nodes), 'click', bad);
  }
  assert.equal(links('see <https://x.org/y>')[0].href, 'https://x.org/y');
  const bare = links('at https://x.org/a_b. Done');
  assert.equal(bare[0].children[0].text, 'https://x.org/a_b');
  assert.equal(safeHref('https://x.org'), 'https://x.org/');
  assert.equal(safeHref(' javascript:1'), null);
  assert.equal(safeHref('https://'), null);
});

test('inlines: emphasis, code, escapes; intraword underscores stay text', () => {
  const nodes = parseInlines('**bold** *em* ~~gone~~ `has_label("x")` has_label and snake_case_name \\*not em\\*');
  assert.deepEqual(nodes.map((n) => n.type), ['strong', 'text', 'em', 'text', 'del', 'text', 'code', 'text']);
  assert.equal(nodes[6].text, 'has_label("x")');
  assert.equal(nodes[7].text, ' has_label and snake_case_name *not em*');
  assert.deepEqual(parseInlines('`` a ` b ``'), [{ type: 'code', text: 'a ` b' }]);
  assert.deepEqual(parseInlines('2 * 3 * 4'), [{ type: 'text', text: '2 * 3 * 4' }]);
  assert.deepEqual(parseInlines('line one  \nline two').map((n) => n.type), ['text', 'br', 'text']);
  assert.equal(inlineText(parseInlines('soft\nbreak')), 'soft break');
});

test('blocks: headings, lists (nested, ordered with a start), quotes, rules, paragraphs', () => {
  const blocks = parseMarkdown('# Title\n\nSome text\nmore text\n\n- one\n- two\n  - nested\n\n3. c\n4. d\n\n> quoted\n\n---\nend');
  assert.deepEqual(blocks.map((b) => b.type), ['heading', 'paragraph', 'list', 'list', 'quote', 'hr', 'paragraph']);
  assert.equal(blocks[0].level, 1);
  assert.equal(inlineText(blocks[1].children), 'Some text more text');
  assert.equal(blocks[2].items.length, 2);
  assert.equal(blocks[2].items[1][1].type, 'list', 'the nested list is inside the second item');
  assert.equal(blocks[3].ordered, true);
  assert.equal(blocks[3].start, 3);
  assert.equal(blocks[4].children[0].type, 'paragraph');
  // A paragraph is interrupted by a fence, a heading and a table.
  assert.deepEqual(parseMarkdown('text\n```\ncode\n```').map((b) => b.type), ['paragraph', 'code']);
  assert.deepEqual(parseMarkdown('text\n| a |\n|---|\n| 1 |').map((b) => b.type), ['paragraph', 'table']);
  // Deep nesting is bounded (no runaway recursion on hostile input).
  assert.ok(parseMarkdown('>'.repeat(5000) + ' x').length >= 1);
  assert.ok(parseMarkdown('['.repeat(5000) + 'x' + ']'.repeat(5000)).length === 1);
  assert.ok(parseMarkdown('*'.repeat(3000) + 'x').length === 1);
});
