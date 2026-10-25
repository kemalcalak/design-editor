//! Script-node runs off the UI thread (#19).
//!
//! Core decides which scripts are stale (`EditorState::take_script_jobs`,
//! a walk only when the document, theme or page moved) and lands results
//! (`land_script_output`, which drops a result its node has moved past).
//! This host runs the programs on one worker thread, so a slow or runaway
//! script (stopped at `de_mcp::script_node::TIME_LIMIT`) never blocks a
//! frame. Core never hands out a second job for a node whose job is
//! running, so a resize drag updates as fast as the script runs and no
//! queue of stale runs builds up behind it.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc::{channel, Receiver, SendError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use de_editor_core::script_regen::{ScriptJob, ScriptOutcome};
use de_editor_core::{EditorState, NodeId};
use de_editor_ui::layout_scene::LayoutScene;
use de_mcp::script_node::{run_script_job, scene_size, ScriptNodeError, ScriptNodeRun};
use winit::event_loop::EventLoopProxy;

use crate::DesktopEvent;

type WakeProxy = Arc<Mutex<Option<EventLoopProxy<DesktopEvent>>>>;

pub(crate) struct ScriptNodeHost {
    /// `None` until a job needs the worker, and again after the worker
    /// stopped; the next job then spawns a fresh one.
    job_tx: Option<Sender<ScriptJob>>,
    result_tx: Sender<(ScriptJob, ScriptNodeRun)>,
    result_rx: Receiver<(ScriptJob, ScriptNodeRun)>,
    wake_proxy: WakeProxy,
}

impl Default for ScriptNodeHost {
    fn default() -> Self {
        let (result_tx, result_rx) = channel();
        Self {
            job_tx: None,
            result_tx,
            result_rx,
            wake_proxy: Arc::new(Mutex::new(None)),
        }
    }
}

impl ScriptNodeHost {
    pub(crate) fn set_wake_proxy(&mut self, proxy: EventLoopProxy<DesktopEvent>) {
        if let Ok(mut wake) = self.wake_proxy.lock() {
            *wake = Some(proxy);
        }
    }

    /// Land finished runs, then send out the jobs the document needs now.
    /// `scene` is the canvas layout fill-sized scripts take their size
    /// from. Returns true when the document changed.
    pub(crate) fn pump(&mut self, state: &mut EditorState, scene: &LayoutScene) -> bool {
        let sizes = |id: &NodeId| scene_size(scene, id);
        let mut changed = false;
        while let Ok((job, run)) = self.result_rx.try_recv() {
            let elapsed_ms = run.elapsed.as_millis() as u64;
            changed |= state.land_script_output(
                &job.node_id,
                &job.hash,
                run.outcome(),
                elapsed_ms,
                &sizes,
            );
        }
        for job in state.take_script_jobs(&sizes) {
            if let Err(job) = self.send(job) {
                // No worker would take it: record the failure so the node
                // does not stay "running" and is not retried every frame.
                let failed = ScriptOutcome::Failed("script worker unavailable".into());
                state.land_script_output(&job.node_id, &job.hash, failed, 0, &sizes);
            }
        }
        changed
    }

    /// Hand `job` to the worker, spawning a fresh one once if the current
    /// one is gone. Gives the job back when no worker takes it.
    fn send(&mut self, mut job: ScriptJob) -> Result<(), ScriptJob> {
        for _ in 0..2 {
            let Some(tx) = self.worker() else {
                return Err(job);
            };
            match tx.send(job) {
                Ok(()) => return Ok(()),
                Err(SendError(back)) => {
                    job = back;
                    self.job_tx = None;
                }
            }
        }
        Err(job)
    }

    fn worker(&mut self) -> Option<&Sender<ScriptJob>> {
        if self.job_tx.is_none() {
            let (job_tx, job_rx) = channel::<ScriptJob>();
            let results = self.result_tx.clone();
            let wake = Arc::clone(&self.wake_proxy);
            let spawned = std::thread::Builder::new()
                .name("de-script-node".into())
                .spawn(move || worker_loop(job_rx, results, wake));
            match spawned {
                Ok(_) => self.job_tx = Some(job_tx),
                Err(err) => eprintln!("design-editor: spawn script worker failed: {err}"),
            }
        }
        self.job_tx.as_ref()
    }
}

fn worker_loop(
    jobs: Receiver<ScriptJob>,
    results: Sender<(ScriptJob, ScriptNodeRun)>,
    wake: WakeProxy,
) {
    while let Ok(job) = jobs.recv() {
        // A panic in the engine fails this run instead of the worker.
        let run = catch_unwind(AssertUnwindSafe(|| run_script_job(&job))).unwrap_or_else(|_| {
            ScriptNodeRun {
                result: Err(ScriptNodeError::Sandbox("script engine panicked".into())),
                elapsed: Duration::ZERO,
            }
        });
        if results.send((job, run)).is_err() {
            break;
        }
        if let Some(proxy) = wake.lock().ok().and_then(|wake| wake.clone()) {
            let _ = proxy.send_event(DesktopEvent::ScriptNodesReady);
        }
    }
}

#[cfg(test)]
#[path = "script_node_host_tests.rs"]
mod tests;
