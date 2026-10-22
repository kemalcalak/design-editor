//! Shared GL + Skia context module (spec v19 §3).
//!
//! - [`provider`] — `GlContextProvider` trait (cross-platform) +
//!   `GlutinProvider` (desktop). The trait is exposed on every non-wasm
//!   target.
//! - [`shared`] — `SharedSkiaContext` owning the GL stack +
//!   `skia_safe::DirectContext` + `skia_safe::Surface`. Frame-scoped
//!   `with_frame` callback + idempotent teardown. Needs `gl-host`.

pub mod provider;

// Cross-platform: trait + error types, importable on every non-wasm target.
pub use provider::{GlContextProvider, ProviderError, ProviderResult};

// Desktop provider implementation.
#[cfg(feature = "gl-host")]
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
pub use provider::GlutinProvider;

// `SharedSkiaContext` depends on `skia_safe` + `winit` (cfg-gated
// dependencies).
#[cfg(feature = "gl-host")]
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
mod shared;
#[cfg(feature = "gl-host")]
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
pub use shared::{SharedSkiaContext, SharedSkiaError, SharedSkiaResult, SurfaceConfig};
