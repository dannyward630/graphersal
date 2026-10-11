// Where the floating CodeMirror tooltips of an editor go (js/editor/tooltips.js); no CodeMirror
// import, so it is tested under Node.

/**
 * The element the tooltips of an editor mounted in `host` are appended to: the modal <dialog>
 * the editor sits in (the top layer covers everything else), else the document body.
 */
export function tooltipParent(host) {
  const dialog = host && typeof host.closest === 'function' ? host.closest('dialog') : null;
  if (dialog) return dialog;
  return host && host.ownerDocument ? host.ownerDocument.body : document.body;
}
