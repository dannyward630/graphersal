// Renders an assistant answer of the AI Chat: the tree of js/markdown.js as Vue nodes. Every
// piece of model text becomes a TEXT node (never HTML), links are http/https only and open in a
// new tab with rel="noopener noreferrer". Tables scroll horizontally inside the message and copy
// as Markdown; query blocks (```graphersal, gremlin, groovy, rhai, or untagged starting with
// `g.`) offer Copy and Run (a new query tab that runs it), other code blocks Copy. The buttons
// show on hover and on keyboard focus.

import { h, ref, computed } from 'vue';
import { parseMarkdown, tableToMarkdown } from '../markdown.js';
import { copyText } from '../util.js';
import { openQueryTab } from '../store.js';

/** A Copy button with a short "Copied" feedback. */
const CopyButton = {
  props: { text: { type: String, required: true }, label: { type: String, default: 'Copy' } },
  setup(props) {
    const state = ref(''); // '' | 'copied' | 'failed'
    let timer = null;
    async function copy() {
      state.value = (await copyText(props.text)) ? 'copied' : 'failed';
      clearTimeout(timer);
      timer = setTimeout(() => { state.value = ''; }, 1500);
    }
    return () => h('button', {
      type: 'button',
      class: 'btn small md-btn',
      title: props.label,
      'aria-label': props.label,
      onClick: copy,
    }, state.value === 'copied' ? 'Copied' : state.value === 'failed' ? 'Blocked' : 'Copy');
  },
};

const CodeBlock = {
  props: { block: { type: Object, required: true } },
  setup(props) {
    function runIt() {
      openQueryTab({ title: 'From chat', script: props.block.text, execute: true });
    }
    return () => {
      const b = props.block;
      const tools = [h(CopyButton, { text: b.text, label: b.query ? 'Copy the query' : 'Copy the code' })];
      if (b.query) {
        tools.push(h('button', {
          type: 'button',
          class: 'btn small primary md-btn',
          title: 'Open the query in a new query tab and run it',
          'aria-label': 'Run the query in a new query tab',
          onClick: runIt,
        }, 'Run'));
      }
      return h('div', { class: ['md-code', b.query ? 'query' : ''] }, [
        h('div', { class: 'md-tools' }, [b.lang ? h('span', { class: 'md-lang' }, b.lang) : null, ...tools]),
        h('pre', { class: 'mono' }, [h('code', null, b.text)]),
      ]);
    };
  },
};

const TableBlock = {
  props: { block: { type: Object, required: true } },
  setup(props) {
    const markdown = computed(() => tableToMarkdown(props.block));
    return () => {
      const b = props.block;
      const cell = (tag, c, i) => h(tag, { style: b.align[i] ? { textAlign: b.align[i] } : null }, inlineNodes(c.children));
      return h('div', { class: 'md-table' }, [
        h('div', { class: 'md-tools' }, [h(CopyButton, { text: markdown.value, label: 'Copy the table as Markdown' })]),
        h('div', { class: 'md-table-scroll', tabindex: 0, 'aria-label': 'Table (scrolls horizontally)' }, [
          h('table', null, [
            h('thead', null, [h('tr', null, b.header.map((c, i) => cell('th', c, i)))]),
            h('tbody', null, b.rows.map((row) => h('tr', null, row.map((c, i) => cell('td', c, i))))),
          ]),
        ]),
      ]);
    };
  },
};

function inlineNodes(nodes) {
  return (nodes || []).map((n) => {
    switch (n.type) {
      case 'text': return n.text;
      case 'code': return h('code', { class: 'md-inline-code' }, n.text);
      case 'strong': return h('strong', null, inlineNodes(n.children));
      case 'em': return h('em', null, inlineNodes(n.children));
      case 'del': return h('del', null, inlineNodes(n.children));
      case 'br': return h('br');
      case 'link': return h('a', { href: n.href, target: '_blank', rel: 'noopener noreferrer' }, inlineNodes(n.children));
      default: return '';
    }
  });
}

function blockNodes(blocks) {
  return (blocks || []).map((b) => {
    switch (b.type) {
      case 'paragraph': return h('p', null, inlineNodes(b.children));
      case 'heading': return h(`h${Math.min(6, Math.max(1, b.level))}`, { class: 'md-h' }, inlineNodes(b.children));
      case 'hr': return h('hr');
      case 'quote': return h('blockquote', null, blockNodes(b.children));
      case 'code': return h(CodeBlock, { block: b });
      case 'table': return h(TableBlock, { block: b });
      case 'list': return h(b.ordered ? 'ol' : 'ul', b.ordered && b.start !== 1 ? { start: b.start } : null,
        b.items.map((item) => h('li', null,
          item.length === 1 && item[0].type === 'paragraph' ? inlineNodes(item[0].children) : blockNodes(item))));
      default: return null;
    }
  });
}

export const MarkdownView = {
  props: { text: { type: String, default: '' } },
  setup(props) {
    const blocks = computed(() => parseMarkdown(props.text));
    return () => h('div', { class: 'md' }, blockNodes(blocks.value));
  },
};
