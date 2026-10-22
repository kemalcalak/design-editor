//! Guards for the live-sync latches that need no live shell: the refused
//! credential latch and the undoable-apply rule behind
//! `replace_document_from_sync`.
//!
//! Split out of `live_sync_glue.rs` at the 800-line cap.

mod auth_latch_tests {
    use crate::live_sync_glue::{auth_is_invalid, clear_auth_invalid, note_auth_invalid};

    #[test]
    fn a_refused_credential_stops_the_tab_pushing() {
        clear_auth_invalid();
        assert!(!auth_is_invalid(), "a healthy tab pushes");

        // The document on screen belongs to whoever WAS signed in; pushing it
        // after a switch would write one account's work into another's tenant.
        note_auth_invalid();
        assert!(auth_is_invalid());

        // Only the identity reset lifts it, once the tab has been rebuilt.
        clear_auth_invalid();
        assert!(!auth_is_invalid());
    }
}

mod undoable_apply_tests {
    use de_editor_core::web_sync::WebSyncClient;

    #[test]
    fn every_apply_after_the_first_is_undoable() {
        // `apply_document_response` derives its `undoable` flag from
        // `WebSyncClient::initialized`, and hands it to
        // `replace_document_from_sync`, which picks
        // `replace_document_with_undo` when it is set.
        //
        // So an external apply after the mount pull (an AI turn or an MCP
        // client write) is one undo step, while the mount pull is not.
        let mut client = WebSyncClient::new();
        assert!(
            !client.initialized(),
            "the mount pull must NOT be an undo step: it is starter -> daemon, \
             not a user edit being overwritten"
        );

        client.mark_applied(1);
        assert!(client.initialized());

        // Never reset by later applies, so an apply at any later version is
        // still undoable.
        for version in 2..10 {
            client.mark_applied(version);
            assert!(client.initialized(), "version {version}");
        }
    }
}
