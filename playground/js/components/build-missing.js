// Shown instead of the playground when the WebAssembly engine has not been built
// (playground/pkg/ is missing): how to build it, instead of a broken UI.

export const BuildMissing = {
  setup() {
    return { origin: window.location.origin };
  },
  template: `
  <div class="start">
    <header class="topbar">
      <a class="brand" href="#/">Graphersal <small>Playground</small></a>
    </header>
    <div class="start-inner">
      <h1>The engine is not built yet</h1>
      <p class="lead">
        The playground runs the real Graphersal engine compiled to WebAssembly. Its build output,
        <code>playground/pkg/</code>, is not committed and was not found next to this page.
      </p>
      <h2>Build it</h2>
      <p>One-time prerequisites:</p>
      <pre class="code-view">rustup target add wasm32-unknown-unknown
cargo install --locked wasm-bindgen-cli --version 0.2.129
# optional, makes the module smaller: wasm-opt from binaryen (e.g. brew install binaryen)</pre>
      <p>Then, from the repository root:</p>
      <pre class="code-view">playground/build.sh
python3 -m http.server -d playground 8000</pre>
      <p>and reload <a :href="origin + '/'">{{ origin }}/</a>. See <code>playground/README.md</code> for details.</p>
    </div>
  </div>
  `,
};
