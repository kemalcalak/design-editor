//! End-to-end guard for the account-switch path that clears the editor toast
//! in the browser host.
//!
//! It drives the real function against a real `WidgetHost` rather than
//! asserting on source text.

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::JsValue;

use crate::repaint_ctx::RepaintContext;
use crate::widget_host::WidgetHost;

/// Minimal host context: everything the identity path touches is
/// `host_mut().editor_state_mut()` plus a repaint tally.
struct ToastContext {
    host: WidgetHost,
    repaints: usize,
}

impl RepaintContext for ToastContext {
    fn host(&self) -> &WidgetHost {
        &self.host
    }

    fn host_mut(&mut self) -> &mut WidgetHost {
        &mut self.host
    }

    fn viewport_size(&self) -> (f32, f32) {
        (1440.0, 900.0)
    }

    fn register_system_font(&mut self, _family: &str, _bytes: &[u8]) -> bool {
        false
    }

    fn register_imported_font(&mut self, _family: &str, _bytes: &[u8]) -> bool {
        false
    }

    fn register_imported_font_from_bytes(&mut self, _bytes: &[u8]) -> Option<String> {
        None
    }

    fn imported_family_list(&self) -> Vec<String> {
        Vec::new()
    }

    fn remove_imported_font(&mut self, _family: &str) {}

    fn repaint(&mut self) -> Result<(), JsValue> {
        self.repaints += 1;
        Ok(())
    }
}

fn context() -> Rc<RefCell<ToastContext>> {
    Rc::new(RefCell::new(ToastContext {
        host: WidgetHost::new(),
        repaints: 0,
    }))
}

fn toast_key(inner: &Rc<RefCell<ToastContext>>) -> Option<String> {
    inner
        .borrow()
        .host
        .editor_state()
        .editor_ui
        .editor_toast
        .as_ref()
        .map(|toast| toast.i18n_key.clone())
}

#[test]
fn switching_accounts_clears_a_toast_raised_for_the_previous_one() {
    // A toast describes something that happened to the previous account's
    // document. Leaving it up would show one user a sentence about another
    // user's data.
    let inner = context();
    inner
        .borrow_mut()
        .host
        .editor_state_mut()
        .editor_ui
        .show_toast(
            "menu.saveAsTemplate.saved",
            Vec::new(),
            de_editor_core::editor_toast::EditorToastLevel::Info,
            1_000,
        );
    assert!(
        toast_key(&inner).is_some(),
        "a toast is up before the switch"
    );
    {
        let mut context = inner.borrow_mut();
        context.host.editor_state_mut().editor_ui.pending_locale = Some(de_editor_core::Locale::Ja);
        context
            .host
            .editor_state_mut()
            .editor_ui
            .locale_persistence_override = Some(de_editor_core::Locale::Ja);
    }
    let repaints_before_reset = inner.borrow().repaints;

    super::live_sync_identity::reset_for_new_identity(&inner);

    assert_eq!(
        toast_key(&inner),
        None,
        "the reset must not carry one account's notice into the next"
    );
    let context = inner.borrow();
    assert_eq!(context.repaints, repaints_before_reset);
    assert_eq!(context.host.editor_state().editor_ui.pending_locale, None);
    assert_eq!(
        context
            .host
            .editor_state()
            .editor_ui
            .locale_persistence_override,
        None
    );
}
