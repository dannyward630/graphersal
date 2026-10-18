// Syntax highlighting of the Graphersal DSL (Gremlin in Rhai) in the book's code blocks.
//
// mdBook bundles highlight.js 10 without a Rhai grammar, so its book.js leaves ```rhai blocks
// plain (and stops at the first one with an "Unknown language" error). This script registers the
// language `rhai` and highlights every code block that is not highlighted yet. It runs after
// book.js (mdBook loads `additional-js` last).
(function () {
  'use strict';
  if (typeof hljs === 'undefined') return;

  hljs.registerLanguage('rhai', function (hljs) {
    return {
      name: 'Rhai',
      aliases: ['graphersal'],
      keywords: {
        keyword: 'let const fn if else for in while loop break continue return throw try catch ' +
          'import export as private switch do until',
        literal: 'true false',
        built_in: 'g __ GraphSource P TextP T Order Scope Column Direction Cardinality Pop GType ' +
          'Merge Operator Barrier By Pick DT SchemaMode CastPolicy ProfileType UUID Set Fake',
      },
      contains: [
        hljs.C_LINE_COMMENT_MODE,
        hljs.C_BLOCK_COMMENT_MODE,
        hljs.QUOTE_STRING_MODE,
        { className: 'string', begin: '`', end: '`', contains: [hljs.BACKSLASH_ESCAPE] },
        { className: 'number', begin: '\\b\\d+(\\.\\d+)?([eE][-+]?\\d+)?\\b', relevance: 0 },
        // a step or function call: `.has_label(`, `count(`
        { className: 'title', begin: '\\b[A-Za-z_][A-Za-z0-9_]*(?=\\()', relevance: 0 },
        // a map literal's opening `#{`
        { className: 'meta', begin: '#\\{', relevance: 0 },
      ],
    };
  });

  function highlightRest() {
    document.querySelectorAll('pre code').forEach(function (block) {
      if (block.classList.contains('hljs') && block.querySelector('span')) return;
      var lang = null;
      block.classList.forEach(function (c) {
        if (c.indexOf('language-') === 0) lang = c.slice('language-'.length);
      });
      if (lang === null || !hljs.getLanguage(lang)) return;
      try {
        hljs.highlightBlock(block);
      } catch (e) {
        /* a block the grammar cannot read stays plain */
      }
    });
  }

  // The page's code blocks are parsed already (mdBook loads its scripts at the end of the body);
  // this runs before playground-links.js adds its run buttons to the blocks.
  highlightRest();
})();
