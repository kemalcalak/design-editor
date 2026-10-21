/**
 * The one live instance of the Rust logic core, and nothing else.
 *
 * Everything the extension decides without a browser API lives in
 * `crates/op-chrome-extension-core`, compiled to `wasm32-unknown-unknown` and
 * bound with `wasm-bindgen --target web`. `scripts/build-wasm.sh` drops the
 * `.wasm` plus its generated ES-module shim into `wasm/`; that directory is a
 * build product and is not checked in.
 *
 * # Why this file exists separately from the loaders
 *
 * The extension has two entry points that need the core, and they cannot load
 * it the same way:
 *
 * * The **popup** is a document, so it can `import()` the shim dynamically —
 *   and must, because a static import of a build product that has not been
 *   built yet would abort the popup script before any handler is attached and
 *   present as a blank window with no explanation. `wasm-core.js` owns that.
 * * The **service worker** cannot. `import()` is disallowed on
 *   `ServiceWorkerGlobalScope` by the HTML specification (w3c/ServiceWorker
 *   issue 1356) — not "only after startup", but at all, and Chrome rejects the
 *   call itself. It therefore imports the shim with a static `import`
 *   declaration and initializes it by URL. `background.js` owns that.
 *
 * `capture.js` and `client.js` are used from both, so they must not depend on
 * either strategy. They reach the core through {@link getCore} here, and this
 * module imports nothing at all — which is also what keeps the service
 * worker's static import graph provably free of a dynamic `import(`.
 */

/** Initialized module namespace, or null before the first successful load. */
let core = null;

/**
 * Build a core-related failure.
 *
 * `code` distinguishes the two situations that used to be conflated, because
 * they call for different advice:
 *
 * * `wasmMissing` — the shim is not there. The checkout has not been built.
 * * `wasmInit` — the shim is there and would not initialize. The build is
 *   present but broken, or the environment refused to instantiate it. Telling
 *   the user to run the build script would be a misdiagnosis.
 */
export function coreError(code, cause) {
  const detail = cause && cause.message ? cause.message : cause;
  const error = new Error(`the OpenPencil logic core could not be loaded: ${detail}`);
  error.code = code;
  return error;
}

/** Record a freshly initialized core. */
export function setCore(module) {
  core = module;
  return module;
}

/**
 * The already-loaded core.
 *
 * Callers on a path only reached after the owning entry point finished
 * loading use this so they stay synchronous.
 *
 * @throws {Error} when called before the core finished loading.
 */
export function getCore() {
  if (!core) throw coreError('wasmInit', 'the core was used before it finished loading');
  return core;
}

/** Whether a core has been initialized in this context. */
export function hasCore() {
  return core !== null;
}

/**
 * Drop the cached instance so the next load builds a fresh one.
 *
 * A wasm trap (an out-of-memory allocation on a very large capture, say)
 * leaves the module instance permanently unusable: its memory is poisoned and
 * every later call traps too. Without this, one failed capture would make
 * every subsequent action fail for a reason that has nothing to do with it.
 * Re-instantiating costs a few milliseconds against a local file.
 *
 * Safe to call at any time; a caller still holding a reference from
 * {@link getCore} keeps using the old instance, which is exactly what an
 * in-flight operation wants.
 */
export function invalidateCore() {
  core = null;
}

/** Whether `error` is a wasm trap, i.e. a reason to {@link invalidateCore}. */
export function isCoreTrap(error) {
  return (
    typeof WebAssembly !== 'undefined' &&
    WebAssembly.RuntimeError !== undefined &&
    error instanceof WebAssembly.RuntimeError
  );
}
