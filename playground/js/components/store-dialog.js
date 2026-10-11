// The dialog of the Store menu's actions (dev server on a Store directory, or the browser's store
// in memory): the three actions on a target typed in "Go to" (commit or time), "Fork..." (on the
// server it asks for the directory, which the server validates and reports how to open; in memory
// it makes a new database, offered to switch to), "Roll back here..." (explains the effect, option
// "delete instead of keeping in the attic"), and the attic: "Show changes" (page by page, 100
// commits at a time, the totals over all of them), "Fork", "Remove". The header has a full-screen
// toggle for the wide changes table (Esc restores the size before it closes the dialog); the body
// scrolls in both directions. Opened by the Store menu through `open(mode, payload)`; the texts
// come from js/store-model.js.

import { ref, reactive, computed } from 'vue';
import { state, toast, openStoreView, rollbackStore, switchDatabase, runStoreTask } from '../store.js';
import {
  actionsFor, targetText, targetLabel, defaultForkDir, rollbackPlan, changesModel, CHANGES_PAGE,
  taskButton,
} from '../store-model.js';
import { StoreTaskLabel } from './store-task-label.js';

export const StoreDialog = {
  components: { StoreTaskLabel },
  props: {
    /** The `GET /api/store` answer the menu shows (position, marks, attic, dir, memory). */
    store: { type: Object, default: null },
  },
  emits: ['changed'],
  setup(props, { expose, emit }) {
    const dlg = ref(null);
    const full = ref(false);
    const d = reactive({
      mode: null, // 'target' | 'fork' | 'rollback' | 'changes' | 'atticFork' | 'atticRemove'
      target: null,
      entry: null, // an attic row
      dir: '',
      deleteHistory: false,
      busy: false,
      error: '',
      result: null, // what the engine reported, shown in place of the form
      changes: null,
    });
    const memory = computed(() => !!(props.store && props.store.memory));

    function open(mode, payload = {}) {
      Object.assign(d, {
        mode, target: payload.target || null, entry: payload.entry || null, dir: '',
        deleteHistory: false, busy: false, error: '', result: null, changes: null,
      });
      full.value = false;
      if (dlg.value && !dlg.value.open) dlg.value.showModal();
      if (mode === 'changes') loadChanges(0);
    }
    function close() {
      full.value = false;
      if (dlg.value) dlg.value.close();
    }
    /** Esc: a full-screen dialog first goes back to its size; the next Esc closes it. */
    function onCancel(event) {
      if (full.value) {
        event.preventDefault();
        event.stopPropagation();
        full.value = false;
      }
    }

    const message = (err) => (err && err.message ? err.message : String(err));

    /**
     * Runs a store action of the dialog as THE running store action (`runStoreTask`: its button,
     * the Store button and the live region show it); refused while another one runs.
     */
    async function task(id, fn) {
      const res = await runStoreTask(id, fn);
      if (res.refused) throw new Error('wait for the running store action (or query) to finish');
      return res.value;
    }
    async function busy(fn) {
      if (d.busy) return;
      d.busy = true;
      d.error = '';
      try {
        await fn();
      } catch (err) {
        d.error = message(err);
      } finally {
        d.busy = false;
      }
    }

    const title = computed(() => {
      const where = memory.value ? 'a new database' : 'a new directory';
      switch (d.mode) {
        case 'target': return `Go to ${targetText(d.target)}`;
        case 'fork': return `Fork ${targetText(d.target)} into ${where}`;
        case 'rollback': return plan.value ? plan.value.title : 'Roll back';
        case 'changes': return `Attic entry: ${d.entry ? d.entry.range : ''}`;
        case 'atticFork': return `Fork the attic entry (${d.entry ? d.entry.range : ''})`;
        case 'atticRemove': return 'Remove the attic entry?';
        default: return '';
      }
    });
    const plan = computed(() => (d.mode === 'rollback' && d.target ? rollbackPlan(props.store, d.target, d.deleteHistory) : null));
    const forkPlaceholder = computed(() => {
      const dir = props.store ? props.store.dir : 'store';
      const label = d.mode === 'atticFork' && d.entry ? `attic-${d.entry.id}` : (d.target ? targetLabel(d.target) : '');
      return defaultForkDir(dir, label);
    });
    const changes = computed(() => (d.changes ? changesModel(d.changes) : null));

    function chooseAction(id) {
      if (id === 'view') view();
      else d.mode = id;
    }

    function view() {
      busy(async () => {
        const res = await task('view', () => openStoreView(d.target));
        if (!res) return;
        toast(`${res.view.label}. Queries read this state; writes are refused. "Back to the live graph" in the banner.`, 'info', 8000);
        close();
      });
    }

    function fork() {
      busy(async () => {
        const res = await task(d.mode, () => (d.mode === 'atticFork'
          ? state.engine.atticFork(d.entry.id, d.dir.trim())
          : state.engine.fork(d.target, d.dir.trim())));
        d.result = { kind: res.fork.database ? 'database' : 'fork', ...res.fork };
        toast(res.fork.database
          ? `Forked at commit ${res.fork.commitSeq} into the database "${res.fork.label}".`
          : `Forked at commit ${res.fork.commitSeq} into ${res.fork.dir}.`, 'info', 7000);
        state.storeVersion++;
        emit('changed');
      });
    }

    function switchToFork() {
      busy(async () => {
        await task('switch', () => switchDatabase(d.result.database));
        toast(`Switched to "${d.result.label}".`, 'info', 6000);
        close();
      });
    }

    function rollback() {
      busy(async () => {
        const res = await task('rollback', () => rollbackStore(d.target, d.deleteHistory));
        if (!res) return;
        d.result = { kind: 'rollback', ...res.rollback };
        toast(`Rolled back to commit ${res.rollback.target}.`, 'info', 7000);
        emit('changed');
      });
    }

    function loadChanges(offset) {
      busy(async () => {
        d.changes = await state.engine.atticChanges(d.entry.id, offset, CHANGES_PAGE);
      });
    }

    function removeEntry() {
      busy(async () => {
        await task('atticRemove', () => state.engine.atticRemove(d.entry.id));
        toast(`Removed the attic entry ${d.entry.id}.`, 'info', 6000);
        emit('changed');
        close();
      });
    }

    expose({ open, close });
    return {
      dlg, d, full, memory, title, plan, forkPlaceholder, changes, state,
      actions: computed(() => actionsFor(memory.value)),
      taskButton: (id, title) => taskButton(state.storeTask, id, title),
      close, onCancel, chooseAction, fork, switchToFork, rollback, removeEntry, loadChanges, targetText,
    };
  },
  template: `
  <dialog ref="dlg" class="store-dialog" :class="{ wide: d.mode === 'changes', full }" aria-labelledby="store-dialog-title"
          @click.self="close" @cancel="onCancel" @keydown.esc="onCancel">
    <div class="dialog-head">
      <h2 id="store-dialog-title">{{ title }}</h2><span class="spacer"></span>
      <button type="button" class="icon-btn" @click="full = !full" :title="full ? 'Restore size (Esc)' : 'Full screen'"
              :aria-label="full ? 'Restore the dialog size' : 'Show the dialog full screen'" :aria-pressed="String(full)">
        <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" aria-hidden="true">
          <template v-if="full"><path d="M2.5 9.5h4v4M6.5 9.5L2 14M13.5 6.5h-4v-4M9.5 6.5L14 2"/></template>
          <template v-else><path d="M9.5 2.5h4v4M13.5 2.5L9 7M6.5 13.5h-4v-4M2.5 13.5L7 9"/></template>
        </svg>
      </button>
      <button type="button" class="icon-btn" @click="close" aria-label="Close">×</button>
    </div>
    <div class="dialog-body">
      <template v-if="d.mode === 'target'">
        <p>What should happen with the state at {{ targetText(d.target) }}?</p>
        <div v-for="a in actions" :key="a.id" class="store-choice">
          <button type="button" class="btn" :disabled="d.busy || state.storeBusy || !!state.storeTask" v-bind="a.id === 'view' ? taskButton('view', null) : {}" @click="chooseAction(a.id)"><store-task-label v-if="a.id === 'view'" task="view" :label="a.label"></store-task-label><template v-else>{{ a.label }}</template></button>
          <span class="muted">{{ a.title }}</span>
        </div>
      </template>

      <template v-else-if="(d.mode === 'fork' || d.mode === 'atticFork') && !d.result">
        <template v-if="memory">
          <p v-if="d.mode === 'fork'">A fork is a NEW database in memory that starts from the state at {{ targetText(d.target) }}.
            This one is not changed; the fork is listed under Databases (switch to it there, or close it to undo).</p>
          <p v-else>The rolled-back history of this entry (up to its last commit) becomes a NEW database in memory.
            The attic entry stays.</p>
          <p class="muted">Like everything in the browser it is lost on page reload.</p>
        </template>
        <template v-else>
          <p v-if="d.mode === 'fork'">A fork is a NEW store directory that starts from the state at {{ targetText(d.target) }}.
            This store is not changed; to undo the fork, delete the new directory.</p>
          <p v-else>The rolled-back history of this entry (up to its last commit) becomes a NEW store directory.
            The attic entry stays.</p>
          <form class="form-grid" @submit.prevent="fork">
            <label for="store-fork-dir">Directory</label>
            <input id="store-fork-dir" class="input" v-model="d.dir" :placeholder="forkPlaceholder" autocomplete="off">
            <span class="hint">Must not exist yet. A relative path is created next to the store; empty: {{ forkPlaceholder }}.</span>
          </form>
        </template>
        <div class="store-dialog-buttons">
          <span class="spacer"></span>
          <button type="button" class="btn" @click="close">Cancel</button>
          <button type="button" class="btn primary" :disabled="d.busy || !!state.storeTask" v-bind="taskButton(d.mode, null)" @click="fork"><store-task-label :task="d.mode" label="Fork"></store-task-label></button>
        </div>
      </template>

      <template v-else-if="d.mode === 'rollback' && !d.result">
        <p v-for="(line, i) in plan.lines" :key="i" :class="{ 'store-warn': i === 0 && d.deleteHistory }">{{ line }}</p>
        <p v-if="plan.nothingToDo" class="store-warn">The store is at commit {{ store && store.commitSeq }}: there is nothing after commit {{ plan.commit }}.</p>
        <label class="store-check"><input type="checkbox" v-model="d.deleteHistory"> Delete instead of keeping in the attic (cannot be undone)</label>
        <div class="store-dialog-buttons">
          <span class="spacer"></span>
          <button type="button" class="btn" @click="close">Cancel</button>
          <button type="button" class="btn danger" :disabled="d.busy || !!state.storeTask || plan.nothingToDo" v-bind="taskButton('rollback', null)" @click="rollback">
            <store-task-label task="rollback" :label="d.deleteHistory ? 'Roll back and delete' : 'Roll back'"></store-task-label></button>
        </div>
      </template>

      <template v-else-if="d.result && d.result.kind === 'fork'">
        <p><b>Forked</b> at commit {{ d.result.commitSeq }} into <code>{{ d.result.dir }}</code>.</p>
        <p>Open it in another dev server (this one keeps serving this store):</p>
        <pre class="store-code">{{ d.result.open }}</pre>
        <p class="muted">To undo: {{ d.result.undo }}.</p>
        <p v-if="d.result.marksNotCarried && d.result.marksNotCarried.length" class="muted">Marks not carried (a fork starts without marks): {{ d.result.marksNotCarried.map(m => m.name + ' @ ' + m.commitSeq).join(', ') }}.</p>
        <div class="store-dialog-buttons"><span class="spacer"></span><button type="button" class="btn primary" @click="close">Close</button></div>
      </template>

      <template v-else-if="d.result && d.result.kind === 'database'">
        <p><b>Forked</b> at commit {{ d.result.commitSeq }} into the database "{{ d.result.label }}" (in memory).</p>
        <p class="muted">It is listed under Databases in the Store menu. To undo: {{ d.result.undo }}.</p>
        <p v-if="d.result.marksNotCarried && d.result.marksNotCarried.length" class="muted">Marks not carried (a fork starts without marks): {{ d.result.marksNotCarried.map(m => m.name + ' @ ' + m.commitSeq).join(', ') }}.</p>
        <div class="store-dialog-buttons">
          <span class="spacer"></span>
          <button type="button" class="btn" @click="close">Stay here</button>
          <button type="button" class="btn primary" :disabled="d.busy || !!state.storeTask" v-bind="taskButton('switch', null)" @click="switchToFork"><store-task-label task="switch" label="Switch to it"></store-task-label></button>
        </div>
      </template>

      <template v-else-if="d.result && d.result.kind === 'rollback'">
        <p><b>Rolled back</b> from commit {{ d.result.previousCommitSeq }} to commit {{ d.result.target }}. The live graph shows that state now.</p>
        <p v-if="d.result.attic">The history after it is in the attic ({{ d.result.attic }}). Undo: {{ d.result.undo }}.</p>
        <p v-else class="store-warn">The history after it was deleted.</p>
        <div class="store-dialog-buttons"><span class="spacer"></span><button type="button" class="btn primary" @click="close">Close</button></div>
      </template>

      <template v-else-if="d.mode === 'changes'">
        <p v-if="d.busy && !changes" class="muted">Reading the attic entry…</p>
        <template v-if="changes">
          <p><b>{{ changes.headline }}</b></p>
          <p class="muted">Rolled back {{ d.entry.when }} ({{ d.entry.reason }}), {{ d.entry.size }}.</p>
          <div class="store-pager">
            <button type="button" class="btn small" :disabled="d.busy || changes.prevOffset === null" @click="loadChanges(changes.prevOffset)">‹ Previous</button>
            <span class="store-page" aria-live="polite">{{ changes.page }}</span>
            <button type="button" class="btn small" :disabled="d.busy || changes.nextOffset === null" @click="loadChanges(changes.nextOffset)">Next ›</button>
          </div>
          <div class="store-changes-wrap">
            <table class="data-table store-changes">
              <thead><tr><th>Commit</th><th>Time</th><th>What</th><th>Examples</th></tr></thead>
              <tbody>
                <tr v-for="r in changes.rows" :key="r.commit">
                  <td>{{ r.commit }}</td><td>{{ r.when }}<div v-if="r.by" class="muted">by {{ r.by }}</div></td>
                  <td>{{ r.what }}</td>
                  <td><div v-for="(s, i) in r.samples" :key="i" class="mono">{{ s }}</div></td>
                </tr>
              </tbody>
            </table>
          </div>
        </template>
      </template>

      <template v-else-if="d.mode === 'atticRemove'">
        <p>The rolled-back history {{ d.entry.range }} ({{ d.entry.size }}) is deleted for good: it can be neither restored nor forked afterwards.</p>
        <div class="store-dialog-buttons">
          <span class="spacer"></span>
          <button type="button" class="btn" @click="close">Cancel</button>
          <button type="button" class="btn danger" :disabled="d.busy || !!state.storeTask" v-bind="taskButton('atticRemove', null)" @click="removeEntry"><store-task-label task="atticRemove" label="Remove"></store-task-label></button>
        </div>
      </template>

      <p v-if="d.error" class="badge danger store-error" role="alert">{{ d.error }}</p>
    </div>
  </dialog>
  `,
};
