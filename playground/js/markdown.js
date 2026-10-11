// A small, safe Markdown renderer for the AI Chat's assistant answers (pure, no DOM: it returns
// a tree of plain objects that components/markdown-view.js turns into Vue nodes, every piece of
// text as a TEXT node). The text is untrusted (a model may echo graph data): nothing here ever
// becomes HTML, raw HTML in the text stays visible text, and links keep only http/https URLs.
// Tested under Node (playground/tests/markdown.test.mjs).
//
// The subset: paragraphs, headings (#), emphasis (* _ ** __, ~~), inline code, links
// ([text](url), <url>, bare http(s) URLs), ordered and unordered lists (nested by indentation),
// block quotes, thematic breaks, fenced code blocks (``` and ~~~) and GFM tables (alignment row,
// escaped pipes). Setext headings, indented code, HTML blocks, reference links and images are not
// part of it (their text is shown as written).
//
// Blocks: {type: 'paragraph'|'heading', level?, children}, {type: 'list', ordered, start,
// items: [blocks]}, {type: 'quote', children: blocks}, {type: 'hr'}, {type: 'code', lang, text,
// query}, {type: 'table', align: ['left'|'center'|'right'|null], header: [cell], rows: [[cell]]}
// where a cell is {text (its source with `\|` unescaped), children}.
// Inlines: {type: 'text', text}, {type: 'code', text}, {type: 'strong'|'em'|'del', children},
// {type: 'link', href, children}, {type: 'br'}.

/** Info strings of a fenced block that make it a query (Run offered). */
export const QUERY_LANGS = Object.freeze(['graphersal', 'gremlin', 'groovy', 'rhai']);

const MAX_DEPTH = 8; // nested lists / quotes deeper than this are shown as text

/** Whether a fenced block is a query: a query info string, or untagged text starting with `g.`. */
export function isQueryBlock(lang, text) {
  const l = String(lang || '').trim().toLowerCase();
  if (l) return QUERY_LANGS.includes(l);
  return /^g\./.test(String(text || '').trimStart());
}

/** Whether `href` is a link the chat may open: an absolute http or https URL. */
export function safeHref(href) {
  const h = String(href || '').trim();
  if (!/^https?:\/\//i.test(h)) return null;
  try {
    const url = new URL(h);
    return url.protocol === 'http:' || url.protocol === 'https:' ? url.href : null;
  } catch (e) {
    return null;
  }
}

// ------------------------------------------------------------------ blocks

const FENCE = /^ {0,3}(`{3,}|~{3,})\s*([^`\s]*)[^`]*$/;
const HEADING = /^ {0,3}(#{1,6})(?:[ \t]+(.*?))?(?:[ \t]+#+)?[ \t]*$/;
const HR = /^ {0,3}([-*_])(?:[ \t]*\1){2,}[ \t]*$/;
const ITEM = /^( *)([-*+]|\d{1,9}[.)])([ \t]+|$)(.*)$/;
const QUOTE = /^ {0,3}> ?(.*)$/;
const DELIM_CELL = /^\s*:?-+:?\s*$/;

/** Parses Markdown text into blocks. */
export function parseMarkdown(text) {
  const lines = String(text ?? '').replace(/\r\n?/g, '\n').replace(/\t/g, '    ').split('\n');
  return parseBlocks(lines, 0);
}

function isBlank(line) {
  return !line || !line.trim();
}

/** Splits a table row on the pipes that are not escaped; the outer pipes are dropped. */
export function splitRow(line) {
  let s = line.trim();
  if (s.startsWith('|')) s = s.slice(1);
  if (s.endsWith('|') && !s.endsWith('\\|')) s = s.slice(0, -1);
  const cells = [];
  let cur = '';
  for (let i = 0; i < s.length; i++) {
    const c = s[i];
    if (c === '\\' && s[i + 1] === '|') {
      cur += '\\|';
      i++;
    } else if (c === '|') {
      cells.push(cur);
      cur = '';
    } else {
      cur += c;
    }
  }
  cells.push(cur);
  return cells.map((c) => c.trim());
}

function delimiterAlign(line) {
  if (!line.includes('-')) return null;
  const cells = splitRow(line);
  if (!cells.every((c) => DELIM_CELL.test(c))) return null;
  return cells.map((c) => {
    const left = c.startsWith(':');
    const right = c.endsWith(':');
    if (left && right) return 'center';
    if (right) return 'right';
    if (left) return 'left';
    return null;
  });
}

/** A table starting at `i` (header row + delimiter row), or null. */
function tableAt(lines, i) {
  const head = lines[i];
  const delim = lines[i + 1];
  if (head === undefined || delim === undefined || !head.includes('|')) return null;
  const align = delimiterAlign(delim);
  if (!align) return null;
  const header = splitRow(head);
  if (header.length !== align.length) return null;
  const cell = (raw) => {
    const t = raw.replace(/\\\|/g, '|');
    return { text: t, children: parseInlines(t) };
  };
  const fit = (cells) => {
    const out = cells.slice(0, align.length);
    while (out.length < align.length) out.push('');
    return out.map(cell);
  };
  const rows = [];
  let j = i + 2;
  for (; j < lines.length; j++) {
    const line = lines[j];
    if (isBlank(line) || !line.includes('|')) break;
    rows.push(fit(splitRow(line)));
  }
  return { block: { type: 'table', align, header: fit(header), rows }, next: j };
}

/** Whether `line` starts a block other than a paragraph (it interrupts a paragraph). */
function startsBlock(lines, i) {
  const line = lines[i];
  return FENCE.test(line) || HEADING.test(line) || HR.test(line) || QUOTE.test(line)
    || /^ {0,3}([-*+]|1[.)])[ \t]+\S/.test(line) || !!tableAt(lines, i);
}

function parseBlocks(lines, depth) {
  const blocks = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    if (isBlank(line)) {
      i++;
      continue;
    }
    let m = FENCE.exec(line);
    if (m) {
      const fence = m[1];
      const indent = line.length - line.trimStart().length;
      const lang = m[2] || '';
      const body = [];
      i++;
      while (i < lines.length) {
        const l = lines[i];
        const close = /^ {0,3}(`{3,}|~{3,})\s*$/.exec(l);
        if (close && close[1][0] === fence[0] && close[1].length >= fence.length) {
          i++;
          break;
        }
        // Strip the opening fence's indentation from the content, as CommonMark does.
        let k = 0;
        while (k < indent && l[k] === ' ') k++;
        body.push(l.slice(k));
        i++;
      }
      const text = body.join('\n');
      blocks.push({ type: 'code', lang, text, query: isQueryBlock(lang, text) });
      continue;
    }
    m = HEADING.exec(line);
    if (m) {
      blocks.push({ type: 'heading', level: m[1].length, children: parseInlines(m[2] || '') });
      i++;
      continue;
    }
    const table = tableAt(lines, i);
    if (table) {
      blocks.push(table.block);
      i = table.next;
      continue;
    }
    if (HR.test(line)) {
      blocks.push({ type: 'hr' });
      i++;
      continue;
    }
    if (QUOTE.test(line)) {
      const inner = [];
      while (i < lines.length && !isBlank(lines[i])) {
        const q = QUOTE.exec(lines[i]);
        inner.push(q ? q[1] : lines[i]);
        i++;
      }
      if (depth >= MAX_DEPTH) blocks.push({ type: 'paragraph', children: parseInlines(inner.join('\n')) });
      else blocks.push({ type: 'quote', children: parseBlocks(inner, depth + 1) });
      continue;
    }
    m = ITEM.exec(line);
    if (m && depth < MAX_DEPTH) {
      const parsed = parseList(lines, i, depth);
      blocks.push(parsed.block);
      i = parsed.next;
      continue;
    }
    // A paragraph: up to a blank line or a line that starts another block.
    const para = [line];
    i++;
    while (i < lines.length && !isBlank(lines[i]) && !startsBlock(lines, i)) {
      para.push(lines[i]);
      i++;
    }
    blocks.push({ type: 'paragraph', children: parseInlines(para.map((l) => l.trim()).join('\n')) });
  }
  return blocks;
}

function parseList(lines, start, depth) {
  const first = ITEM.exec(lines[start]);
  const ordered = /\d/.test(first[2]);
  const marker = ordered ? first[2].slice(-1) : first[2];
  const baseIndent = first[1].length;
  const items = [];
  let i = start;
  while (i < lines.length) {
    const m = ITEM.exec(lines[i]);
    if (!m || m[1].length !== baseIndent) break;
    const isOrdered = /\d/.test(m[2]);
    if (isOrdered !== ordered || (ordered ? m[2].slice(-1) : m[2]) !== marker) break;
    const contentIndent = baseIndent + m[2].length + Math.max(1, m[3].length);
    const body = [m[4]];
    i++;
    while (i < lines.length) {
      const l = lines[i];
      if (isBlank(l)) {
        // A blank line continues the item only when the next line is indented into it.
        const next = lines[i + 1];
        if (next !== undefined && !isBlank(next) && next.length - next.trimStart().length >= contentIndent) {
          body.push('');
          i++;
          continue;
        }
        break;
      }
      const indent = l.length - l.trimStart().length;
      if (indent >= contentIndent) {
        body.push(l.slice(contentIndent));
      } else if (indent > baseIndent && ITEM.test(l)) {
        body.push(l.slice(Math.min(indent, contentIndent)));
      } else if (!ITEM.test(l) && !startsBlock(lines, i)) {
        body.push(l.trim()); // a lazy continuation line
      } else {
        break;
      }
      i++;
    }
    items.push(parseBlocks(body, depth + 1));
  }
  // Skip the blank lines between this list and what follows.
  return {
    block: { type: 'list', ordered, start: ordered ? parseInt(first[2], 10) : 1, items },
    next: i,
  };
}

// ------------------------------------------------------------------ inlines

const PUNCT = /[!-/:-@[-`{-~]/;
const ALNUM = /[\p{L}\p{N}]/u;
const BARE_URL = /^https?:\/\/[^\s<>"'`]+/i;

/** Parses inline Markdown into inline nodes. */
export function parseInlines(text) {
  return inlines(String(text ?? ''), 0);
}

function pushText(out, text) {
  if (!text) return;
  const last = out[out.length - 1];
  if (last && last.type === 'text') last.text += text;
  else out.push({ type: 'text', text });
}

function inlines(s, depth) {
  const out = [];
  let i = 0;
  while (i < s.length) {
    const c = s[i];
    if (c === '\\' && i + 1 < s.length) {
      if (s[i + 1] === '\n') {
        out.push({ type: 'br' });
        i += 2;
        continue;
      }
      if (PUNCT.test(s[i + 1])) {
        pushText(out, s[i + 1]);
        i += 2;
        continue;
      }
    }
    if (c === '\n') {
      // A hard break after two trailing spaces; otherwise a soft break (a space).
      if (/ {2,}$/.test(s.slice(0, i))) {
        const last = out[out.length - 1];
        if (last && last.type === 'text') last.text = last.text.replace(/ +$/, '');
        out.push({ type: 'br' });
      } else {
        pushText(out, ' ');
      }
      i++;
      continue;
    }
    if (c === '`') {
      let n = 0;
      while (s[i + n] === '`') n++;
      const ticks = '`'.repeat(n);
      let j = s.indexOf(ticks, i + n);
      while (j >= 0 && s[j + n] === '`') {
        let k = j;
        while (s[k] === '`') k++;
        j = s.indexOf(ticks, k);
      }
      if (j >= 0) {
        let code = s.slice(i + n, j).replace(/\n/g, ' ');
        if (code.length > 2 && code.startsWith(' ') && code.endsWith(' ') && code.trim()) code = code.slice(1, -1);
        out.push({ type: 'code', text: code });
        i = j + n;
        continue;
      }
      pushText(out, ticks);
      i += n;
      continue;
    }
    if (c === '[' && depth < MAX_DEPTH) {
      const link = linkAt(s, i, depth);
      if (link) {
        for (const node of link.nodes) {
          if (node.type === 'text') pushText(out, node.text);
          else out.push(node);
        }
        i = link.next;
        continue;
      }
    }
    if (c === '<') {
      const m = /^<(https?:\/\/[^\s<>]+)>/i.exec(s.slice(i));
      if (m && safeHref(m[1])) {
        out.push({ type: 'link', href: safeHref(m[1]), children: [{ type: 'text', text: m[1] }] });
        i += m[0].length;
        continue;
      }
    }
    if ((c === 'h' || c === 'H') && (i === 0 || !ALNUM.test(s[i - 1]))) {
      const m = BARE_URL.exec(s.slice(i));
      if (m) {
        const url = m[0].replace(/[.,;:!?)\]}*_~]+$/, '');
        const href = safeHref(url);
        if (href) {
          out.push({ type: 'link', href, children: [{ type: 'text', text: url }] });
          i += url.length;
          continue;
        }
      }
    }
    if ((c === '*' || c === '_' || c === '~') && depth < MAX_DEPTH) {
      const em = emphasisAt(s, i, depth);
      if (em) {
        out.push(em.node);
        i = em.next;
        continue;
      }
    }
    pushText(out, c);
    i++;
  }
  return out;
}

/** `[text](url)` at `i`; a link with an unsafe URL (not http/https) becomes its plain text. */
function linkAt(s, i, depth) {
  let level = 0;
  let j = i;
  for (; j < s.length; j++) {
    if (s[j] === '\\') {
      j++;
      continue;
    }
    if (s[j] === '`') {
      const end = s.indexOf('`', j + 1);
      if (end > 0) j = end;
      continue;
    }
    if (s[j] === '[') level++;
    else if (s[j] === ']' && --level === 0) break;
  }
  if (j >= s.length || s[j + 1] !== '(') return null;
  const m = /^\(\s*<?([^\s()<>]*(?:\([^\s()<>]*\)[^\s()<>]*)*)>?(?:\s+"[^"]*")?\s*\)/.exec(s.slice(j + 1));
  if (!m) return null;
  const children = inlines(s.slice(i + 1, j), depth + 1);
  const href = safeHref(m[1]);
  const next = j + 1 + m[0].length;
  if (!href) return { nodes: children.length ? children : [{ type: 'text', text: m[1] }], next };
  return { nodes: [{ type: 'link', href, children }], next };
}

/** `*x*`, `_x_`, `**x**`, `__x__`, `~~x~~` at `i`. Intraword `_` is never emphasis. */
function emphasisAt(s, i, depth) {
  const c = s[i];
  let n = 0;
  while (s[i + n] === c) n++;
  if (c === '~' && n !== 2) return null;
  const size = c === '~' ? 2 : n >= 2 ? 2 : 1;
  const prev = i > 0 ? s[i - 1] : '';
  const after = s[i + size];
  if (!after || /\s/.test(after)) return null; // not left-flanking
  if (c === '_' && prev && ALNUM.test(prev)) return null;
  const marker = c.repeat(size);
  let j = i + size;
  while (j < s.length) {
    const k = s.indexOf(marker, j);
    if (k < 0) return null;
    if (s[k - 1] === '\\') {
      j = k + 1;
      continue;
    }
    const before = s[k - 1];
    const next = s[k + size];
    const closes = k > i + size && before && !/\s/.test(before)
      && (c !== '_' || !next || !ALNUM.test(next))
      && (size === 2 || s[k + 1] !== c || s[k + 2] === c);
    if (closes) {
      const type = c === '~' ? 'del' : size === 2 ? 'strong' : 'em';
      return { node: { type, children: inlines(s.slice(i + size, k), depth + 1) }, next: k + size };
    }
    j = k + 1;
  }
  return null;
}

// ------------------------------------------------------------------ copy as Markdown

/** One cell for a GFM table: one line, pipes escaped. */
function gfmCell(text) {
  return String(text ?? '').replace(/\r?\n/g, ' ').replace(/\|/g, '\\|').trim();
}

/**
 * A parsed table as normalised GFM text: the header row, the alignment row and the rows, every
 * row with the header's number of cells and every `|` inside a cell escaped.
 */
export function tableToMarkdown(table) {
  const line = (cells) => `| ${cells.map((c) => gfmCell(c.text)).join(' | ')} |`;
  const alignCell = (a) => (a === 'center' ? ':---:' : a === 'right' ? '---:' : a === 'left' ? ':---' : '---');
  const out = [line(table.header), `| ${table.align.map(alignCell).join(' | ')} |`];
  for (const row of table.rows) out.push(line(row));
  return out.join('\n') + '\n';
}

/** The plain text of inline nodes (for titles and tests). */
export function inlineText(nodes) {
  return (nodes || []).map((n) => {
    if (n.type === 'text' || n.type === 'code') return n.text;
    if (n.type === 'br') return '\n';
    return inlineText(n.children);
  }).join('');
}
