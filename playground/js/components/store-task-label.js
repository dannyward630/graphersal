// The label of a Store action's button (the Store menu and its dialog): the idle text, or, while
// that action runs (`state.storeTask`, see `runStoreTask` in js/store.js), an inline spinner, the
// running text ("Verifying the store…") and the elapsed time ticking ("0:42"). The button itself
// carries the class `store-running` (bold, accent colour) and `aria-busy` (see `taskButton` in
// js/store-model.js).

import { state } from '../store.js';
import { isRunning, runningTask } from '../store-model.js';

export const StoreTaskLabel = {
  props: {
    /** The action (a key of `STORE_TASKS`). */
    task: { type: String, required: true },
    /** The idle text. */
    label: { type: String, required: true },
    /** The item's key among several of the same action (`isRunning`); none: any. */
    taskKey: { type: String, default: undefined },
  },
  setup() {
    return { state, isRunning, runningTask };
  },
  template: `<template v-if="isRunning(state.storeTask, task, taskKey)"><span class="store-spinner" aria-hidden="true"></span>{{ runningTask(state.storeTask).running }} <span class="store-elapsed">{{ runningTask(state.storeTask).elapsed }}</span></template><template v-else>{{ label }}</template>`,
};
