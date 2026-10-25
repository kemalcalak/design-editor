//! Script nodes in the web editor (#219).
//!
//! Core decides which scripts are stale (`EditorState::take_script_jobs`,
//! a walk only when the document, theme or page moved) and lands results
//! (`land_script_output`, which drops a result its node has moved past).
//! The browser bundle has no script engine, so each job goes to the
//! daemon's `POST /api/scripts/run`, which runs it exactly as the desktop
//! does. The answer lands on a later frame: nothing waits on the network,
//! and core never hands out a second job for a node still running, so no
//! backlog builds up while a drag resizes one. At most
//! [`MAX_IN_FLIGHT`] requests are out at once; the rest wait here.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use de_editor_core::script_regen::{ScriptJob, ScriptOutcome};
use de_editor_core::script_wire::{parse_script_run_answer, script_run_request};
use de_editor_core::{EditorState, NodeId};
use de_editor_ui::layout_scene::LayoutScene;

/// Requests out at once: a page of script nodes must not take every
/// connection the browser gives the daemon.
const MAX_IN_FLIGHT: usize = 3;

/// A finished run waiting for the next frame, tagged with the document
/// epoch it was sent for.
struct Landing {
    epoch: u64,
    job: ScriptJob,
    outcome: ScriptOutcome,
    elapsed_ms: u64,
}

#[derive(Default)]
pub(crate) struct WebScriptRunner {
    /// The host's document epoch the queue and core's run state belong to.
    epoch: u64,
    landed: Rc<RefCell<Vec<Landing>>>,
    queued: VecDeque<ScriptJob>,
    in_flight: Rc<Cell<usize>>,
}

impl WebScriptRunner {
    /// Land finished runs. `scene` gives fill-sized scripts their size, as
    /// it did when the jobs were taken. Returns true when the document
    /// changed.
    pub(crate) fn land(
        &mut self,
        state: &mut EditorState,
        scene: &LayoutScene,
        epoch: u64,
    ) -> bool {
        self.adopt_epoch(state, epoch);
        let sizes = |id: &NodeId| de_pen_loader::scene_node_size(scene, id.as_str());
        let landed: Vec<_> = self.landed.borrow_mut().drain(..).collect();
        let mut changed = false;
        for landing in landed {
            // An answer for a replaced document names nodes this one may
            // share by id only.
            if landing.epoch != epoch {
                continue;
            }
            let Landing {
                job,
                outcome,
                elapsed_ms,
                ..
            } = landing;
            changed |=
                state.land_script_output(&job.node_id, &job.hash, outcome, elapsed_ms, &sizes);
        }
        changed
    }

    /// Queue the jobs the document needs now and send as many as the
    /// in-flight cap allows. Call with the scene rebuilt after [`Self::land`]
    /// changed the document.
    pub(crate) fn send_stale(&mut self, state: &mut EditorState, scene: &LayoutScene, epoch: u64) {
        self.adopt_epoch(state, epoch);
        let sizes = |id: &NodeId| de_pen_loader::scene_node_size(scene, id.as_str());
        self.queued.extend(state.take_script_jobs(&sizes));
        while self.in_flight.get() < MAX_IN_FLIGHT {
            let Some(job) = self.queued.pop_front() else {
                break;
            };
            self.send(job);
        }
    }

    /// A new document: drop its predecessor's queue and run state.
    fn adopt_epoch(&mut self, state: &mut EditorState, epoch: u64) {
        if self.epoch != epoch {
            self.epoch = epoch;
            self.queued.clear();
            state.reset_script_runs();
        }
    }

    fn send(&self, job: ScriptJob) {
        let url = crate::daemon_base::daemon_url("/api/scripts/run");
        let body = script_run_request(&job);
        let epoch = self.epoch;
        let landed = Rc::clone(&self.landed);
        let in_flight = Rc::clone(&self.in_flight);
        let unsent = job.clone();
        self.in_flight.set(self.in_flight.get() + 1);
        let on_response = Rc::new(move |status: u16, text: String| {
            let (outcome, elapsed_ms) = parse_script_run_answer(status, &text);
            in_flight.set(in_flight.get().saturating_sub(1));
            landed.borrow_mut().push(Landing {
                epoch,
                job: job.clone(),
                outcome,
                elapsed_ms,
            });
            crate::repaint_coalescer::request();
        });
        // A request that could not start never calls back: settle it here
        // so the node does not stay "running", and wake a frame to land it.
        if !crate::live_sync::post_json_with_status(&url, &body, on_response) {
            self.in_flight.set(self.in_flight.get().saturating_sub(1));
            self.landed.borrow_mut().push(Landing {
                epoch,
                job: unsent,
                outcome: ScriptOutcome::Unreachable,
                elapsed_ms: 0,
            });
            crate::repaint_coalescer::request();
        }
    }
}
