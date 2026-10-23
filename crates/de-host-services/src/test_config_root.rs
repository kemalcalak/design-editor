//! Keeps this crate's unit tests off the user's real `~/.design-editor`.
//! Mirrors de-host-desktop's `test_config_root.rs`: the guard sits at the
//! entry points that resolve the user root, so the redirect does not depend
//! on which test happens to run first.

/// Point the config store at a scratch directory the first time a user-root
/// entry point runs inside this crate's TEST binary. Idempotent.
#[cfg(test)]
pub(crate) fn guard_user_config() {
    let root = std::env::temp_dir().join(format!(
        "de-host-services-test-config-{}",
        std::process::id()
    ));
    de_config_store::redirect_user_root_for_tests(root);
}

/// Real builds resolve the real `~/.design-editor`; compiles away.
#[cfg(not(test))]
#[inline]
pub(crate) fn guard_user_config() {}
