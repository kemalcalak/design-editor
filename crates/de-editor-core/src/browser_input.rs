//! Input for a browser node in interaction mode (#324, phase 4): the
//! mouse, wheel and key events a host forwards to the node's page, in the
//! page's CSS px, plus the batch both transports carry. Plain serde types
//! and pure rules, no transport: the desktop driver turns a batch into CDP
//! calls on the node's stream tab (`browser_input_cdp`), the web editor
//! posts it to the serve-web daemon ([`BROWSER_INPUT_PATH`]).
//!
//! Key fields follow the DOM `KeyboardEvent`: `key` is the key's value
//! (`"a"`, `"Enter"`, `"ArrowLeft"`), `code` the physical key (`"KeyA"`,
//! `"Enter"`), `text` what the key types, if anything. A browser host can
//! copy them from its own event; the desktop maps winit's.

/// The daemon route a web client posts a [`BrowserInputBatch`] to.
pub const BROWSER_INPUT_PATH: &str = "/api/browser/input";
/// Most events one batch may carry; a client sends more as later batches.
pub const MAX_INPUT_EVENTS: usize = 256;
/// Most events a queue holds for a page that is not keeping up (the
/// desktop driver, the web client's outbox, the daemon's lane); past it
/// only what keeps the page's held buttons and keys right is queued
/// ([`InputBatch::extend_within`]).
pub const MAX_QUEUED_INPUT_EVENTS: usize = 4 * MAX_INPUT_EVENTS;
/// Longest text one key or insert event may carry, in chars.
pub const MAX_INPUT_TEXT_CHARS: usize = 4096;

/// Modifier keys held during an event (CDP's bit mask: Alt 1, Ctrl 2,
/// Meta / Command 4, Shift 8).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputModifiers {
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub meta: bool,
    #[serde(default)]
    pub shift: bool,
}

impl InputModifiers {
    /// CDP's `modifiers` bit mask.
    pub fn bits(self) -> u8 {
        u8::from(self.alt)
            | u8::from(self.ctrl) << 1
            | u8::from(self.meta) << 2
            | u8::from(self.shift) << 3
    }
}

/// A mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

impl MouseButton {
    /// The button's bit in a `buttons` mask (DOM order: left 1, right 2,
    /// middle 4).
    pub fn bit(self) -> u8 {
        match self {
            MouseButton::Left => 1,
            MouseButton::Right => 2,
            MouseButton::Middle => 4,
        }
    }
}

/// One event for the page, positions in CSS px of its layout viewport.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum BrowserInputEvent {
    MouseMove {
        x: f64,
        y: f64,
        /// Buttons held ([`MouseButton::bit`] mask).
        #[serde(default)]
        buttons: u8,
        #[serde(default)]
        modifiers: InputModifiers,
    },
    MouseDown {
        x: f64,
        y: f64,
        button: MouseButton,
        /// 1 for a click, 2 for the second press of a double-click.
        click_count: u32,
        /// Buttons held before this press (its own bit is added when it is
        /// sent).
        #[serde(default)]
        buttons: u8,
        #[serde(default)]
        modifiers: InputModifiers,
    },
    MouseUp {
        x: f64,
        y: f64,
        button: MouseButton,
        click_count: u32,
        /// Buttons still held after this one is released.
        #[serde(default)]
        buttons: u8,
        #[serde(default)]
        modifiers: InputModifiers,
    },
    /// Scroll by CSS px (positive y scrolls down).
    Wheel {
        x: f64,
        y: f64,
        delta_x: f64,
        delta_y: f64,
        #[serde(default)]
        modifiers: InputModifiers,
    },
    KeyDown {
        key: String,
        code: String,
        /// What the key types (`None` for a key that types nothing).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        #[serde(default)]
        modifiers: InputModifiers,
    },
    KeyUp {
        key: String,
        code: String,
        #[serde(default)]
        modifiers: InputModifiers,
    },
    /// Committed text with no key behind it (an IME composition).
    InsertText { text: String },
}

impl BrowserInputEvent {
    /// Whether the event is well-formed: finite positions and deltas, text
    /// within [`MAX_INPUT_TEXT_CHARS`], keys named.
    pub fn is_valid(&self) -> bool {
        let finite = |values: &[f64]| values.iter().all(|v| v.is_finite());
        let short = |text: &str| text.chars().count() <= MAX_INPUT_TEXT_CHARS;
        match self {
            Self::MouseMove { x, y, .. }
            | Self::MouseDown { x, y, .. }
            | Self::MouseUp { x, y, .. } => finite(&[*x, *y]),
            Self::Wheel {
                x,
                y,
                delta_x,
                delta_y,
                ..
            } => finite(&[*x, *y, *delta_x, *delta_y]),
            Self::KeyDown {
                key, code, text, ..
            } => !key.is_empty() && short(key) && short(code) && text.as_deref().is_none_or(short),
            Self::KeyUp { key, code, .. } => !key.is_empty() && short(key) && short(code),
            Self::InsertText { text } => !text.is_empty() && short(text),
        }
    }
}

impl BrowserInputEvent {
    /// What a press or release acts on: a mouse button, or a key by its
    /// physical `code` (its `key` when it has none). `None` for the rest.
    fn held_target(&self) -> Option<HeldTarget<'_>> {
        match self {
            Self::MouseDown { button, .. } | Self::MouseUp { button, .. } => {
                Some(HeldTarget::Button(*button))
            }
            Self::KeyDown { key, code, .. } | Self::KeyUp { key, code, .. } => {
                Some(HeldTarget::Key(if code.is_empty() { key } else { code }))
            }
            _ => None,
        }
    }

    /// A mouse button or key going up.
    pub fn is_release(&self) -> bool {
        matches!(self, Self::MouseUp { .. } | Self::KeyUp { .. })
    }
}

/// What a press or release acts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeldTarget<'a> {
    Button(MouseButton),
    Key(&'a str),
}

/// What of `events` must still reach the page when the rest are dropped,
/// in order: every release, and every press whose release is not among
/// them (it comes later and must find the press). A press and its release
/// go together; a repeated press or release of the same button or key is
/// kept once. So the page never keeps a button or key held that the user
/// let go of, and at most two events per button or key are kept.
pub fn keep_held_state(
    events: impl IntoIterator<Item = BrowserInputEvent>,
) -> Vec<BrowserInputEvent> {
    let mut kept: Vec<BrowserInputEvent> = Vec::new();
    for event in events {
        let Some(target) = event.held_target() else {
            continue;
        };
        // The last kept event on the same button or key.
        let last = kept.iter().rposition(|k| k.held_target() == Some(target));
        let last_is_press = last.is_some_and(|at| !kept[at].is_release());
        match (event.is_release(), last) {
            // Its press is kept too: the click is dropped whole.
            (true, Some(at)) if last_is_press => {
                kept.remove(at);
            }
            // Released already: once is enough.
            (true, Some(_)) => {}
            // Pressed already (a key repeat): once is enough.
            (false, Some(_)) if last_is_press => {}
            _ => kept.push(event),
        }
    }
    kept
}

/// Events on their way to one page, oldest first, with the redundant ones
/// folded together: a pointer move replaces a move before it (same buttons
/// and modifiers) and wheel steps at the same modifiers add up, so a busy
/// pointer or a fast trackpad costs one call per batch instead of one per
/// OS event. Presses, releases and keys are never reordered, and a
/// release is never dropped: a queue past its limit
/// ([`Self::extend_within`]) or input that did not reach the page
/// ([`Self::restore_held_state`]) keeps every release and every press
/// still waiting for one.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InputBatch {
    events: Vec<BrowserInputEvent>,
}

impl InputBatch {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Queue `event`, folding it into the last one when it only moves on.
    pub fn push(&mut self, event: BrowserInputEvent) {
        use BrowserInputEvent::{MouseMove, Wheel};
        match (self.events.last_mut(), event) {
            (
                Some(MouseMove {
                    x,
                    y,
                    buttons,
                    modifiers,
                }),
                MouseMove {
                    x: nx,
                    y: ny,
                    buttons: nb,
                    modifiers: nm,
                },
            ) if *buttons == nb && *modifiers == nm => {
                *x = nx;
                *y = ny;
            }
            (
                Some(Wheel {
                    x,
                    y,
                    delta_x,
                    delta_y,
                    modifiers,
                }),
                Wheel {
                    x: nx,
                    y: ny,
                    delta_x: ndx,
                    delta_y: ndy,
                    modifiers: nm,
                },
            ) if *modifiers == nm => {
                *x = nx;
                *y = ny;
                *delta_x += ndx;
                *delta_y += ndy;
            }
            (_, event) => self.events.push(event),
        }
    }

    /// Queue every event of `events` in order.
    pub fn extend(&mut self, events: impl IntoIterator<Item = BrowserInputEvent>) {
        for event in events {
            self.push(event);
        }
    }

    /// Queue `events` when the batch stays within `max`. Past it the page
    /// is far behind: the queued and the new events are cut down to what
    /// keeps its held buttons and keys right ([`keep_held_state`]), so a
    /// release is never dropped. Returns whether every event was queued.
    pub fn extend_within(&mut self, events: Vec<BrowserInputEvent>, max: usize) -> bool {
        if self.events.len() + events.len() <= max {
            self.extend(events);
            return true;
        }
        let queued = std::mem::take(&mut self.events);
        self.events = keep_held_state(queued.into_iter().chain(events));
        false
    }

    /// Put `events` back in front of what is queued (they were taken but
    /// did not reach the page), cut down to what keeps its held buttons
    /// and keys right.
    pub fn restore_held_state(&mut self, events: Vec<BrowserInputEvent>) {
        let mut restored = keep_held_state(events);
        if restored.is_empty() {
            return;
        }
        restored.append(&mut self.events);
        self.events = restored;
    }

    /// Take the queued events, leaving the batch empty.
    pub fn take(&mut self) -> Vec<BrowserInputEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn events(&self) -> &[BrowserInputEvent] {
        &self.events
    }
}

/// The body of `POST` [`BROWSER_INPUT_PATH`]: events for one live node of
/// a web client.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserInputBatch {
    /// The id from the client's live channel (`BrowserLiveEvent::Hello`).
    pub client: String,
    /// The node's key in the client's live set; it must be streaming.
    pub key: String,
    pub events: Vec<BrowserInputEvent>,
}

/// The answer to a [`BrowserInputBatch`].
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserInputReply {
    pub ok: bool,
    /// Events handed to the page.
    #[serde(default)]
    pub dispatched: usize,
    /// Why the batch was refused (`ok` false).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[cfg(test)]
#[path = "browser_input_tests.rs"]
mod tests;
