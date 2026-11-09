//! Mouse and keyboard input for a streamed tab (#324, phase 4): the user
//! drives a live canvas browser node's page. A [`PageInput`] is a DevTools
//! connection of its own, attached to the stream's tab
//! ([`crate::PageStream::input`]), so the page that gets the input is
//! exactly the one the canvas shows and the stream's own connection keeps
//! reading frames meanwhile. Only `Input.*` methods pass; each call blocks
//! until Chrome handled the event (about a millisecond locally), so
//! callers send from a worker thread. A JavaScript dialog the input opens
//! (`alert`, `confirm`, `prompt`, `beforeunload`) would hold that call
//! until it closes, and a headless tab shows no dialog to close, so it is
//! accepted at once. Dropping it detaches from the tab (the tab stays
//! open).

use std::time::Instant;

use serde_json::{json, Value};

use crate::cdp_client::{Awaited, CdpClient};
use crate::tab::{Tab, COMMAND_TIMEOUT};
use crate::BrowserRenderError;

/// The only domain a [`PageInput`] may call.
const INPUT_DOMAIN: &str = "Input";
/// The event a page dialog opens with.
const DIALOG_OPENING: &str = "Page.javascriptDialogOpening";

pub struct PageInput {
    client: CdpClient,
    /// `None` only while dropping.
    tab: Option<Tab>,
}

impl PageInput {
    /// Attach to the tab `target_id` of the browser at `ws_url`.
    pub(crate) fn connect(ws_url: &str, target_id: &str) -> Result<Self, BrowserRenderError> {
        let mut client = CdpClient::connect(ws_url)?;
        let tab = Tab::attach(&mut client, target_id)?;
        Ok(Self {
            client,
            tab: Some(tab),
        })
    }

    /// Send one `Input.*` call and wait for Chrome to handle it. Any other
    /// domain is refused ([`BrowserRenderError::MethodRefused`]).
    pub fn dispatch(&mut self, method: &str, params: Value) -> Result<(), BrowserRenderError> {
        match method.split_once('.') {
            Some((INPUT_DOMAIN, name)) if !name.is_empty() => {}
            _ => {
                return Err(BrowserRenderError::MethodRefused {
                    method: method.to_string(),
                    allowed: INPUT_DOMAIN.to_string(),
                })
            }
        }
        let tab = self.tab.as_ref().ok_or(BrowserRenderError::TabClosed)?;
        let session = tab.session().to_string();
        let id = self.client.send(Some(&session), method, params)?;
        let deadline = Instant::now() + COMMAND_TIMEOUT;
        loop {
            let watch = Some((session.as_str(), DIALOG_OPENING));
            match self.client.await_reply(id, method, deadline, watch)? {
                Awaited::Reply(_) => return Ok(()),
                Awaited::Event => {
                    // The reply to `id` arrives once the dialog closed and
                    // is kept for the next wait.
                    self.client.call(
                        Some(&session),
                        "Page.handleJavaScriptDialog",
                        json!({ "accept": true }),
                        COMMAND_TIMEOUT,
                    )?;
                }
            }
        }
    }

    /// Any call on the tab, for tests that check what the input did.
    #[cfg(test)]
    pub(crate) fn tab_command(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<Value, BrowserRenderError> {
        let tab = self.tab.as_ref().ok_or(BrowserRenderError::TabClosed)?;
        tab.command(&mut self.client, method, params)
    }
}

impl Drop for PageInput {
    fn drop(&mut self) {
        if let Some(tab) = self.tab.take() {
            tab.detach(&mut self.client);
        }
    }
}

#[cfg(test)]
#[path = "page_input_tests.rs"]
mod tests;
