// The history calendar of the Store menu: the store's snapshots and marks grouped by LOCAL day,
// a month grid, colour buckets, keyboard moves between days, and the "most recent few" lists of
// the menu. Pure functions, no Vue, no DOM (tested under Node: tests/store-calendar.test.mjs).
//
// A store can hold thousands of snapshots and marks: everything here is computed once per store
// answer (`historyIndex`: one pass over the entries, one sort), and a month cell only looks its
// day up in a Map, it never scans the entries.
//
// Times are the engine's microseconds since the epoch (`GET /api/store`: a mark's `time`, a
// snapshot's `lastCommitAt`, the time of the commit it holds, or `createdAt` for a snapshot of a
// state without a commit). The local day comes from an offset function (minutes east of UTC at a
// given instant; default: this browser's time zone), so tests pin a zone.

/** How many snapshots and marks the Store menu shows before "Calendar…". */
export const RECENT_COUNT = 5;

/** The number of colour steps of a day (0 = nothing that day, 1..4 = more). */
export const BUCKETS = 4;

/** The month names, January first. */
export const MONTHS = Object.freeze([
  'January', 'February', 'March', 'April', 'May', 'June',
  'July', 'August', 'September', 'October', 'November', 'December',
]);

/** The weekday names, Sunday first (`Date#getDay`). */
const WEEKDAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];

/** This browser's offset at an instant (ms), minutes east of UTC. */
export function localOffset(ms) {
  return -new Date(ms).getTimezoneOffset();
}

const pad = (n, w = 2) => String(n).padStart(w, '0');

/** A day key `YYYY-MM-DD` of a year, a month (0-11) and a day (1-31). */
export function dayKey(year, month, day) {
  return `${pad(year, 4)}-${pad(month + 1)}-${pad(day)}`;
}

/** `2026-10-08` -> `{year: 2026, month: 9, day: 8}` (`null` for anything else). */
export function parseDayKey(key) {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(String(key || ''));
  return m ? { year: Number(m[1]), month: Number(m[2]) - 1, day: Number(m[3]) } : null;
}

/**
 * The local calendar fields of a time in microseconds: `{key, year, month, day, hh, mm, ss}`,
 * `null` for a missing time (0 or below: the engine's "no commit yet").
 */
export function localParts(micros, offset = localOffset) {
  const us = Number(micros);
  if (!Number.isFinite(us) || us <= 0) return null;
  const ms = Math.floor(us / 1000);
  const d = new Date(ms + offset(ms) * 60000);
  const year = d.getUTCFullYear();
  const month = d.getUTCMonth();
  const day = d.getUTCDate();
  return {
    key: dayKey(year, month, day), year, month, day,
    hh: d.getUTCHours(), mm: d.getUTCMinutes(), ss: d.getUTCSeconds(),
  };
}

/** `14:03:09` of local parts. */
export function clockText(parts) {
  return parts ? `${pad(parts.hh)}:${pad(parts.mm)}:${pad(parts.ss)}` : '-';
}

/** A snapshot's time: the commit it holds, else when it was written. */
function snapshotTime(s) {
  return Number(s.lastCommitAt) > 0 ? Number(s.lastCommitAt) : Number(s.createdAt) || 0;
}

/**
 * The snapshots and marks of a store answer as one list of entries, newest first (commit, then
 * time; a snapshot before a mark at the same commit and time): `{kind: 'snapshot'|'mark', id,
 * commitSeq, time, name, parts, clock, raw}`. `id` is unique (`s:<commit>`, `m:<name>`).
 */
export function historyEntries(store, offset = localOffset) {
  const out = [];
  for (const s of (store && store.snapshots) || []) {
    const time = snapshotTime(s);
    const parts = localParts(time, offset);
    out.push({
      kind: 'snapshot', id: `s:${s.commitSeq}`, commitSeq: Number(s.commitSeq), time,
      name: s.name || '', parts, clock: clockText(parts), raw: s,
    });
  }
  for (const m of (store && store.marks) || []) {
    const time = Number(m.time) || 0;
    const parts = localParts(time, offset);
    out.push({
      kind: 'mark', id: `m:${m.name}`, commitSeq: Number(m.commitSeq), time,
      name: String(m.name), parts, clock: clockText(parts), raw: m,
    });
  }
  out.sort(newestFirst);
  return out;
}

function newestFirst(a, b) {
  return (b.commitSeq - a.commitSeq) || (b.time - a.time)
    || (a.kind === b.kind ? 0 : (a.kind === 'snapshot' ? -1 : 1))
    || (a.name < b.name ? -1 : a.name > b.name ? 1 : 0);
}

/**
 * The menu's short lists: the `n` newest snapshots and marks (newest first) and how many there
 * are in all.
 */
export function recentHistory(store, n = RECENT_COUNT) {
  const byCommit = (a, b) => (Number(b.commitSeq) - Number(a.commitSeq))
    || ((Number(b.time ?? b.lastCommitAt) || 0) - (Number(a.time ?? a.lastCommitAt) || 0));
  const snapshots = ((store && store.snapshots) || []).slice().sort(byCommit);
  const marks = ((store && store.marks) || []).slice().sort(byCommit);
  return {
    snapshots: snapshots.slice(0, n),
    marks: marks.slice(0, n),
    snapshotCount: snapshots.length,
    markCount: marks.length,
    more: snapshots.length > n || marks.length > n,
  };
}

/**
 * Keeps the entries a filter admits: `kinds` `{snapshot, mark}` (both by default) and a text
 * matched case-insensitively against the name (a mark's, or a named snapshot's) or `#<commit>`.
 */
export function filterEntries(entries, { text = '', kinds = { snapshot: true, mark: true } } = {}) {
  const q = String(text || '').trim().toLowerCase();
  const commit = /^#?\d+$/.test(q) ? Number(q.replace('#', '')) : null;
  return entries.filter((e) => {
    if (!kinds[e.kind]) return false;
    if (!q) return true;
    if (commit !== null && e.commitSeq === commit) return true;
    return e.name.toLowerCase().includes(q);
  });
}

/**
 * Groups entries (newest first, as `historyEntries` gives them) by local day, once:
 * `{days: Map<key, {key, marks, snapshots, total}>, undated, newest, oldest, max}`. `newest` /
 * `oldest` are day keys (`null` when nothing is dated), `max` the largest per-day count of each
 * kind and of both (`{mark, snapshot, total}`) for the colour scale, `undated` the entries without
 * a time.
 */
export function historyIndex(entries) {
  const days = new Map();
  const undated = [];
  const max = { mark: 0, snapshot: 0, total: 0 };
  let newest = null;
  let oldest = null;
  for (const e of entries) {
    if (!e.parts) {
      undated.push(e);
      continue;
    }
    const key = e.parts.key;
    let day = days.get(key);
    if (!day) {
      day = { key, marks: [], snapshots: [], total: 0 };
      days.set(key, day);
    }
    (e.kind === 'mark' ? day.marks : day.snapshots).push(e);
    day.total++;
    if (newest === null || key > newest) newest = key;
    if (oldest === null || key < oldest) oldest = key;
  }
  for (const day of days.values()) {
    // Within a day the newest time first (the commit order may differ after a rollback).
    day.marks.sort((a, b) => b.time - a.time || b.commitSeq - a.commitSeq);
    day.snapshots.sort((a, b) => b.time - a.time || b.commitSeq - a.commitSeq);
    if (day.marks.length > max.mark) max.mark = day.marks.length;
    if (day.snapshots.length > max.snapshot) max.snapshot = day.snapshots.length;
    if (day.total > max.total) max.total = day.total;
  }
  return { days, undated, newest, oldest, max };
}

/**
 * The colour step of a count against the largest count: 0 for none, else 1..BUCKETS on a
 * logarithmic scale (one busy day must not wash every other day out).
 */
export function bucket(count, max) {
  if (!(count > 0)) return 0;
  if (!(max > 1)) return 1;
  const step = 1 + Math.floor((BUCKETS - 1) * (Math.log(count) / Math.log(max)) + 1e-9);
  return Math.min(BUCKETS, Math.max(1, step));
}

/** The number of days of a month (0-11). */
export function daysInMonth(year, month) {
  return new Date(Date.UTC(year, month + 1, 0)).getUTCDate();
}

/** The weekday (0 = Sunday) of a date. */
function weekday(year, month, day) {
  return new Date(Date.UTC(year, month, day)).getUTCDay();
}

/** The weekday names of the grid's columns, starting at `weekStart` (1 = Monday). */
export function weekdayNames(weekStart = 1) {
  return Array.from({ length: 7 }, (_, i) => WEEKDAYS[(weekStart + i) % 7]);
}

/**
 * The cells of a month as whole weeks starting at `weekStart` (1 = Monday): `{key, year, month,
 * day, inMonth}`, the days of the neighbouring months filling the first and last week. 4 to 6
 * rows of 7.
 */
export function monthGrid(year, month, weekStart = 1) {
  const lead = (weekday(year, month, 1) - weekStart + 7) % 7;
  const count = daysInMonth(year, month);
  const rows = Math.ceil((lead + count) / 7);
  const cells = [];
  for (let i = 0; i < rows * 7; i++) {
    const d = new Date(Date.UTC(year, month, 1 - lead + i));
    const y = d.getUTCFullYear();
    const mo = d.getUTCMonth();
    const day = d.getUTCDate();
    cells.push({ key: dayKey(y, mo, day), year: y, month: mo, day, inMonth: mo === month && y === year });
  }
  return cells;
}

/** A day key moved by `delta` days (arrows: ±1, ±7). */
export function addDays(key, delta) {
  const p = parseDayKey(key);
  if (!p) return key;
  const d = new Date(Date.UTC(p.year, p.month, p.day + delta));
  return dayKey(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate());
}

/** `{year, month}` moved by `delta` months. */
export function addMonths({ year, month }, delta) {
  const n = year * 12 + month + delta;
  return { year: Math.floor(n / 12), month: ((n % 12) + 12) % 12 };
}

/** The key of a day moved to another month, keeping the day when it exists there. */
export function sameDayIn({ year, month }, day) {
  return dayKey(year, month, Math.min(day, daysInMonth(year, month)));
}

/** Today's local day key. */
export function todayKey(nowMs = Date.now(), offset = localOffset) {
  return localParts(nowMs * 1000, offset).key;
}

/**
 * The years the year selector offers, newest first: from the oldest entry's year to the newest's,
 * widened to today's year and to the year shown (`shownYear`, which a filter may have left
 * without entries).
 */
export function yearRange(index, today, shownYear) {
  const years = [index.oldest, index.newest, today].filter(Boolean).map((k) => parseDayKey(k).year);
  if (Number.isInteger(shownYear)) years.push(shownYear);
  const lo = Math.min(...years);
  const hi = Math.max(...years);
  const out = [];
  for (let y = hi; y >= lo; y--) out.push(y);
  return out;
}

/**
 * Per month (`YYYY-MM`) of the index, how many marks and snapshots it holds: the month selector
 * shows them so a long history can be found without paging. Computed once over the days.
 */
export function monthTotals(index) {
  const totals = new Map();
  for (const day of index.days.values()) {
    const m = day.key.slice(0, 7);
    const t = totals.get(m) || { marks: 0, snapshots: 0 };
    t.marks += day.marks.length;
    t.snapshots += day.snapshots.length;
    totals.set(m, t);
  }
  return totals;
}

/** `2 marks, 14 snapshots` (empty parts left out; `nothing` when both are 0). */
export function countText(marks, snapshots) {
  const parts = [];
  if (marks) parts.push(`${marks} mark${marks === 1 ? '' : 's'}`);
  if (snapshots) parts.push(`${snapshots} snapshot${snapshots === 1 ? '' : 's'}`);
  return parts.join(', ') || 'nothing';
}

/** The accessible name of a day cell: `Thursday, 8 October 2026: 2 marks, 14 snapshots`. */
export function dayLabel(key, day) {
  const p = parseDayKey(key);
  if (!p) return key;
  const names = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'];
  const what = day ? countText(day.marks.length, day.snapshots.length) : 'nothing';
  return `${names[weekday(p.year, p.month, p.day)]}, ${p.day} ${MONTHS[p.month]} ${p.year}: ${what}`;
}
