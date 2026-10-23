// Public entry for the Design Editor read-only web SDK core.
// Replaced at build/test time with the root Cargo.toml workspace version
// (packages/scripts/workspace-version.mjs); never hard-code it here.
declare const __DE_VERSION__: string;
export const VERSION: string = __DE_VERSION__;
export { createViewer, OpViewer } from './viewer.js';
export type { Viewport, CreateViewerOptions, PenDocument, PenPage } from './types.js';
export type { ViewerEvent } from './events.js';
