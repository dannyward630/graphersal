// The dialog of a saved query, opened from the Catalog dropdown's Saved queries manager
// (js/components/catalog.js), with two modes:
// - EDIT (create or change): name, folder, description, the parameter list (name, type,
//   constraints, default, description; add, remove, reorder) and the function body in a code
//   editor with the DSL completion. Save calls defineQuery and shows the engine's validation
//   errors next to their field; Delete asks first; moving a query = changing its folder.
// - RUN: a form built from the parameter schemas (number fields with min/max, a select for an
//   enum, a checkbox for a boolean, UUID text, one line per item of a list; defaults prefilled,
//   required fields marked, descriptions as hints). Run WRITES `g.query("name", #{..})` into a
//   tab and runs it there (visible, editable, in the history); the last values per query are
//   kept in localStorage.
// The models are js/saved-queries.js (tested under Node); the engine calls go through store.js.

import { ref, reactive, computed, nextTick, onBeforeUnmount, markRaw } from 'vue';
import { EditorState } from '@codemirror/state';
import { EditorView, keymap, lineNumbers, highlightActiveLine, drawSelection } from '@codemirror/view';
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
import { bracketMatching, indentOnInput } from '@codemirror/language';
import { autocompletion, closeBrackets, closeBracketsKeymap, completionKeymap } from '@codemirror/autocomplete';
import {
  state, toast, defineSavedQuery, dropSavedQuery, runSavedQuery,
} from '../store.js';
import { storage } from '../util.js';
import {
  PARAM_TYPES, ITEM_TYPES, newParam, newDraft, draftFromQuery, draftToSpec,
  errorsByField, formFields, initialValues, formArgs, rememberValues, formatExample,
} from '../saved-queries.js';
import { dslLanguage, dslHighlight } from '../editor/dsl-language.js';
import { dslCompletionSource } from '../editor/completion.js';
import { loadCompletionData } from '../editor/completion-data.js';
import { floatingTooltips } from '../editor/tooltips.js';
import { editorTheme } from './query-editor.js';

const VALUES_KEY = 'savedQueryValues';
let completionData = null; // the DSL completion data (catalogue and token values), loaded once

/** The dialog of a saved query: edit (new or existing) and run. */
export const SavedQueryDialog = {
  setup(props, { expose }) {
    const dlg = ref(null);
    const host = ref(null);
    const d = reactive({
      mode: null, // 'edit' | 'run'
      draft: null,
      entry: null, // the run mode's query
      fields: [],
      values: {},
      errors: {}, // field -> [messages] (edit) or name -> message (run)
      busy: false,
      loading: false,
    });
    let view = null;

    const message = (err) => (err && err.message ? err.message : String(err));

    function destroyEditor() {
      if (view) view.destroy();
      view = null;
    }

    async function mountEditor() {
      await nextTick();
      destroyEditor();
      if (!host.value || !d.draft) return;
      if (!completionData) completionData = markRaw(await loadCompletionData(state.engine));
      view = markRaw(new EditorView({
        parent: host.value,
        state: EditorState.create({
          doc: d.draft.body,
          extensions: [
            lineNumbers(), history(), drawSelection(), indentOnInput(), bracketMatching(), closeBrackets(),
            highlightActiveLine(),
            floatingTooltips(host.value),
            autocompletion({
              override: [dslCompletionSource(() => completionData, () => state.savedQueries)],
              activateOnTyping: true,
              icons: true,
            }),
            keymap.of([...closeBracketsKeymap, ...defaultKeymap, ...historyKeymap, ...completionKeymap]),
            dslLanguage, dslHighlight, editorTheme, EditorView.lineWrapping,
            EditorView.contentAttributes.of({ 'aria-label': 'Body of the saved query' }),
            EditorView.updateListener.of((u) => {
              if (u.docChanged && d.draft) d.draft.body = u.state.doc.toString();
            }),
          ],
        }),
      }));
    }

    function show() {
      if (dlg.value && !dlg.value.open) dlg.value.showModal();
    }

    function close() {
      destroyEditor();
      if (dlg.value && dlg.value.open) dlg.value.close();
      d.mode = null;
    }

    /** "New saved query" (in `folder`). */
    async function openNew(folder = '') {
      Object.assign(d, { mode: 'edit', draft: newDraft(folder), entry: null, errors: {}, busy: false });
      show();
      await mountEditor();
    }

    async function loadEntry(name) {
      d.loading = true;
      try {
        const entry = await state.engine.getQuery(name);
        if (!entry) throw new Error(`the saved query "${name}" no longer exists`);
        return entry;
      } finally {
        d.loading = false;
      }
    }

    /** "Edit" of a stored query. */
    async function openEdit(name) {
      try {
        const entry = await loadEntry(name);
        Object.assign(d, { mode: 'edit', draft: draftFromQuery(entry), entry, errors: {}, busy: false });
        show();
        await mountEditor();
      } catch (err) {
        toast(`Cannot edit: ${message(err)}`, 'error', 8000);
      }
    }

    /** "Run…": the parameter form of a stored query. */
    async function openRun(name) {
      try {
        const entry = await loadEntry(name);
        destroyEditor();
        const fields = formFields(entry);
        const saved = (storage.get(VALUES_KEY, {}) || {})[name] || {};
        Object.assign(d, { mode: 'run', entry, draft: null, fields, values: initialValues(fields, saved), errors: {}, busy: false });
        show();
        if (!fields.length) return; // nothing to ask: the form shows Run only
      } catch (err) {
        toast(`Cannot run: ${message(err)}`, 'error', 8000);
      }
    }

    expose({ openNew, openEdit, openRun, close });

    // ---------------------------------------------------------------- edit
    const fieldErrors = computed(() => (d.mode === 'edit' ? d.errors : {}));
    const signature = computed(() => {
      if (!d.draft) return '';
      const params = d.draft.params.map((p) => p.name.trim() || '?').join(', ');
      return `fn ${d.draft.name.trim() || 'name'}(${params}) {`;
    });

    function addParam() {
      d.draft.params.push(newParam(`p${d.draft.params.length + 1}`));
    }
    function removeParam(i) {
      d.draft.params.splice(i, 1);
    }
    function moveParam(i, delta) {
      const j = i + delta;
      if (j < 0 || j >= d.draft.params.length) return;
      const [p] = d.draft.params.splice(i, 1);
      d.draft.params.splice(j, 0, p);
    }
    function paramErrors(param, index) {
      const e = fieldErrors.value;
      return [...(e[`params.${param.name.trim()}`] || []), ...(e[`params.${index}`] || [])];
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
        const res = await defineSavedQuery(local.spec);
        if (res && res.ok) {
          toast(`Saved query "${local.spec.name}" saved.`, 'info', 4000);
          close();
        } else {
          d.errors = errorsByField((res && res.errors) || [{ field: 'general', message: 'The engine refused it.' }]);
        }
      } catch (err) {
        d.errors = { general: [message(err)] };
      } finally {
        d.busy = false;
      }
    }

    async function remove() {
      const name = d.draft && d.draft.replaces;
      if (!name || d.busy) return;
      if (!window.confirm(`Delete the saved query "${name}"? Scripts that call it fail afterwards.`)) return;
      d.busy = true;
      try {
        await dropSavedQuery(name);
        toast(`Saved query "${name}" deleted.`, 'info', 4000);
        close();
      } catch (err) {
        d.errors = { general: [message(err)] };
      } finally {
        d.busy = false;
      }
    }

    // ---------------------------------------------------------------- run
    async function runNow() {
      if (d.busy || !d.entry) return;
      const { args, errors } = formArgs(d.fields, d.values);
      d.errors = errors;
      if (Object.keys(errors).length) return;
      const kept = Object.fromEntries(Object.entries(d.values).map(([k, v]) => [k, typeof v === "boolean" ? v : String(v ?? "")]));
      storage.set(VALUES_KEY, rememberValues(storage.get(VALUES_KEY, {}), d.entry.name, kept));
      d.busy = true;
      try {
        const name = d.entry.name;
        close();
        await runSavedQuery(name, args);
      } catch (err) {
        toast(`Run failed: ${message(err)}`, 'error', 9000);
      } finally {
        d.busy = false;
      }
    }

    function editFromRun() {
      const name = d.entry && d.entry.name;
      if (name) openEdit(name);
    }

    onBeforeUnmount(destroyEditor);

    return {
      state, dlg, host, d, PARAM_TYPES, ITEM_TYPES, fieldErrors, signature, close, formatExample,
      addParam, removeParam, moveParam, paramErrors, save, remove, runNow, editFromRun,
    };
  },
  template: `
  <dialog ref="dlg" class="saved-query-dialog" aria-labelledby="sq-title" @close="d.mode = null">
    <form v-if="d.mode === 'edit' && d.draft" method="dialog" @submit.prevent="save">
      <div class="dialog-head">
        <h2 id="sq-title">{{ d.draft.replaces ? 'Edit saved query' : 'New saved query' }}</h2><span class="spacer"></span>
        <button type="button" class="icon-btn" @click="close" aria-label="Close">×</button>
      </div>
      <div class="dialog-body">
        <div class="form-grid">
          <label for="sq-name">Name</label>
          <input id="sq-name" type="text" v-model="d.draft.name" :class="{ invalid: fieldErrors.name }" placeholder="top_customers" autocomplete="off" spellcheck="false">
          <div v-for="m in fieldErrors.name || []" class="field-error sq-span">{{ m }}</div>
          <label for="sq-folder">Folder</label>
          <input id="sq-folder" type="text" v-model="d.draft.folder" :class="{ invalid: fieldErrors.folder }" placeholder="reports/sales (empty: the root)" autocomplete="off" spellcheck="false">
          <div v-for="m in fieldErrors.folder || []" class="field-error sq-span">{{ m }}</div>
          <label for="sq-description">Description</label>
          <input id="sq-description" type="text" v-model="d.draft.description" :class="{ invalid: fieldErrors.description }" placeholder="What it answers, in one sentence">
          <div v-for="m in fieldErrors.description || []" class="field-error sq-span">{{ m }}</div>
        </div>

        <h3>Parameters</h3>
        <p v-if="!d.draft.params.length" class="muted sq-note">No parameters. Callers pass named values, for example g.query("name", #{limit: 5}).</p>
        <div v-for="(p, i) in d.draft.params" :key="i" class="sq-param" :class="{ invalid: paramErrors(p, i).length }">
          <div class="sq-param-row">
            <input type="text" v-model="p.name" class="sq-param-name" :aria-label="'Name of parameter ' + (i + 1)" placeholder="name" spellcheck="false">
            <select v-model="p.type" :aria-label="'Type of ' + p.name">
              <option v-for="t in PARAM_TYPES" :key="t" :value="t">{{ t === 'custom' ? 'schema (JSON)' : t }}</option>
            </select>
            <select v-if="p.type === 'array'" v-model="p.itemsType" :aria-label="'Item type of ' + p.name">
              <option v-for="t in ITEM_TYPES" :key="t" :value="t">of {{ t }}</option>
            </select>
            <input type="text" v-model="p.defaultText" class="sq-default" :aria-label="'Default of ' + p.name" :placeholder="p.type === 'boolean' ? 'default: true / false' : 'default (empty: required)'">
            <span class="spacer"></span>
            <button type="button" class="icon-btn" :disabled="i === 0" @click="moveParam(i, -1)" :aria-label="'Move ' + p.name + ' up'" title="Move up">↑</button>
            <button type="button" class="icon-btn" :disabled="i === d.draft.params.length - 1" @click="moveParam(i, 1)" :aria-label="'Move ' + p.name + ' down'" title="Move down">↓</button>
            <button type="button" class="icon-btn" @click="removeParam(i)" :aria-label="'Remove ' + p.name" title="Remove">✕</button>
          </div>
          <div class="sq-param-row">
            <template v-if="p.type === 'integer' || p.type === 'number'">
              <input type="text" v-model="p.minimum" placeholder="minimum" :aria-label="'Minimum of ' + p.name" class="sq-small">
              <input type="text" v-model="p.maximum" placeholder="maximum" :aria-label="'Maximum of ' + p.name" class="sq-small">
            </template>
            <template v-if="p.type === 'string'">
              <input type="text" v-model="p.minLength" placeholder="min length" :aria-label="'Minimum length of ' + p.name" class="sq-small">
              <input type="text" v-model="p.maxLength" placeholder="max length" :aria-label="'Maximum length of ' + p.name" class="sq-small">
              <input type="text" v-model="p.pattern" placeholder="pattern (regex)" :aria-label="'Pattern of ' + p.name" spellcheck="false">
            </template>
            <input v-if="['string', 'integer', 'number'].includes(p.type)" type="text" v-model="p.enumText" placeholder="allowed values: a, b, c" :aria-label="'Allowed values of ' + p.name">
            <input v-if="p.type !== 'custom'" type="text" v-model="p.description" placeholder="description (a hint in the run form)" :aria-label="'Description of ' + p.name" class="sq-grow">
          </div>
          <textarea v-if="p.type === 'custom'" v-model="p.schemaText" class="code-input sq-schema" rows="3" spellcheck="false"
                    :aria-label="'Schema of ' + p.name" placeholder='{"type": ["integer", "null"], "minimum": 0}'></textarea>
          <div v-for="m in paramErrors(p, i)" class="field-error">{{ m }}</div>
        </div>
        <div v-for="m in fieldErrors.params || []" class="field-error">{{ m }}</div>
        <button type="button" class="btn small" @click="addParam" style="margin-top:6px">+ Add parameter</button>

        <h3>Body</h3>
        <p v-if="d.draft.raw" class="muted sq-note">The whole function text (its stored layout cannot be split): keep <code>fn {{ d.draft.name || 'name' }}(…)</code> and its parameters in step with the list above.</p>
        <div v-else class="sq-signature mono">{{ signature }}</div>
        <div ref="host" class="sq-editor" :class="{ invalid: fieldErrors.body }"></div>
        <div v-if="!d.draft.raw" class="sq-signature mono">}</div>
        <div v-for="m in fieldErrors.body || []" class="field-error">{{ m }}</div>
        <p class="muted sq-note">A saved query is read-only: it may read the graph, never change it. Return a traversal (callers can continue it) or a value.</p>
        <div v-for="m in [...(fieldErrors.general || []), ...(fieldErrors.meta || [])]" class="field-error">{{ m }}</div>

        <div class="sq-buttons">
          <button v-if="d.draft.replaces" type="button" class="btn danger" :disabled="d.busy" @click="remove">Delete…</button>
          <span class="spacer"></span>
          <button type="button" class="btn" @click="close">Cancel</button>
          <button type="submit" class="btn primary" :disabled="d.busy">{{ d.busy ? 'Saving…' : 'Save' }}</button>
        </div>
      </div>
    </form>

    <form v-else-if="d.mode === 'run' && d.entry" method="dialog" @submit.prevent="runNow">
      <div class="dialog-head">
        <h2 id="sq-title">Run "{{ d.entry.name }}"</h2><span class="spacer"></span>
        <button type="button" class="icon-btn" @click="close" aria-label="Close">×</button>
      </div>
      <div class="dialog-body">
        <p v-if="d.entry.description" class="sq-description">{{ d.entry.description }}</p>
        <p v-if="d.entry.status === 'error'" class="field-error">It currently fails: {{ d.entry.error }}</p>
        <p v-if="!d.fields.length" class="muted sq-note">No parameters.</p>
        <div v-else class="form-grid">
          <template v-for="f in d.fields" :key="f.name">
            <label :for="'sq-run-' + f.name">{{ f.name }}<span v-if="f.required" class="sq-required" title="required">*</span></label>
            <select v-if="f.kind === 'select'" :id="'sq-run-' + f.name" v-model="d.values[f.name]" :class="{ invalid: d.errors[f.name] }">
              <option v-if="!f.required" value="">{{ f.hasDefault ? '(default)' : '(none)' }}</option>
              <option v-for="o in f.options" :key="JSON.stringify(o)" :value="JSON.stringify(o)">{{ typeof o === 'string' ? o : JSON.stringify(o) }}</option>
            </select>
            <input v-else-if="f.kind === 'checkbox'" :id="'sq-run-' + f.name" type="checkbox" v-model="d.values[f.name]" style="justify-self:start">
            <input v-else-if="f.kind === 'integer' || f.kind === 'number'" :id="'sq-run-' + f.name" type="number"
                   :step="f.kind === 'integer' ? 1 : 'any'" :min="f.min" :max="f.max" v-model="d.values[f.name]"
                   :class="{ invalid: d.errors[f.name] }" :placeholder="f.hasDefault ? 'default ' + f.defaultValue : ''" style="width:160px">
            <textarea v-else-if="f.kind === 'list' || f.kind === 'json'" :id="'sq-run-' + f.name" v-model="d.values[f.name]" rows="3"
                      class="code-input" :class="{ invalid: d.errors[f.name] }" spellcheck="false"
                      :placeholder="f.kind === 'list' ? 'one ' + f.itemsKind + ' per line' : 'JSON'"></textarea>
            <input v-else :id="'sq-run-' + f.name" type="text" v-model="d.values[f.name]" :class="{ invalid: d.errors[f.name] }"
                   :placeholder="f.hasDefault ? 'default: ' + f.defaultValue : formatExample(f.kind)" spellcheck="false">
            <span v-if="d.errors[f.name]" class="hint sq-error">{{ d.errors[f.name] }}</span>
            <span v-if="f.hint" class="hint">{{ f.hint }}</span>
          </template>
        </div>
        <p class="muted sq-note">Run writes <code>g.query("{{ d.entry.name }}", #{…})</code> into a query tab and runs it there: visible, editable and in the history. An empty field takes the parameter's default.</p>
        <div class="sq-buttons">
          <button type="button" class="btn" @click="editFromRun">Edit…</button>
          <span class="spacer"></span>
          <button type="button" class="btn" @click="close">Cancel</button>
          <button type="submit" class="btn primary" :disabled="d.busy || state.running">Run</button>
        </div>
      </div>
    </form>
  </dialog>
  `,
};
