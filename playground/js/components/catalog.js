// The Catalog: everything stored in the graph besides its data and schema (the catalog of
// definitions: saved queries, compression rules, later indexes ...).
//
// - `CatalogMenu`: the top bar's "Catalog ▾" dropdown. One item per kind with its count, each
//   opening that kind's manager, and the catalog file: "Save catalog (.json)" downloads every
//   kind as one file (`graphersal-catalog-<stamp>.json`), "Load catalog…" stores a file's
//   definitions (a compression rule re-encodes its values).
// - `CatalogDialogs`: the managers, hosted once by the workspace and opened through
//   `openCatalog(id)` (store.js).
//   * Saved queries: the queries by folder with Run, Edit, Delete and New (the editor and run
//     form are js/components/saved-queries.js's dialog).
//   * Compression: the rules with their stats (values, plain -> stored size, ratio, saving,
//     dictionary) with Edit, Recompress and Drop, and the form (name, element, label with the
//     graph's labels as suggestions, path, minimum size, dictionary) with the engine's field
//     errors next to their inputs.
// The models are js/compression.js and js/saved-queries.js (tested under Node).

import { ref, reactive, computed, watch, nextTick } from 'vue';
import {
  state, toast, openCatalog, saveCatalogFile, loadCatalogFile, refreshSavedQueries, refreshCompressions,
  defineCompressionRule, dropCompressionRule, recompressRule, dropSavedQuery,
} from '../store.js';
import {
  catalogItems, ruleRow, totals, newDraft, draftFromRule, draftToSpec, labelSuggestions,
} from '../compression.js';
import { queryTree, signatureText, errorsByField } from '../saved-queries.js';
import { formatBytes } from '../statistics.js';
import { DropMenu } from './drop-menu.js';
import { SavedQueryDialog } from './saved-queries.js';

const message = (err) => (err && err.message ? err.message : String(err));

/** The top bar's "Catalog ▾" dropdown. */
export const CatalogMenu = {
  components: { DropMenu },
  setup() {
    const fileInput = ref(null);
    const items = computed(() => catalogItems(state.graph));
    function pickFile(close) {
      close();
      if (fileInput.value) fileInput.value.click();
    }
    async function onFile(e) {
      const file = e.target.files && e.target.files[0];
      e.target.value = '';
      if (file) await loadCatalogFile(file);
    }
    return { state, items, fileInput, pickFile, onFile, openCatalog, saveCatalogFile };
  },
  template: `
  <drop-menu button-class="btn" title="The catalog: saved queries and compression rules stored in the graph" menu-style="min-width: 280px">
    <template #trigger>Catalog ▾</template>
    <template #default="{ close }">
      <div class="menu-title">Stored in the graph</div>
      <button v-for="item in items" :key="item.id" class="menu-item" :title="item.hint" @click="openCatalog(item.id)">
        {{ item.title }}<span class="spacer"></span><span class="badge">{{ item.count }}</span>
      </button>
      <div class="menu-sep"></div>
      <div class="menu-title">Catalog file (no data inside)</div>
      <button class="menu-item" :disabled="state.saving || state.running" @click="saveCatalogFile()"
              title="Every definition of the catalog (saved queries, compression rules) as one JSON file (graphersal-catalog-YYYYMMDD-HHMMSS.json)">Save catalog (.json)</button>
      <button class="menu-item" :disabled="state.loadingGraph || state.running" @click.stop="pickFile(close)"
              title="Store the definitions of a catalog file in this graph: one of the same kind and name is replaced; a compression rule compresses its values at once">Load catalog…</button>
    </template>
  </drop-menu>
  <input ref="fileInput" type="file" accept=".json,application/json" hidden @change="onFile">
  `,
};

/** The Saved queries manager: the list by folder with Run, Edit, Delete and New. */
export const SavedQueriesManager = {
  components: { SavedQueryDialog },
  setup(props, { expose }) {
    const dlg = ref(null);
    const editor = ref(null);
    const tree = computed(() => queryTree(state.savedQueries || []));
    const count = computed(() => (state.savedQueries || []).length);
    async function open() {
      await refreshSavedQueries();
      if (dlg.value && !dlg.value.open) dlg.value.showModal();
    }
    function close() {
      if (dlg.value && dlg.value.open) dlg.value.close();
    }
    // The editor and run form are a dialog of their own: this one closes first.
    function openNew() { close(); editor.value.openNew(''); }
    function openEdit(q) { close(); editor.value.openEdit(q.name); }
    function openRun(q) { close(); editor.value.openRun(q.name); }
    async function remove(q) {
      if (!window.confirm(`Delete the saved query "${q.name}"? Scripts that call it fail afterwards.`)) return;
      try {
        await dropSavedQuery(q.name);
        toast(`Saved query "${q.name}" deleted.`, 'info', 4000);
      } catch (err) {
        toast(`Delete failed: ${message(err)}`, 'error', 9000);
      }
    }
    const tooltip = (q) => [signatureText(q), q.description, q.status === 'error' ? `Fails: ${q.error}` : null]
      .filter(Boolean).join('\n');
    expose({ open, close });
    return { state, dlg, editor, tree, count, close, openNew, openEdit, openRun, remove, tooltip };
  },
  template: `
  <dialog ref="dlg" class="catalog-dialog" aria-labelledby="cq-title">
    <div class="dialog-head">
      <h2 id="cq-title">Saved queries</h2><span class="spacer"></span>
      <button type="button" class="icon-btn" @click="close" aria-label="Close">×</button>
    </div>
    <div class="dialog-body">
      <p class="muted sq-note">Named, parameterized, read-only queries stored in the graph; call one with g.query("name", #{…}) or Run it here. They travel with the catalog file (Catalog ▸ Save catalog) and with a snapshot.</p>
      <div v-if="state.savedQueriesError" class="field-error">{{ state.savedQueriesError }}</div>
      <p v-else-if="!count" class="muted">No saved queries yet.</p>
      <template v-for="grp in tree" :key="grp.folder">
        <h3 v-if="grp.folder">{{ grp.folder }}</h3>
        <div v-for="q in grp.queries" :key="q.name" class="catalog-row">
          <span class="code" :title="tooltip(q)">{{ q.name }}</span>
          <span v-if="q.status === 'error'" class="badge danger" title="Its body does not compile any more">error</span>
          <span v-if="q.description" class="faint sq-desc">{{ q.description }}</span>
          <span class="spacer"></span>
          <button type="button" class="btn small" @click="openRun(q)">Run…</button>
          <button type="button" class="btn small" @click="openEdit(q)">Edit…</button>
          <button type="button" class="btn small danger" @click="remove(q)" :aria-label="'Delete ' + q.name">Delete</button>
        </div>
      </template>
      <div class="sq-buttons">
        <button type="button" class="btn primary" @click="openNew">New saved query…</button>
        <span class="spacer"></span>
        <button type="button" class="btn" @click="close">Close</button>
      </div>
    </div>
  </dialog>
  <saved-query-dialog ref="editor"></saved-query-dialog>
  `,
};

/** The Compression manager: the rules with their stats, and the rule form. */
export const CompressionManager = {
  setup(props, { expose }) {
    const dlg = ref(null);
    const d = reactive({
      draft: null, // the form's draft (null: the list)
      errors: {}, // field -> [messages]
      busy: false,
      labels: { vertex: [], edge: [] },
    });
    const rows = computed(() => (state.compressions || []).map((rule) => ({ rule, row: ruleRow(rule) })));
    const sum = computed(() => totals(state.compressions || []));
    const suggestions = computed(() => (d.draft ? d.labels[d.draft.element] || [] : []));

    async function loadLabels() {
      try {
        const stats = await state.engine.statistics();
        d.labels = { vertex: labelSuggestions(stats, 'vertex'), edge: labelSuggestions(stats, 'edge') };
      } catch (err) {
        d.labels = { vertex: [], edge: [] };
      }
    }
    async function open() {
      Object.assign(d, { draft: null, errors: {}, busy: false });
      await refreshCompressions();
      if (dlg.value && !dlg.value.open) dlg.value.showModal();
    }
    function close() {
      if (dlg.value && dlg.value.open) dlg.value.close();
    }
    async function startNew() {
      Object.assign(d, { draft: newDraft(), errors: {} });
      await loadLabels();
      await nextTick();
    }
    async function startEdit(rule) {
      Object.assign(d, { draft: draftFromRule(rule), errors: {} });
      await loadLabels();
    }
    function cancelForm() {
      Object.assign(d, { draft: null, errors: {} });
    }
    async function save() {
      if (d.busy || !d.draft) return;
      const local = draftToSpec(d.draft);
      if (local.errors) {
        d.errors = errorsByField(local.errors);
        return;
      }
      d.busy = true;
      try {
        const res = await defineCompressionRule(local.spec);
        if (res && res.ok) {
          const r = res.rule && res.rule.stats;
          toast(`Compression rule "${local.spec.name}" saved${r ? `: ${r.compressedValues} value${r.compressedValues === 1 ? '' : 's'} compressed` : ''}.`, 'info', 5000);
          cancelForm();
        } else {
          d.errors = errorsByField((res && res.errors) || [{ field: 'general', message: 'The engine refused it.' }]);
        }
      } catch (err) {
        d.errors = { general: [message(err)] };
      } finally {
        d.busy = false;
      }
    }
    async function recompress(rule) {
      if (d.busy) return;
      d.busy = true;
      try {
        const entry = await recompressRule(rule.name);
        const s = entry && entry.stats;
        toast(`Recompressed "${rule.name}"${s ? `: ${s.compressedValues} values, ${formatBytes(s.storedBytes)} stored` : ''}.`, 'info', 5000);
      } catch (err) {
        toast(`Recompress failed: ${message(err)}`, 'error', 9000);
      } finally {
        d.busy = false;
      }
    }
    async function drop(rule) {
      if (d.busy) return;
      if (!window.confirm(`Drop the compression rule "${rule.name}"? Its values are decompressed; no data changes.`)) return;
      d.busy = true;
      try {
        await dropCompressionRule(rule.name);
        toast(`Compression rule "${rule.name}" dropped.`, 'info', 4000);
      } catch (err) {
        toast(`Drop failed: ${message(err)}`, 'error', 9000);
      } finally {
        d.busy = false;
      }
    }
    expose({ open, close });
    return {
      state, dlg, d, rows, sum, suggestions, formatBytes, close, startNew, startEdit, cancelForm, save, recompress, drop,
    };
  },
  template: `
  <dialog ref="dlg" class="catalog-dialog wide" aria-labelledby="cc-title">
    <div class="dialog-head">
      <h2 id="cc-title">Compression</h2><span class="spacer"></span>
      <button type="button" class="icon-btn" @click="close" aria-label="Close">×</button>
    </div>
    <div class="dialog-body">
      <template v-if="!d.draft">
        <p class="muted sq-note">A rule keeps the strings of one vertex or edge label at one property path LZ4-compressed in memory (when at least the minimum size and at most 90 % after compression). Reads are always plain; a read or a has() on such a field decompresses it. Files hold plain text. Rules travel with the catalog file and with a snapshot.</p>
        <div v-if="state.compressionsError" class="field-error">{{ state.compressionsError }}</div>
        <p v-else-if="!rows.length" class="muted">No compression rules yet. Large text that queries rarely read (a CV, a description, a document body) is the use case.</p>
        <div v-else class="catalog-table-wrap">
          <table class="data-table catalog-table">
            <thead><tr>
              <th>Name</th><th>Applies to</th><th>Path</th><th>Min. size</th><th>Dictionary</th>
              <th class="num">Values</th><th>Plain → stored</th><th class="num">Ratio</th><th class="num">Saved</th><th></th>
            </tr></thead>
            <tbody>
              <tr v-for="{ rule, row } in rows" :key="rule.name">
                <td class="code">{{ row.name }}</td><td>{{ row.target }}</td><td class="code">{{ row.path }}</td>
                <td>{{ row.threshold }}</td><td>{{ row.dictionary }}</td><td class="num">{{ row.values }}</td>
                <td>{{ row.sizes }}</td><td class="num">{{ row.ratio }}</td><td class="num">{{ row.saved }}</td>
                <td class="catalog-actions">
                  <button type="button" class="btn small" :disabled="d.busy || state.running" @click="startEdit(rule)">Edit…</button>
                  <button type="button" class="btn small" :disabled="d.busy || state.running" @click="recompress(rule)"
                          title="Rebuild the dictionary from the current values and re-encode them (after much data was added); changes no data">Recompress</button>
                  <button type="button" class="btn small danger" :disabled="d.busy || state.running" @click="drop(rule)">Drop</button>
                </td>
              </tr>
            </tbody>
          </table>
          <p class="muted sq-note">Together: {{ sum.compressedValues }} values, {{ formatBytes(sum.plainBytes) }} plain stored as {{ formatBytes(sum.storedBytes) }}, dictionaries {{ formatBytes(sum.dictionaryBytes) }}.</p>
        </div>
        <div class="sq-buttons">
          <button type="button" class="btn primary" :disabled="state.running" @click="startNew">New rule…</button>
          <span class="spacer"></span>
          <button type="button" class="btn" @click="close">Close</button>
        </div>
      </template>

      <form v-else @submit.prevent="save">
        <h3>{{ d.draft.replaces ? 'Edit rule "' + d.draft.replaces + '"' : 'New rule' }}</h3>
        <div class="form-grid">
          <label for="cc-name">Name</label>
          <input id="cc-name" type="text" v-model="d.draft.name" :class="{ invalid: d.errors.name }" placeholder="customer_cv" autocomplete="off" spellcheck="false">
          <div v-for="m in d.errors.name || []" class="field-error sq-span">{{ m }}</div>
          <label for="cc-element">Element</label>
          <select id="cc-element" v-model="d.draft.element" :class="{ invalid: d.errors.element }" style="justify-self:start">
            <option value="vertex">vertex</option><option value="edge">edge</option>
          </select>
          <div v-for="m in d.errors.element || []" class="field-error sq-span">{{ m }}</div>
          <label for="cc-label">Label</label>
          <input id="cc-label" type="text" list="cc-labels" v-model="d.draft.label" :class="{ invalid: d.errors.label }" placeholder="customer" autocomplete="off" spellcheck="false">
          <datalist id="cc-labels"><option v-for="l in suggestions" :key="l" :value="l"></option></datalist>
          <div v-for="m in d.errors.label || []" class="field-error sq-span">{{ m }}</div>
          <span class="hint">The label need not exist yet; the graph's {{ d.draft.element }} labels are suggested.</span>
          <label for="cc-path">Path</label>
          <input id="cc-path" type="text" v-model="d.draft.path" :class="{ invalid: d.errors.path }" placeholder="details.career.cv" autocomplete="off" spellcheck="false">
          <div v-for="m in d.errors.path || []" class="field-error sq-span">{{ m }}</div>
          <span class="hint">A property name or a path of names (cv, details.career.cv); no index such as [0].</span>
          <label for="cc-min">Minimum size</label>
          <input id="cc-min" type="text" inputmode="numeric" v-model="d.draft.minBytesText" :class="{ invalid: d.errors.minBytes }" style="width:14ch;justify-self:start">
          <div v-for="m in d.errors.minBytes || []" class="field-error sq-span">{{ m }}</div>
          <span class="hint">Bytes (UTF-8); shorter strings stay plain.</span>
          <label for="cc-dict">Dictionary</label>
          <span><input id="cc-dict" type="checkbox" v-model="d.draft.dictionary"> <label for="cc-dict" class="muted">built from a sample of the rule's values (better ratio for many similar texts)</label></span>
          <div v-for="m in d.errors.dictionary || []" class="field-error sq-span">{{ m }}</div>
        </div>
        <div v-for="m in [...(d.errors.general || []), ...(d.errors.codec || [])]" class="field-error">{{ m }}</div>
        <p class="muted sq-note">Saving re-encodes every value of the label at once (one commit); no data changes.</p>
        <div class="sq-buttons">
          <span class="spacer"></span>
          <button type="button" class="btn" @click="cancelForm">Back</button>
          <button type="submit" class="btn primary" :disabled="d.busy || state.running">{{ d.busy ? 'Saving…' : 'Save' }}</button>
        </div>
      </form>
    </div>
  </dialog>
  `,
};

/** The managers of the Catalog dropdown, opened through `openCatalog(id)`. */
export const CatalogDialogs = {
  components: { SavedQueriesManager, CompressionManager },
  setup() {
    const queries = ref(null);
    const compression = ref(null);
    watch(() => state.catalogDialogN, async () => {
      const id = state.catalogDialog;
      state.catalogDialog = null;
      await nextTick();
      if (id === 'queries' && queries.value) await queries.value.open();
      if (id === 'compression' && compression.value) await compression.value.open();
    });
    return { queries, compression };
  },
  template: `
  <saved-queries-manager ref="queries"></saved-queries-manager>
  <compression-manager ref="compression"></compression-manager>
  `,
};
