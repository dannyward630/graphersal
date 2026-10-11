// "Share link" (the top bar's green button; also "Share as link…" in the Query panel's tab menu): a link that opens this tab's query together with
// the graph, for example under a bug report. An unchanged sample graph goes into the link by its
// name; any other graph goes into the link itself (GraphSON + schema, compressed; js/link.js), so
// only small graphs fit and the dialog asks for the smallest graph that shows the problem.
// Nothing is uploaded: the link is made in the page and holds everything after its `#`.

import { ref, computed } from 'vue';
import { state, makeShareLink } from '../store.js';
import { MAX_SHARE_LINK, MAX_SHARE_ELEMENTS } from '../link.js';
import { copyText } from '../util.js';

const SAMPLE_NAMES = { modern: 'Modern (TinkerPop)', empty: 'empty', large: 'large generated', tree: 'file tree' };
const fmt = (n) => Number(n).toLocaleString('en-US');

export const ShareDialog = {
  setup(props, { expose }) {
    const dlg = ref(null);
    const run = ref(true);
    const busy = ref(false);
    const error = ref(null);
    const link = ref(null);
    const copied = ref(false);
    let request = 0;

    async function refresh() {
      const mine = ++request;
      busy.value = true;
      error.value = null;
      copied.value = false;
      try {
        const made = await makeShareLink({ run: run.value });
        if (mine === request) link.value = made;
      } catch (err) {
        if (mine === request) {
          link.value = null;
          error.value = err && err.message ? err.message : String(err);
        }
      } finally {
        if (mine === request) busy.value = false;
      }
    }

    function open() {
      link.value = null;
      if (dlg.value && !dlg.value.open) dlg.value.showModal();
      refresh();
    }
    function close() {
      if (dlg.value) dlg.value.close();
    }
    async function copy() {
      if (!link.value || !link.value.fits) return;
      copied.value = await copyText(link.value.url);
      if (!copied.value) error.value = 'The browser blocked clipboard access: select the link below and copy it.';
    }

    const graphText = computed(() => {
      const l = link.value;
      if (!l) return '';
      if (l.sample) return `the ${SAMPLE_NAMES[l.sample] || l.sample} sample, unchanged: the link names it and carries no data.`;
      const what = `this graph's data: ${fmt(l.vertexCount)} vertices and ${fmt(l.edgeCount)} edges`;
      return l.schema ? `${what}, and its schema.` : `${what}.`;
    });
    const hasScript = computed(() => state.tab && state.tab.kind !== 'ai' && state.tab.script.trim() !== '');

    expose({ open, close });
    return {
      state, dlg, run, busy, error, link, copied, graphText, hasScript, refresh, close, copy, fmt,
      MAX_SHARE_LINK, MAX_SHARE_ELEMENTS, server: computed(() => !!(state.engineInfo && state.engineInfo.server)),
    };
  },
  template: `
  <dialog ref="dlg" aria-labelledby="share-title" @click.self="close" style="max-width:560px">
    <div class="dialog-head">
      <h2 id="share-title">Share as link</h2><span class="spacer"></span>
      <button type="button" class="icon-btn" @click="close" aria-label="Close">×</button>
    </div>
    <div class="dialog-body share-dialog">
      <p>The link opens <b>this tab's query</b> together with the graph. Everything is inside the link itself (after its
        <code>#</code>): nothing is uploaded, and anyone who has the link can read the graph in it.</p>
      <p class="share-note" role="note"><b>Only small graphs fit into a link</b> (at most {{ fmt(MAX_SHARE_LINK) }} characters,
        a graph of roughly a thousand elements). For a bug report, reduce the graph to the few vertices and edges that
        reproduce the problem and leave out everything else, then share it. Saved queries and compression rules are not
        included.</p>
      <p v-if="!hasScript" class="muted">This tab has no query: the link opens only the graph.</p>
      <p v-if="server" class="muted">The link opens the public playground (play.graphersal.dev), which runs in the
        reader's browser; this dev server stays private.</p>
      <p v-if="busy" class="muted">Making the link…</p>
      <template v-else-if="link">
        <p><b>Graph:</b> {{ graphText }}</p>
        <p v-if="link.tooMany" class="badge danger share-error">The graph has more than {{ fmt(MAX_SHARE_ELEMENTS) }} vertices and
          edges: far too large for a link. Reduce it to the elements that show the problem, or save it with
          Save ▸ Data (GraphSON) and attach the file instead.</p>
        <p v-else-if="!link.fits" class="badge danger share-error">Too large for a link: {{ fmt(link.chars) }} characters, at most
          {{ fmt(MAX_SHARE_LINK) }}. Reduce the graph to the elements that show the problem, or save it with Save ▸ Data
          (GraphSON) and attach the file instead.</p>
        <template v-else>
          <p class="muted">The link has {{ fmt(link.chars) }} characters.</p>
          <textarea class="share-url" readonly rows="3" :value="link.url" aria-label="The link" @focus="$event.target.select()"></textarea>
        </template>
      </template>
      <p v-if="error" class="badge danger share-error">{{ error }}</p>
      <label class="share-run"><input type="checkbox" v-model="run" @change="refresh" :disabled="!hasScript"> Run the query when the link is opened</label>
      <div style="display:flex;gap:8px;margin-top:18px">
        <span class="spacer"></span>
        <button type="button" class="btn" @click="close">Close</button>
        <button type="button" class="btn primary" @click="copy" :disabled="busy || !link || !link.fits">{{ copied ? 'Copied' : 'Copy link' }}</button>
      </div>
    </div>
  </dialog>
  `,
};
