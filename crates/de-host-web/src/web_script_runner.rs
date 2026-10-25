//! Script nodes in the web editor (#219).
//!
//! Core decides which scripts are stale (`EditorState::take_script_jobs`,
//! a walk only when the document, theme or page moved) and lands results
//! (`land_script_output`, which drops a result its node has moved past).
//! The browser bundle has no script engine, so each job goes to the
//! daemon's `POST /api/scripts/run`, which runs it exactly as the desktop
//! does. The answer lands on a later frame: nothing waits on the network,
//! and core never hands out a second job for a node still running, so no
//! backlog builds up while a drag resizes one.

use std::cell::RefCell;
use std::rc::Rc;

use de_editor_core::script_regen::{ScriptJob, ScriptOutcome};
use de_editor_core::script_wire::{
    parse_script_run_answer, script_run_request, SCRIPT_ENGINE_UNREACHABLE,
};
use de_editor_core::{EditorState, NodeId};
use de_editor_ui::layout_scene::LayoutScene;

/// Finished runs waiting for the next frame: job, outcome, run time.
type Landed = Rc<RefCell<Vec<(ScriptJob, ScriptOutcome, u64)>>>;

#[derive(Default)]
pub(crate) struct WebScriptRunner {
    landed: Landed,
}

impl WebScriptRunner {
    /// Land finished runs, then send the jobs the document needs now.
    /// `scene` gives fill-sized scripts their size. Returns true when the
    /// document changed.
    pub(crate) fn pump(&mut self, state: &mut EditorState, scene: &LayoutScene) -> bool {
        let sizes = |id: &NodeId| de_pen_loader::scene_node_size(scene, id.as_str());
        let mut changed = false;
        let landed: Vec<_> = self.landed.borrow_mut().drain(..).collect();
        for (job, outcome, elapsed_ms) in landed {
            changed |=
                state.land_script_output(&job.node_id, &job.hash, outcome, elapsed_ms, &sizes);
        }
        for job in state.take_script_jobs(&sizes) {
            self.send(job);
        }
        changed
    }

    fn send(&self, job: ScriptJob) {
        let url = format!("{}/api/scripts/run", crate::daemon_base::daemon_base());
        let body = script_run_request(&job);
        let landed = Rc::clone(&self.landed);
        let unsent = job.clone();
        let on_response = Rc::new(move |status: u16, text: String| {
            let (outcome, elapsed_ms) = parse_script_run_answer(status, &text);
            landed.borrow_mut().push((job.clone(), outcome, elapsed_ms));
            crate::repaint_coalescer::request();
        });
        // A request that could not start never calls back: fail it here so
        // the node does not stay "running".
        if !crate::live_sync::post_json_with_status(&url, &body, on_response) {
            let failed = ScriptOutcome::Failed(SCRIPT_ENGINE_UNREACHABLE.to_string());
            self.landed.borrow_mut().push((unsent, failed, 0));
        }
    }
}
