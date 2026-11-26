//! `OsClipboard` over a fake backend: the handle recovers after a failure
//! that may have broken it, and a node Cmd+C → Cmd+V round trip through
//! platform-wrapped HTML pastes the nodes (#436).

use super::{ClipboardWrite, OsClipboard, SystemClipboard};
use crate::clipboard_backend::ClipboardBackend;
use crate::DesktopApp;
use arboard::{Error, ImageData};
use de_editor_core::NodeId;
use jian_ops_schema::node::PenNode;
use std::cell::RefCell;
use std::rc::Rc;

/// The shared "OS clipboard" every fake handle reads and writes.
#[derive(Default)]
struct FakeOs {
    text: Option<String>,
    html: Option<String>,
    /// How the platform hands written HTML back.
    wrap: Option<fn(&str) -> String>,
    /// Handles opened so far; each handle remembers its number.
    opens: usize,
    /// The handle whose connection has died.
    broken_handle: Option<usize>,
}

struct FakeHandle {
    os: Rc<RefCell<FakeOs>>,
    number: usize,
}

impl FakeHandle {
    fn check(&self) -> Result<(), Error> {
        if self.os.borrow().broken_handle == Some(self.number) {
            return Err(Error::Unknown {
                description: "connection lost".to_string(),
            });
        }
        Ok(())
    }
}

impl ClipboardBackend for FakeHandle {
    fn get_text(&mut self) -> Result<String, Error> {
        self.check()?;
        self.os
            .borrow()
            .text
            .clone()
            .ok_or(Error::ContentNotAvailable)
    }

    fn get_html(&mut self) -> Result<String, Error> {
        self.check()?;
        self.os
            .borrow()
            .html
            .clone()
            .ok_or(Error::ContentNotAvailable)
    }

    fn get_image(&mut self) -> Result<ImageData<'static>, Error> {
        self.check()?;
        Err(Error::ContentNotAvailable)
    }

    fn set_text(&mut self, text: &str) -> Result<(), Error> {
        self.check()?;
        let mut os = self.os.borrow_mut();
        os.text = Some(text.to_string());
        os.html = None;
        Ok(())
    }

    fn set_image(&mut self, _image: ImageData<'static>) -> Result<(), Error> {
        self.check()?;
        let mut os = self.os.borrow_mut();
        os.text = None;
        os.html = None;
        Ok(())
    }

    fn set_private_html(&mut self, html: &str) -> Result<(), Error> {
        self.check()?;
        let mut os = self.os.borrow_mut();
        os.html = Some(os.wrap.map_or_else(|| html.to_string(), |wrap| wrap(html)));
        os.text = None;
        Ok(())
    }
}

fn fake_clipboard(os: &Rc<RefCell<FakeOs>>) -> OsClipboard<FakeHandle> {
    let os = Rc::clone(os);
    OsClipboard::with_opener(move || {
        let number = {
            let mut state = os.borrow_mut();
            state.opens += 1;
            state.opens
        };
        Some(FakeHandle {
            os: Rc::clone(&os),
            number,
        })
    })
}

fn macos_html(html: &str) -> String {
    format!(
        r#"<html><head><meta http-equiv="content-type" content="text/html; charset=utf-8"></head><body>{html}</body></html>"#
    )
}

fn windows_cf_html(html: &str) -> String {
    format!(
        "Version:0.9\r\nStartHTML:0000000105\r\nEndHTML:0000000221\r\nStartFragment:0000000141\r\nEndFragment:0000000185\r\n<html><body>\r\n<!--StartFragment-->{html}<!--EndFragment-->\r\n</body></html>"
    )
}

#[test]
fn the_handle_is_opened_once_and_kept() {
    let os = Rc::new(RefCell::new(FakeOs::default()));
    let mut clipboard = fake_clipboard(&os);

    clipboard.write(ClipboardWrite::Text("a".to_string()));
    assert_eq!(clipboard.read_text().as_deref(), Some("a"));
    // An empty flavour is no failure of the handle.
    assert_eq!(clipboard.read_canvas(), (None, None));

    assert_eq!(os.borrow().opens, 1);
}

#[test]
fn a_broken_handle_is_dropped_and_the_next_read_reopens() {
    let os = Rc::new(RefCell::new(FakeOs::default()));
    let mut clipboard = fake_clipboard(&os);
    clipboard.write(ClipboardWrite::Text("kept".to_string()));
    os.borrow_mut().broken_handle = Some(1);

    assert_eq!(clipboard.read_text(), None);
    assert_eq!(clipboard.read_text().as_deref(), Some("kept"));

    assert_eq!(os.borrow().opens, 2);
}

#[test]
fn a_broken_handle_is_dropped_after_a_failed_write() {
    let os = Rc::new(RefCell::new(FakeOs::default()));
    let mut clipboard = fake_clipboard(&os);
    clipboard.write(ClipboardWrite::Text("old".to_string()));
    os.borrow_mut().broken_handle = Some(1);

    clipboard.write(ClipboardWrite::NodeMarker);
    assert_eq!(os.borrow().text.as_deref(), Some("old"));
    clipboard.write(ClipboardWrite::NodeMarker);

    assert_eq!(os.borrow().opens, 2);
    assert!(os.borrow().html.is_some());
    assert_eq!(os.borrow().text, None);
}

/// Cmd+C on the starter node, Cmd+V, through the real shortcut and paste
/// paths: the written marker comes back wrapped as the platform wraps it.
fn node_copy_paste_round_trip(wrap: fn(&str) -> String) {
    let os = Rc::new(RefCell::new(FakeOs {
        // A stale Copy as SVG sits on the clipboard.
        text: Some(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><path d="M0 0 L9 9"/></svg>"#.to_string(),
        ),
        wrap: Some(wrap),
        ..FakeOs::default()
    }));
    let mut app = DesktopApp::new(None);
    app.clipboard = Box::new(fake_clipboard(&os));
    app.host
        .editor_state_mut()
        .set_single_selection(NodeId::new("n10"));

    assert!(app.handle_cmd_copy());
    let html = os
        .borrow()
        .html
        .clone()
        .expect("the marker is on the clipboard");
    assert_ne!(
        html,
        de_editor_core::node_clipboard_marker::NODE_CLIPBOARD_MARKER_HTML
    );
    app.host.editor_state_mut().clear_selection();

    assert!(app.handle_cmd_paste());

    let state = app.host.editor_state();
    assert_eq!(state.active_children().len(), 2);
    assert_ne!(state.selection.anchor, NodeId::new("n10"));
    let pasted = state.selected_node().expect("the pasted node is selected");
    assert!(!matches!(pasted, PenNode::Group(_) | PenNode::Image(_)));
    assert!(app.pending_html_paste.is_none());
    assert!(app.pending_figma_paste.is_none());
}

#[test]
fn node_copy_then_paste_pastes_the_nodes_through_macos_html() {
    node_copy_paste_round_trip(macos_html);
}

#[test]
fn node_copy_then_paste_pastes_the_nodes_through_windows_cf_html() {
    node_copy_paste_round_trip(windows_cf_html);
}
