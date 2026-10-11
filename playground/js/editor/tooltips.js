// CodeMirror tooltips (the completion list and its info panel) that are never clipped: they are
// rendered with `position: fixed` into a parent outside the panel, whose `overflow: hidden` would
// cut them otherwise. That parent is the document body, or the modal <dialog> the editor sits in:
// a modal dialog is in the browser's top layer, above everything in the body whatever its
// z-index, so a body-parented tooltip would end up behind it. CodeMirror keeps a fixed tooltip at
// the cursor itself (it measures again on scrolling and on any change of the editor's geometry,
// such as a panel resize). The z-index is set in css/app.css (.cm-tooltip).

import { tooltips } from '@codemirror/view';
import { tooltipParent } from './tooltip-parent.js';

/** The tooltip extension for an editor mounted in `host` (an element already in the document). */
export function floatingTooltips(host) {
  return tooltips({ position: 'fixed', parent: tooltipParent(host) });
}
