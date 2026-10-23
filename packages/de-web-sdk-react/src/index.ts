// Replaced at build/test time with the root Cargo.toml workspace version
// (packages/scripts/workspace-version.mjs); never hard-code it here.
declare const __DE_VERSION__: string;
export const VERSION: string = __DE_VERSION__;
export { DesignProvider, useViewer } from './use-viewer.js';
export { useDocument, useViewport, useActivePage } from './hooks.js';
export { DesignView, type DesignViewProps } from './design-view.js';
