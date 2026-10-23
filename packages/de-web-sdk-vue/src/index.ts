// Replaced at build/test time with the root Cargo.toml workspace version
// (packages/scripts/workspace-version.mjs); never hard-code it here.
declare const __DE_VERSION__: string;
export const VERSION: string = __DE_VERSION__;
export { viewerKey } from './injection.js';
export { provideViewer, provideViewerRef, useViewer } from './use-viewer.js';
export { useDocument, useViewport, useActivePage } from './composables.js';
export { DesignView } from './design-view.js';
