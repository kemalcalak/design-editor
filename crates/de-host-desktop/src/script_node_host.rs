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
use de_mcp::script_node::{run_script_job, ScriptNodeError, ScriptNodeRun};
use winit::event_loop::EventLoopProxy;

use crate::DesktopEvent;

type WakeProxy = Arc<Mutex<Option<EventLoopProxy<DesktopEvent>>>>;

/// A job tagged with the document epoch it was taken from.
type EpochJob = (u64, ScriptJob);
type EpochRun = (u64, ScriptJob, ScriptNodeRun);

pub(crate) struct ScriptNodeHost {
    /// The host's document epoch core's run state belongs to.
    epoch: u64,
    /// `None` until a job needs the worker, and again after the worker
    /// stopped; the next job then spawns a fresh one.
    job_tx: Option<Sender<EpochJob>>,
    result_tx: Sender<EpochRun>,
    result_rx: Receiver<EpochRun>,
    wake_proxy: WakeProxy,
}

impl Default for ScriptNodeHost {
    fn default() -> Self {
        let (result_tx, result_rx) = channel();
        Self {
            epoch: 0,
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

    /// Land finished runs. `scene` is the canvas layout fill-sized
    /// scripts take their size from, as it was when the jobs were taken.
    /// Returns true when the document changed.
    pub(crate) fn land(
        &mut self,
        state: &mut EditorState,
        scene: &LayoutScene,
        epoch: u64,
    ) -> bool {
        self.adopt_epoch(state, epoch);
        let sizes = |id: &NodeId| de_pen_loader::scene_node_size(scene, id.as_str());
        let mut changed = false;
        while let Ok((run_epoch, job, run)) = self.result_rx.try_recv() {
            // A run for a replaced document names nodes this one may share
            // by id only.
            if run_epoch != epoch {
                continue;
            }
            let elapsed_ms = run.elapsed.as_millis() as u64;
            changed |= state.land_script_output(
                &job.node_id,
                &job.hash,
                run.outcome(),
                elapsed_ms,
                &sizes,
            );
        }
        changed
    }

    /// Send out the jobs the document needs now. Call with the scene
    /// rebuilt after [`Self::land`] changed the document.
    pub(crate) fn send_stale(&mut self, state: &mut EditorState, scene: &LayoutScene, epoch: u64) {
        self.adopt_epoch(state, epoch);
        let sizes = |id: &NodeId| de_pen_loader::scene_node_size(scene, id.as_str());
        for job in state.take_script_jobs(&sizes) {
            if let Err(job) = self.send(job) {
                // No worker would take it: record the failure so the node
                // does not stay "running" and is not retried every frame.
                let failed = ScriptOutcome::Failed("script worker unavailable".into());
                state.land_script_output(&job.node_id, &job.hash, failed, 0, &sizes);
            }
        }
    }

    /// A new document: its predecessor's runs are dropped as they finish.
    fn adopt_epoch(&mut self, state: &mut EditorState, epoch: u64) {
        if self.epoch != epoch {
            self.epoch = epoch;
            state.reset_script_runs();
        }
    }

    /// Hand `job` to the worker, spawning a fresh one once if the current
    /// one is gone. Gives the job back when no worker takes it.
    fn send(&mut self, mut job: ScriptJob) -> Result<(), ScriptJob> {
        let epoch = self.epoch;
        for _ in 0..2 {
            let Some(tx) = self.worker() else {
                return Err(job);
            };
            match tx.send((epoch, job)) {
                Ok(()) => return Ok(()),
                Err(SendError((_, back))) => {
                    job = back;
                    self.job_tx = None;
                }
            }
        }
        Err(job)
    }

    fn worker(&mut self) -> Option<&Sender<EpochJob>> {
        if self.job_tx.is_none() {
            let (job_tx, job_rx) = channel::<EpochJob>();
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

fn worker_loop(jobs: Receiver<EpochJob>, results: Sender<EpochRun>, wake: WakeProxy) {
    while let Ok((epoch, job)) = jobs.recv() {
        // A panic in the engine fails this run instead of the worker.
        let run = catch_unwind(AssertUnwindSafe(|| run_script_job(&job))).unwrap_or_else(|_| {
            ScriptNodeRun {
                result: Err(ScriptNodeError::Sandbox("script engine panicked".into())),
                elapsed: Duration::ZERO,
            }
        });
        if results.send((epoch, job, run)).is_err() {
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
