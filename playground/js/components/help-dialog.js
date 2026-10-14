// Help dialog: keyboard shortcuts, DSL tips, and how the engine runs (Stop, Save graph).

import { ref } from 'vue';
import { state } from '../store.js';
import { modKey } from '../util.js';

export const HelpDialog = {
  setup(props, { expose }) {
    const dlg = ref(null);
    const open = () => { if (dlg.value && !dlg.value.open) dlg.value.showModal(); };
    const close = () => { if (dlg.value) dlg.value.close(); };
    expose({ open, close });
    const shortcuts = [
      [`${modKey}+Enter`, 'Run the query of the active tab, or only its selection when text is selected'],
      [`${modKey}+Shift+Enter`, 'Run all tabs, left to right'],
      ['Alt+N', 'New query tab'],
      ['Alt+W', 'Close the query tab (asks when its text was never run)'],
      ['Alt+[ / Alt+]', 'Previous / next query tab'],
      ['Alt+Shift+1 … Alt+Shift+9', 'Query tab 1 … 8; 9 is the last tab'],
      ['Double-click a tab (or F2 on it)', 'Rename the tab; drag a tab to reorder, middle-click to close'],
      ['Ctrl+Space', 'Show completions (also opens after typing ".")'],
      ['Alt+↑ / Alt+↓', 'Previous / next query from the history'],
      [`${modKey}+F`, 'Search in the editor'],
      [`${modKey}+/`, 'Toggle line comment'],
      [`${modKey}+Z / ${modKey}+Shift+Z`, 'Undo / redo'],
      ['Alt+1 … Alt+5', 'Show and focus Query, Results, Graph, Schema, Profile'],
      ['Alt+M', 'Maximise the focused (last clicked) panel / restore the layout'],
      ['Esc', 'Restore the layout from a maximised panel'],
      ['Double-click empty drawing space', 'Fit the graph, schema graph or Mermaid diagram to the panel'],
      ['Double-click a vertex', 'Select it and its neighbours (again: one hop more); in the Fast tier, "Open in Detailed" draws them'],
      ['Pinch / Ctrl+wheel, drag', 'Zoom and pan a drawing'],
      ['Alt+T', 'Cycle the theme: system, light, dark'],
      ['Alt+,', 'Settings (display limits)'],
      ['F1 or ?', 'This help'],
      ['Esc (in a dialog, menu or completion list)', 'Close it'],
    ];
    return { dlg, close, shortcuts, state, modKey };
  },
  template: `
  <dialog ref="dlg" aria-labelledby="help-title" @click.self="close">
    <div class="dialog-head">
      <h2 id="help-title">Help</h2><span class="spacer"></span>
      <button class="icon-btn" @click="close" aria-label="Close">×</button>
    </div>
    <div class="dialog-body">
      <h3>Keyboard shortcuts</h3>
      <table class="shortcuts">
        <tr v-for="s in shortcuts" :key="s[0]"><td><kbd>{{ s[0] }}</kbd></td><td>{{ s[1] }}</td></tr>
      </table>

      <p class="muted">The browser keeps {{ modKey }}+T, {{ modKey }}+W and {{ modKey }}+1…9 for its own tabs, so the
      query tabs use Alt instead.</p>

      <h3>Query tabs</h3>
      <p>Each tab has its own query, Profile/Memory toggles, last result and last profile; switching tabs shows that
      tab's results without running anything. The graph is shared: when a run in one tab changes it, the other tabs'
      results get a <span style="color:var(--warn)">●</span> "possibly stale" mark until they run again. An example
      goes into the active tab only when it is empty or still holds an unchanged example; otherwise it opens in a new
      tab, so your own text is never overwritten. The tabs (not the results) are kept in this browser; the history
      records every run with its tab's name.</p>

      <h3>Writing queries</h3>
      <p>Traversals start from <code>g</code> (<code>g.v()</code>, <code>g.e()</code>); child traversals from
      <code>__</code> (<code>__.out("knows")</code>). Every step has a snake_case and a Gremlin camelCase spelling
      (<code>has_label</code> / <code>hasLabel</code>). Predicates: <code>P.gt(30)</code>, <code>P.within("a", "b")</code>,
      <code>TextP.containing("ar")</code>. Tokens: <code>T.label</code>, <code>Order.desc</code>, <code>Scope.local</code>.</p>
      <p>A traversal without a terminal is displayed; <code>to_list()</code> and <code>next()</code> return data.
      A traversal left without a terminal runs as <code>execute()</code>: results and, with <b>Profile</b> on in the
      Query toolbar, the metrics of the same run (the Profile panel follows every run that returns metrics).
      <b>Memory</b> adds the memory section (exact allocator figures in this build). Explicit terminals are honoured:
      <code>execute(#{profile: true, profile_types: ["memory"]})</code> returns the execution object
      (results, error, profile); TinkerPop's <code>.profile()</code> returns only the metrics.
      A runtime error names the step it happened in (a breadcrumb such as <code>union(...) › property(...)</code>) and
      keeps the partial profile up to it, with the failing row marked "failed here".</p>

      <h3>Display limits</h3>
      <p>Display is bounded, data never: results show at most {{ state.settings.maxRows }} rows and
      {{ state.settings.maxItems }} nested items (Settings). One query can override it with
      <code>g.with("render.max_rows", 10)</code> or <code>to_table(#{max_rows: 20})</code>.</p>

      <h3>The engine</h3>
      <p v-if="state.engineInfo && state.engineInfo.server"><b>Dev server mode (DEV, unstable, local only).</b> This page is served by <code>graphersal --server</code>{{ state.engineInfo.version ? " (version " + state.engineInfo.version + ")" : "" }}: queries run in that process on the graph it was started with, which several browser tabs share. <b>Stop</b> cancels the running query on the server; it is rolled back and the graph stays as it was before it. There is no graph choice in this mode (restart the server with another <code>--graph</code>). <b>Save ▾</b> downloads the graph data, the schema or a snapshot; <b>Save to file</b> (when the server was started with a graph file) writes it back to that file. Nothing is saved automatically, except on a <b>Store directory</b> (<code>graphersal --graph data/ --server</code>): there every commit is on disk before its query answers and survives a restart, and the <b>Store</b> menu shows the position, the snapshots (each a <code>.gsnap</code> download) and the marks, and offers <b>Checkpoint</b> and <b>Mark</b>.</p>
      <p v-if="state.engineInfo && (state.engineInfo.store || state.engineInfo.storeMemory)"><b>Snapshots and marks.</b> A <b>snapshot</b> is a full copy of the graph at a commit (a fast start); a <b>mark</b> is a name for a position in the history. Any commit can be a target, not only those: on every snapshot and mark, and through <b>Go to commit</b> / <b>Go to time</b> (your local time), the Store menu offers
      <b>Open read-only here</b> (queries read that past state, writes are refused; a banner offers <b>Back to the live graph</b>),
      <template v-if="state.engineInfo.storeMemory"><b>Fork into a new database</b> (a second store in memory, listed under <b>Databases</b> with <b>Switch to</b> and <b>Close</b>; this one is not changed) and</template>
      <template v-else><b>Fork to a new directory</b> (a new store next to this one by default; this store is not changed; open it with <code>graphersal --graph &lt;new&gt; --server</code>) and</template>
      <b>Roll back here</b> (asks first: everything after that point moves to the <b>attic</b> and the live graph switches to it; the running query is cancelled).
      The attic lists each rollback's history: <b>Show changes</b> (100 commits per page, the totals over all), <b>Restore</b> (undo the rollback; possible only while nothing new was committed or marked), <b>Fork</b>, <b>Remove</b>.
      <b>Verify the store</b> checks every checksum. <b>Disk space</b> says whether <b>Compact</b> is worth it now (cheaply, from the store's own bookkeeping): it prunes old snapshots and WAL (the last two snapshots always stay) and, for a single-file store (<code>graphersal --graph data.gstore --server</code>), rewrites the file without the space removals left in it. Compact asks first when it is not recommended, and always when marks would become unreachable (it names them: afterwards they cannot be opened, only from a backup holding older snapshots). The <b>↻</b> button in the menu's top right corner reloads the store information.</p>
      <p v-if="state.engineInfo && state.engineInfo.storeMemory"><b>The store in memory.</b> Every graph you load (a sample, an empty graph, a file) is kept as a <b>store in memory</b>: the loaded state is commit 0 (a snapshot), every query that changes the graph adds a commit to its write-ahead log, and the <b>Store</b> menu works on it exactly as the dev server's does on a directory: checkpoints, marks, read-only views of any past commit or time, rollback and the attic, forks, verify, <code>.gsnap</code> downloads; a snapshot is written automatically after 4 MiB of log. <b>In memory: everything is lost on page reload</b> (and on <b>Stop</b>, which starts the store over from the last loaded or saved graph). Download a <code>.gsnap</code> to keep a state. The menu shows the store's size next to the graph's.</p>
      <p v-if="!(state.engineInfo && state.engineInfo.server)">Queries run on the Graphersal library compiled to WebAssembly<template v-if="state.engineInfo"> (version {{ state.engineInfo.version }})</template>,
      in a background worker; every run gets a fresh script scope. <b>Stop</b> (next to Run) ends a long
      query by restarting the engine: the graph comes back as last loaded (sample or file) or last saved
      with <b>Save</b>; changes made after that are lost. Memory profiles report exact allocator figures
      ("tracked"). The <b>Save ▾</b> menu downloads one file per entry: the graph data as GraphSON 3.0
      (TinkerPop's format) or GraphML, the schema as JSON (graph files carry no schema), or one snapshot
      (<code>.gsnap</code>) with both. To load data and schema back, choose both in <b>Load from file</b> on the
      start page (the graph file and the optional schema file): the schema is set first and the data imported
      against it (a violation fails the load and nothing changes). A graph file loaded without a schema file
      gets no schema: the previous graph's schema is never kept. <b>Load schema</b> applies a schema to the
      current graph. GraphSON keeps every value type (uuid, dates and times, array, object); GraphML gets them back from
      the schema. A GraphSON import skips what the
      engine cannot hold (a type without a Graphersal twin) and says so in a notice. <b>Save as snapshot</b> (start
      page) downloads one packed snapshot (<code>.gsnap</code>, Graphersal's own binary format): lossless,
      with the schema, loaded back with <b>Load from file</b> in one step.</p>

      <h3>Schema editor</h3>
      <p>The Schema panel's <b>Tables</b> and <b>JSON</b> tabs edit a <b>draft</b> of the schema, never the stored one:
      types, required/nullable, constraints and annotations per property, labels and edge connections, the mode.
      <b>What changes</b> lists the changes, each backward compatible (it only adds or relaxes, so clients written against the old schema keep working) or not; <b>Check against data</b> lists the violations (click
      one to see the offending elements in a new tab), <b>Apply</b> stores the draft as one change and is blocked while
      the data violates it. A schema <code>default</code> (or <code>defaultFunction</code>: <code>uuidv4</code>, <code>uuidv7</code>, <code>now</code>, <code>today</code>) of a top-level property is stored when a vertex or edge is created without it; existing data is never filled in.</p>

      <h3>If the playground freezes</h3>
      <p>Open <a href="reset/">reset/</a> (this address with <code>/reset</code> added): it clears the playground's
      saved settings, layout and tabs in this browser (the query history stays) and starts again with the Modern
      sample graph, without loading the playground first. Large drawings use the Fast (WebGL) tier; a drawing that could
      still freeze the page (Detailed forced above 5000 elements, Fast above 200 000) asks first.</p>
    </div>
  </dialog>
  `,
};
