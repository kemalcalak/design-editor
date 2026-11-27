//! What an `execute` script does, for the "ask before destructive agent
//! actions" gate (#398).
//!
//! A script is judged by running it ([`execute_effects`]), since no reading
//! of its text can tell (`globalThis["Del" + "ete"](…)`). The gate's dry run
//! and the real run must then agree, so every run is deterministic in its
//! inputs: `Math.random` is seeded and `Date` frozen from one
//! [`ExecuteClock`]. The gate pins the dry run's clock and what it found on
//! the thread that then runs the call ([`pinned`]); the real run refuses,
//! applying nothing, when it would do a destructive call the user was not
//! asked about (the document changed while the prompt was open).

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use de_editor_core::shared_state::SharedState;
use serde_json::Value;

use super::script_execute::{eval, EXECUTE_BUDGET};
use super::script_execute_session::ExecSession;
use super::script_runner::{strip_fences, MAX_SCRIPT_BYTES};

/// The destructive calls one `execute` run makes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExecuteEffects {
    pub deletes: usize,
    pub replaces: usize,
    /// `Export` calls (they write files).
    pub exports: usize,
    /// A `SetVariables(…, true)`: every variable not listed is removed.
    pub replaces_variables: bool,
    /// Each destructive call with its arguments, in call order: what
    /// [`ExecuteEffects::permits`] compares.
    ops: Vec<String>,
}

impl ExecuteEffects {
    /// Count one host call (`op` is the prelude's `call` name).
    pub(crate) fn note(&mut self, op: &str, args: &Value) {
        let destructive = match op {
            "delete" => {
                self.deletes += 1;
                true
            }
            "replace" => {
                self.replaces += 1;
                true
            }
            "export" => {
                self.exports += 1;
                true
            }
            "setVariables" => {
                let replaces = args.get("replace").and_then(Value::as_bool) == Some(true);
                self.replaces_variables |= replaces;
                replaces
            }
            _ => false,
        };
        if destructive {
            self.ops.push(format!("{op} {args}"));
        }
    }

    /// Whether any of it deletes, replaces or writes a file.
    pub fn is_destructive(&self) -> bool {
        !self.ops.is_empty()
    }

    /// Whether every destructive call of `run` is one of these (the same
    /// call with the same arguments, as many times at most).
    pub fn permits(&self, run: &ExecuteEffects) -> bool {
        let mut left: Vec<&str> = self.ops.iter().map(String::as_str).collect();
        run.ops
            .iter()
            .all(|op| match left.iter().position(|o| o == op) {
                Some(at) => {
                    left.swap_remove(at);
                    true
                }
                None => false,
            })
    }
}

/// The time a run sees: `Date` is frozen at it and `Math.random` seeded
/// from it, so two runs on one clock and one document do the same.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecuteClock {
    /// Milliseconds since the Unix epoch.
    pub now_ms: u64,
}

impl ExecuteClock {
    /// The current time.
    pub fn now() -> Self {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        Self { now_ms }
    }

    fn seed(self) -> u32 {
        (self.now_ms.wrapping_mul(0x9e37_79b9_7f4a_7c15) >> 32) as u32
    }

    /// The prelude that seeds `Math.random` (mulberry32) and freezes
    /// `Date` (and `performance.now`) for one run.
    pub(crate) fn prelude(self) -> String {
        format!(
            r#"(function (now, seed) {{
  Math.random = function () {{
    seed = (seed + 0x6d2b79f5) | 0;
    var t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  }};
  var RealDate = Date;
  function FrozenDate() {{
    if (!new.target) return new RealDate(now).toString();
    if (arguments.length === 0) return new RealDate(now);
    return Reflect.construct(RealDate, Array.prototype.slice.call(arguments), new.target);
  }}
  FrozenDate.prototype = RealDate.prototype;
  FrozenDate.now = function () {{ return now; }};
  FrozenDate.parse = RealDate.parse;
  FrozenDate.UTC = RealDate.UTC;
  globalThis.Date = FrozenDate;
  if (typeof performance === "object" && performance) {{
    try {{ performance.now = function () {{ return 0; }}; }} catch (e) {{}}
  }}
}})({now}, {seed});"#,
            now = self.now_ms,
            seed = self.seed() as i32,
        )
    }
}

/// What the real run of a gated call is held to on this thread.
#[derive(Debug, Clone)]
pub(crate) struct Pin {
    pub(crate) clock: ExecuteClock,
    /// `None`: the user allowed a script the dry run could not judge, so
    /// anything it does was asked about.
    pub(crate) allowed: Option<ExecuteEffects>,
}

thread_local! {
    static PIN: RefCell<Option<Pin>> = const { RefCell::new(None) };
}

/// Run `f` with every `execute` run on this thread on `clock` and held to
/// `allowed` (`None` = anything): a run that would make a destructive call
/// outside it fails and applies nothing.
pub fn pinned<R>(clock: ExecuteClock, allowed: Option<ExecuteEffects>, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<Pin>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let previous = self.0.take();
            PIN.with(|pin| *pin.borrow_mut() = previous);
        }
    }
    let previous = PIN.with(|pin| pin.borrow_mut().replace(Pin { clock, allowed }));
    let _restore = Restore(previous);
    f()
}

/// The pin of this thread, or the current time and no limit.
pub(crate) fn current_pin() -> Pin {
    PIN.with(|pin| pin.borrow().clone()).unwrap_or_else(|| Pin {
        clock: ExecuteClock::now(),
        allowed: None,
    })
}

/// Run `script` against `snapshot` on `clock` without applying, rendering
/// or writing anything, and say which destructive calls it made. Exports
/// are recorded, never rendered. `None` when the script could not be judged
/// (it threw, a write failed, it ran out of time, or the source is empty or
/// too large).
pub fn execute_effects(
    snapshot: SharedState,
    script: &str,
    page_id: Option<String>,
    clock: ExecuteClock,
) -> Option<ExecuteEffects> {
    let script = strip_fences(script);
    if script.trim().is_empty() || script.len() > MAX_SCRIPT_BYTES {
        return None;
    }
    let session = Rc::new(RefCell::new(ExecSession::new(snapshot, page_id, true)));
    eval(&script, session.clone(), EXECUTE_BUDGET, clock).ok()?;
    let session = session.borrow();
    session
        .failed_write
        .is_none()
        .then(|| session.effects.clone())
}
