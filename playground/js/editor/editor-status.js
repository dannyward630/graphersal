// The texts of the Query panel's status bar (components/query-editor.js), pure so they are tested
// under Node: the run status of the active tab and the cursor position in its long and its compact
// form (the bar shows one of them by the panel's width, see .editor-status in css/app.css).

import { formatMs } from '../util.js';
import { selectionText } from './run-selection.js';

/**
 * The status of the active tab when it is not running itself: `Ready`, or the last run
 * (`Run #3 · 6 results · 1.2 ms`; a run of the editor's selection adds `selection (N lines)` after the
 * number), plus `possibly stale` and a query running in another tab.
 * `result` is the tab's last result (or null), `stale` its flag, `elsewhere` the running other
 * tab's name (or null) and `elapsedMs` how long that run takes so far.
 */
export function runStatusText({ result, stale = false, elsewhere = null, elapsedMs = 0 }) {
  const time = elapsedMs > 0 ? ` · ${formatMs(elapsedMs)}` : '';
  const other = elsewhere !== null && elsewhere !== undefined ? ` · running in "${elsewhere}"${time}` : '';
  if (!result) return `Ready${other}`;
  const staleText = stale ? ' · possibly stale' : '';
  const head = `Run #${result.run_id}${result.selection ? ` · ${selectionText(result.selection.lines)}` : ''}`;
  if (result.stopped) return `${head} · stopped${other}`;
  if (!result.ok) return `${head} · error · ${formatMs(result.elapsed_ms)}${staleText}${other}`;
  const total = result.display ? result.display.total_rows : undefined;
  const rows = total !== undefined && total !== null ? `${total} result${total === 1 ? '' : 's'}` : 'done';
  return `${head} · ${rows} · ${formatMs(result.elapsed_ms)}${staleText}${other}`;
}

/** The cursor position `{line, col}` (1-based) as `{long: 'Ln 1, Col 30', short: '1:30', title}`. */
export function cursorTexts({ line, col }) {
  return { long: `Ln ${line}, Col ${col}`, short: `${line}:${col}`, title: `Line ${line}, column ${col}` };
}
