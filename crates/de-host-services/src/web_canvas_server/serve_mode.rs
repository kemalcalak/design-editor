//! How the web-canvas daemon was launched.
//!
//! Both modes serve one operator's document: `Local` is the plain
//! `--serve-web <port>` daemon, `Managed` is the same daemon spawned by a
//! supervisor that holds its stdin lease. They differ only in the browser
//! origin boundary and the lifecycle contract (see `run_loop.rs`), never in
//! which routes exist.

/// How this daemon instance is deployed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ServeMode {
    /// `--serve-web <port>`: one local operator, loopback by default.
    #[default]
    Local,
    /// `--serve-web --managed`: spawned by a supervisor (the VS Code
    /// extension) that holds a per-instance token. Still one operator.
    Managed,
}

impl ServeMode {
    /// Stable name for the wire, reported as `serveMode` by
    /// `GET /api/mcp/server`.
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Managed => "managed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_mode_is_the_local_daemon() {
        assert_eq!(ServeMode::default(), ServeMode::Local);
    }

    #[test]
    fn wire_names_are_stable() {
        assert_eq!(ServeMode::Local.wire_name(), "local");
        assert_eq!(ServeMode::Managed.wire_name(), "managed");
    }
}
