// Shows an engine error: message, step path, position, the engine's help() text and the full
// diagnostic.

import { jumpTo } from '../store.js';

export const ErrorView = {
  props: { error: { type: Object, required: true } },
  setup(props) {
    const short = (name) => (name.length > 48 ? name.slice(0, 45) + '…' : name);
    return { jump: () => jumpTo(props.error.position), short };
  },
  template: `
  <div class="error-view" role="alert">
    <div class="error-head">
      <span aria-hidden="true">✗</span>
      <span class="msg">{{ error.message }}</span>
    </div>
    <div class="error-body">
      <nav v-if="error.step_path && error.step_path.length" class="step-path" aria-label="Where the error happened">
        <span class="faint">in step</span>
        <template v-for="(s, i) in error.step_path" :key="s.id">
          <span v-if="i" class="sep" aria-hidden="true">›</span>
          <code :title="s.id + ': ' + s.name" :class="{ last: i === error.step_path.length - 1 }">{{ short(s.name) }}</code>
        </template>
        <span class="faint mono">({{ error.step_id }})</span>
      </nav>
      <div v-if="error.help">
        <p class="help-title">How to fix it</p>
        <p class="help">{{ error.help }}</p>
      </div>
      <details v-if="error.diagnostic" class="diagnostic">
        <summary>Full diagnostic (as the CLI prints it)</summary>
        <pre class="code-view">{{ error.diagnostic }}</pre>
      </details>
      <div class="error-actions">
        <button v-if="error.position" class="btn small" @click="jump">
          Go to line {{ error.position.line }}, column {{ error.position.column }}
        </button>
      </div>
    </div>
  </div>
  `,
};
