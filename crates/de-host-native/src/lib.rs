//! Design Editor shell — native (desktop) backend.
//!
//! Per spec v19 §1.2 (FROZEN 2026-05-04): this crate must NOT be linked into
//! the wasm32-unknown-unknown web bundle. Even though some deps (winit) compile
//! silently on wasm32 via web-sys, we use an explicit compile_error! guard to
//! make accidental inclusion a hard error.
//!
//! Step 1a Task 2: this crate now ships [`SharedSkiaContext`] (own GL stack +
//! Skia DirectContext + Surface, idempotent teardown, lifecycle hooks),
//! [`CanvasViewportStub`] (GL-state-isolation probe), and [`NativeBackend`]
//! (frame-scoped widget facade that translates OP `RenderBackend` calls into
//! Jian `DrawOp`s and submits via `jian_skia::SkiaBackend::draw_on_canvas`).
//!
//! Module layout (spec v19 §2 / §3 / §5.2.1):
//! - [`context`] — `GlContextProvider` trait + `GlutinProvider` desktop impl;
//!   `SharedSkiaContext` owning the GL stack.
//! - [`canvas_view_stub`] — `CanvasViewportStub::render_into` (deliberately
//!   pollutes GL state to verify chrome paint isolation).
//! - [`backend`] — `NativeBackend` exposing frame-scoped methods mirroring the
//!   OP `RenderBackend` trait surface (no direct trait impl in Step 1a; see
//!   spec §5.2.1).

#[cfg(target_arch = "wasm32")]
compile_error!(
    "de-host-native must NOT be compiled for wasm32 targets. \
     Use de-host-web for browser builds (spec v19 §1.2)."
);

// Context module: re-exports the `GlContextProvider` trait +
// `ProviderError` / `ProviderResult` on every (non-wasm) target;
// `GlutinProvider` and `SharedSkiaContext` are compiled in with `gl-host`.
pub mod context;

// Render stack: `backend` (NativeBackend) and `widget_host`
// (NativeFrameBackend + WidgetHostNative) compile on every desktop target
// where `skia-safe` + `jian-skia` compile (see Cargo.toml target-gated deps).
// `canvas_view_stub` additionally needs `gl-host` — it pulls `glow` for a
// GL-state-isolation probe.
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
pub mod backend;
#[cfg(feature = "gl-host")]
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
pub mod boolean_ops;
#[cfg(feature = "gl-host")]
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
pub mod canvas_view_stub;
// The editor-UI composition host. Its GUI-runner glue (preview auto-wire)
// is gl-host-gated inside.
#[cfg(feature = "widget-host")]
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
pub mod widget_host;
// Canvas Preview (Play) mode runtime owner — depends on the OP
// `RenderBackend` trait (same gate as `widget_host`).
#[cfg(feature = "widget-host")]
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
pub use de_preview_core as preview;

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
pub use backend::{
    begin_image_paint_diagnostics, decode_raster, decode_raster_capped,
    end_image_paint_diagnostics, image_paint_diagnostics_snapshot, to_jian_rect,
    ImagePaintDiagnostics, NativeBackend, NativeFrameBackend,
};
#[cfg(feature = "widget-host")]
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
pub use preview::{PreviewEnterError, PreviewLayoutError, PreviewSession};
#[cfg(feature = "widget-host")]
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
pub use widget_host::{CursorHint, WidgetHostNative};

// canvas_view_stub stays desktop-only (uses glow GL-isolation probe).
#[cfg(feature = "gl-host")]
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
pub use canvas_view_stub::CanvasViewportStub;

// Cross-platform re-exports — visible on every (non-wasm) target.
pub use context::{GlContextProvider, ProviderError, ProviderResult};

// Desktop-only re-exports.
#[cfg(feature = "gl-host")]
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
pub use context::{
    GlutinProvider, SharedSkiaContext, SharedSkiaError, SharedSkiaResult, SurfaceConfig,
};

// `placeholder()` from Task 1 was removed by Codex Phase A Gate round 1
// NIT 1 — Task 2's full re-export chain (`SharedSkiaContext`,
// `NativeBackend`, etc.) already proves the shell-core ↔ shell-native
// link, and Task 3's `AppShell::run_desktop` will be the canonical
// entry. Keeping the dead helper just to satisfy a removed link-check
// is YAGNI.

#[cfg(test)]
pub(crate) mod font_registry_test_support {
    use std::sync::{LazyLock, Mutex, MutexGuard};

    static LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    pub(crate) fn lock() -> MutexGuard<'static, ()> {
        LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }
}

// All remaining users live in gl-host-gated `widget_host` test files,
// so the lock must share that gate or a plain `cargo clippy` (no
// gl-host) sees it as dead code.
#[cfg(all(
    test,
    feature = "widget-host",
    any(target_os = "macos", target_os = "linux", target_os = "windows")
))]
pub(crate) mod agent_indicator_test_support {
    use std::sync::{LazyLock, RwLock, RwLockReadGuard, RwLockWriteGuard};

    static LOCK: LazyLock<RwLock<()>> = LazyLock::new(|| RwLock::new(()));

    pub(crate) fn read() -> RwLockReadGuard<'static, ()> {
        LOCK.read().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn write() -> RwLockWriteGuard<'static, ()> {
        LOCK.write().unwrap_or_else(|e| e.into_inner())
    }
}
