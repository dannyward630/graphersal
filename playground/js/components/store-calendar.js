// The history calendar of the Store menu ("Calendar…" under Marks and Snapshots): a modal month
// calendar of the store's marks and snapshots. Each day shows how many of each fall on it (your
// local time) as two badges whose colour deepens with the count; the month and year selectors
// (with per-month counts), previous/next, Today, Newest and Oldest move through a long history.
// A click (or Enter) on a day lists ALL its marks and snapshots beside the calendar with the
// time, the commit and the menu's own actions (View, Fork…, Roll back…, .gsnap): they are
// emitted to the Store menu, which runs them exactly as from its lists. A filter keeps marks
// and/or snapshots and matches a name or `#commit`. Keyboard: arrows move between days (across
// months), PageUp/PageDown change the month, Home/End the week, Enter opens a day, Esc closes.
// The grouping is computed once per store answer and filter (js/store-calendar.js), never per
// cell.

import { ref, computed, nextTick, watch } from 'vue';
import {
  historyEntries, filterEntries, historyIndex, monthGrid, weekdayNames, bucket, addDays,
  addMonths, sameDayIn, parseDayKey, todayKey, yearRange, monthTotals, MONTHS, dayLabel,
  countText,
} from '../store-calendar.js';
import { actionDisabled, commitTarget, markTarget } from '../store-model.js';
import { formatCount } from '../statistics.js';

/** How many rows of one kind the day panel renders before "Show more". */
const PAGE = 200;

export const StoreCalendar = {
  props: {
    /** The `GET /api/store` answer the menu shows. */
    store: { type: Object, default: null },
    /** The actions on a point (`actionsFor`), as in the menu. */
    actions: { type: Array, default: () => [] },
    /** A store action or a query runs: every action is disabled. */
    locked: { type: Boolean, default: false },
    /** The download URL of a snapshot (dev server), or null (then `download` is emitted). */
    snapshotUrl: { type: Function, default: () => null },
  },
  emits: ['action', 'download'],
  setup(props, { expose, emit }) {
    const dlg = ref(null);
    const grid = ref(null);
    const isOpen = ref(false);
    const view = ref({ year: 1970, month: 0 });
    const cursor = ref(''); // the focused day (roving tabindex)
    const selected = ref(''); // the day listed in the panel ('undated': entries without a time)
    const text = ref('');
    const kinds = ref({ mark: true, snapshot: true });
    const shown = ref({ mark: PAGE, snapshot: PAGE });
    const today = ref(todayKey());

    // Computed once per store answer, then once per filter change; cells only look up.
    const entries = computed(() => (isOpen.value ? historyEntries(props.store) : []));
    const filtered = computed(() => filterEntries(entries.value, { text: text.value, kinds: kinds.value }));
    const index = computed(() => historyIndex(filtered.value));
    const totals = computed(() => monthTotals(index.value));
    const years = computed(() => yearRange(index.value, today.value, view.value.year));
    const weekdays = weekdayNames(1);

    const cells = computed(() => {
      const { days, max } = index.value;
      return monthGrid(view.value.year, view.value.month, 1).map((c) => {
        const day = days.get(c.key) || null;
        const marks = day ? day.marks.length : 0;
        const snapshots = day ? day.snapshots.length : 0;
        return {
          ...c,
          marks,
          snapshots,
          mBucket: bucket(marks, max.mark),
          sBucket: bucket(snapshots, max.snapshot),
          tBucket: bucket(marks + snapshots, max.total),
          today: c.key === today.value,
          label: dayLabel(c.key, day),
        };
      });
    });
    const rows = computed(() => {
      const out = [];
      for (let i = 0; i < cells.value.length; i += 7) out.push(cells.value.slice(i, i + 7));
      return out;
    });

    const monthOptions = computed(() => MONTHS.map((name, month) => {
      const t = totals.value.get(`${String(view.value.year).padStart(4, '0')}-${String(month + 1).padStart(2, '0')}`);
      return { month, text: t ? `${name} (${t.marks} M · ${t.snapshots} S)` : name };
    }));
    const monthCount = computed(() => {
      const t = totals.value.get(`${String(view.value.year).padStart(4, '0')}-${String(view.value.month + 1).padStart(2, '0')}`);
      return t ? countText(t.marks, t.snapshots) : 'nothing';
    });

    const day = computed(() => {
      if (selected.value === 'undated') {
        const u = index.value.undated;
        return { key: 'undated', marks: u.filter((e) => e.kind === 'mark'), snapshots: u.filter((e) => e.kind === 'snapshot') };
      }
      return index.value.days.get(selected.value) || null;
    });
    const dayTitle = computed(() => {
      if (selected.value === 'undated') return 'Without a time';
      const p = parseDayKey(selected.value);
      return p ? dayLabel(selected.value, null).replace(/: nothing$/, '') : '';
    });

    function focusDay(key) {
      cursor.value = key;
      nextTick(() => {
        const el = grid.value && grid.value.querySelector(`[data-day="${key}"]`);
        if (el) el.focus({ preventScroll: true });
      });
    }

    /** Shows the month of `key` and puts the cursor (and, with `select`, the panel) on it. */
    function goTo(key, { select = true, focus = false } = {}) {
      const p = parseDayKey(key);
      if (!p) return;
      view.value = { year: p.year, month: p.month };
      cursor.value = key;
      if (select) selectDay(key);
      if (focus) focusDay(key);
    }

    function selectDay(key) {
      selected.value = key;
      shown.value = { mark: PAGE, snapshot: PAGE };
    }

    function moveMonth(delta) {
      const next = addMonths(view.value, delta);
      const p = parseDayKey(cursor.value);
      view.value = next;
      cursor.value = sameDayIn(next, p ? p.day : 1);
    }
    function setYear(year) {
      moveMonth((Number(year) - view.value.year) * 12);
    }
    function setMonth(month) {
      moveMonth(Number(month) - view.value.month);
    }

    function onGridKey(event) {
      const moves = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -7, ArrowDown: 7 };
      const p = parseDayKey(cursor.value);
      let key = null;
      if (event.key in moves) key = addDays(cursor.value, moves[event.key]);
      else if (event.key === 'PageUp' || event.key === 'PageDown') {
        const next = addMonths(view.value, event.key === 'PageUp' ? -1 : 1);
        key = sameDayIn(next, p ? p.day : 1);
      } else if (event.key === 'Home' || event.key === 'End') {
        const col = cells.value.findIndex((c) => c.key === cursor.value) % 7;
        key = addDays(cursor.value, event.key === 'Home' ? -col : 6 - col);
      } else return;
      event.preventDefault();
      goTo(key, { select: false, focus: true });
    }

    // A filter that leaves the listed day empty moves to the newest day it keeps.
    watch([text, () => ({ ...kinds.value })], () => {
      if (!isOpen.value || selected.value === 'undated') return;
      if (!index.value.days.has(selected.value) && index.value.newest) goTo(index.value.newest);
    });

    function open() {
      today.value = todayKey();
      text.value = '';
      kinds.value = { mark: true, snapshot: true };
      isOpen.value = true;
      if (dlg.value && !dlg.value.open) dlg.value.showModal();
      const start = index.value.newest || today.value;
      goTo(start, { select: !!index.value.newest, focus: true });
      if (!index.value.newest) selected.value = index.value.undated.length ? 'undated' : '';
    }
    function close() {
      if (dlg.value && dlg.value.open) dlg.value.close();
    }
    function onClose() {
      isOpen.value = false;
    }

    function act(id, e) {
      emit('action', id, e.kind === 'mark' ? markTarget(e.name) : commitTarget(e.commitSeq));
      if (id === 'view') close();
    }
    const disabled = (id, e) => actionDisabled(props.store, id, e.commitSeq, props.locked);

    expose({ open, close });
    return {
      formatCount,
      dlg, grid, isOpen, view, cursor, selected, text, kinds, shown, today, index, years, weekdays,
      rows, monthOptions, monthCount, day, dayTitle, MONTHS, PAGE,
      goTo, selectDay, moveMonth, setYear, setMonth, onGridKey, close, onClose, act, disabled,
      emit,
    };
  },
  template: `
  <dialog ref="dlg" class="store-dialog store-calendar" aria-labelledby="store-calendar-title" @close="onClose" @click.self="close">
    <div class="dialog-head">
      <h2 id="store-calendar-title">History: marks and snapshots</h2><span class="spacer"></span>
      <button type="button" class="icon-btn" @click="close" aria-label="Close">×</button>
    </div>
    <div v-if="isOpen" class="dialog-body">
      <div class="cal-filter">
        <input class="input" type="text" v-model="text" placeholder="Filter: name or #commit" aria-label="Filter by name or commit number" style="flex:1;min-width:120px">
        <label class="cal-kind"><input type="checkbox" v-model="kinds.mark"> <span class="cal-badge m3" aria-hidden="true">M</span> Marks</label>
        <label class="cal-kind"><input type="checkbox" v-model="kinds.snapshot"> <span class="cal-badge s3" aria-hidden="true">S</span> Snapshots</label>
      </div>
      <div class="cal-layout">
        <div class="cal-main">
          <div class="cal-nav">
            <button type="button" class="btn small" @click="moveMonth(-1)" aria-label="Previous month" title="Previous month (PageUp)">‹</button>
            <select class="input" :value="view.month" @change="setMonth($event.target.value)" aria-label="Month">
              <option v-for="m in monthOptions" :key="m.month" :value="m.month">{{ m.text }}</option>
            </select>
            <select class="input" :value="view.year" @change="setYear($event.target.value)" aria-label="Year">
              <option v-for="y in years" :key="y" :value="y">{{ y }}</option>
            </select>
            <button type="button" class="btn small" @click="moveMonth(1)" aria-label="Next month" title="Next month (PageDown)">›</button>
            <span class="spacer"></span>
            <button type="button" class="btn small" @click="goTo(today, { focus: true })" title="The month of today">Today</button>
            <button type="button" class="btn small" :disabled="!index.newest" @click="goTo(index.newest, { focus: true })" title="The day of the newest mark or snapshot">Newest</button>
            <button type="button" class="btn small" :disabled="!index.oldest" @click="goTo(index.oldest, { focus: true })" title="The day of the oldest mark or snapshot">Oldest</button>
          </div>
          <div class="cal-month muted" aria-live="polite">{{ MONTHS[view.month] }} {{ view.year }}: {{ monthCount }}</div>
          <table ref="grid" class="cal-grid" role="grid" :aria-label="MONTHS[view.month] + ' ' + view.year" @keydown="onGridKey">
            <thead><tr><th v-for="w in weekdays" :key="w" scope="col">{{ w }}</th></tr></thead>
            <tbody>
              <tr v-for="(row, i) in rows" :key="i">
                <td v-for="c in row" :key="c.key" role="gridcell" :aria-selected="String(c.key === selected)">
                  <button type="button" class="cal-day" :data-day="c.key"
                          :class="['t' + c.tBucket, { out: !c.inMonth, today: c.today, sel: c.key === selected }]"
                          :tabindex="c.key === cursor ? 0 : -1" :aria-label="c.label" :aria-current="c.today ? 'date' : null"
                          @click="goTo(c.key)" @focus="cursor = c.key">
                    <span class="cal-num">{{ c.day }}</span>
                    <span class="cal-badges" aria-hidden="true">
                      <span v-if="c.marks" class="cal-badge" :class="'m' + c.mBucket" :title="c.marks + ' mark(s)'">{{ c.marks }}</span>
                      <span v-if="c.snapshots" class="cal-badge" :class="'s' + c.sBucket" :title="c.snapshots + ' snapshot(s)'">{{ c.snapshots }}</span>
                    </span>
                  </button>
                </td>
              </tr>
            </tbody>
          </table>
          <div class="cal-legend muted">
            <span><span class="cal-badge m2">n</span> marks</span>
            <span><span class="cal-badge s2">n</span> snapshots</span>
            <span>deeper colour: more that day · local time</span>
            <button v-if="index.undated.length" type="button" class="btn small" @click="selectDay('undated')">{{ index.undated.length }} without a time</button>
          </div>
        </div>

        <section class="cal-day-panel" aria-live="polite" :aria-label="dayTitle || 'Selected day'">
          <p v-if="!selected" class="muted">Choose a day.</p>
          <template v-else>
            <h3>{{ dayTitle }}</h3>
            <p v-if="!day" class="muted">No marks or snapshots{{ text || !kinds.mark || !kinds.snapshot ? ' match the filter' : '' }} on this day.</p>
            <template v-else>
              <template v-if="day.marks.length">
                <div class="menu-title">Marks ({{ day.marks.length }})</div>
                <div v-for="e in day.marks.slice(0, shown.mark)" :key="e.id" class="store-row cal-row">
                  <div class="store-row-head">
                    <span><span class="cal-time">{{ e.clock }}</span> <b>{{ e.name }}</b></span><small>commit {{ e.commitSeq }}</small>
                  </div>
                  <div class="store-actions">
                    <button v-for="a in actions" :key="a.id" type="button" class="btn small" :disabled="disabled(a.id, e)"
                            :title="a.title" @click="act(a.id, e)">{{ a.short }}</button>
                  </div>
                </div>
                <button v-if="day.marks.length > shown.mark" type="button" class="btn small" @click="shown.mark += PAGE">Show {{ Math.min(PAGE, day.marks.length - shown.mark) }} more</button>
              </template>
              <template v-if="day.snapshots.length">
                <div class="menu-title">Snapshots ({{ day.snapshots.length }})</div>
                <div v-for="e in day.snapshots.slice(0, shown.snapshot)" :key="e.id" class="store-row cal-row">
                  <div class="store-row-head">
                    <span><span class="cal-time">{{ e.clock }}</span> #{{ e.commitSeq }}<b v-if="e.name">&nbsp;{{ e.name }}</b></span>
                    <small>{{ formatCount(e.raw.vertexCount) }} V · {{ formatCount(e.raw.edgeCount) }} E</small>
                  </div>
                  <div class="store-actions">
                    <button v-for="a in actions" :key="a.id" type="button" class="btn small" :disabled="disabled(a.id, e)"
                            :title="a.title" @click="act(a.id, e)">{{ a.short }}</button>
                    <a v-if="snapshotUrl(e.commitSeq)" class="btn small" :href="snapshotUrl(e.commitSeq)" :download="'graph-' + e.commitSeq + '.gsnap'"
                       title="Download this snapshot as one packed file (.gsnap)">.gsnap</a>
                    <button v-else type="button" class="btn small" @click="emit('download', e.commitSeq)"
                            title="Download this snapshot as one packed file (.gsnap)">.gsnap</button>
                  </div>
                </div>
                <button v-if="day.snapshots.length > shown.snapshot" type="button" class="btn small" @click="shown.snapshot += PAGE">Show {{ Math.min(PAGE, day.snapshots.length - shown.snapshot) }} more</button>
              </template>
            </template>
          </template>
        </section>
      </div>
    </div>
  </dialog>
  `,
};
