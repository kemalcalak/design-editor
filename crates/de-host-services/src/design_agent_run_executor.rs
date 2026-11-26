//! The headless [`ChatToolExecutor`] behind a design-agent run: it owns a
//! clone of the document behind a mutex and applies every tool call through
//! the same seam the desktop uses
//! (`execute_agent_tool_with_root_seed_guard` → `state.apply(EditorCommand)`).
//! It started as de-smoke's loop-mode executor.

use std::sync::{Arc, Mutex};

use de_ai::chat_provider::{
    ChatToolExecutor, ChatToolResult, FinalizeReport, UnfilledScreensReport,
};
use de_editor_core::write_scope::WriteBaseline;
use de_editor_core::EditorState;
use de_orchestrator::agent_run_finalize::{agent_run_screens, finalize_agent_run};

use crate::design_agent_tools::{execute_agent_tool_with_root_seed_guard, RootSeedGuard};

pub(crate) struct HeadlessDesignExecutor {
    state: Arc<Mutex<EditorState>>,
    root_seed_guard: Mutex<RootSeedGuard>,
    finalize_report: Mutex<Option<FinalizeReport>>,
    /// The page before the loop ran: finalize and the screen reports cover
    /// only the roots the loop created (#116).
    run_baseline: WriteBaseline,
}

impl HeadlessDesignExecutor {
    pub(crate) fn new(state: Arc<Mutex<EditorState>>, brief: &str) -> Self {
        let run_baseline = WriteBaseline::capture(
            &state
                .lock()
                .expect("EditorState mutex poisoned before the loop started"),
        );
        Self {
            state,
            root_seed_guard: Mutex::new(RootSeedGuard::from_prompt(brief)),
            finalize_report: Mutex::new(None),
            run_baseline,
        }
    }

    pub(crate) fn take_finalize_report(&self) -> Option<FinalizeReport> {
        self.finalize_report
            .lock()
            .expect("finalize report mutex poisoned")
            .take()
    }
}

impl ChatToolExecutor for HeadlessDesignExecutor {
    fn execute(&self, name: &str, args_json: &str) -> ChatToolResult {
        // Sub-agent launches are desktop-only glue (sub_agent_session.rs);
        // the de_mcp stub would silently drop the sub-designers' work. Tell
        // the model honestly so it designs inline — the same contract the
        // headless smoke loop ships.
        if name == "spawn_agents" {
            let envelope = serde_json::json!({
                "success": false,
                "error": "spawn_agents is unavailable in the headless design loop — \
                          design the layout directly with batch_design instead of spawning sub-agents",
            });
            return ChatToolResult {
                content: envelope.to_string(),
                is_error: true,
            };
        }
        let mut state = self
            .state
            .lock()
            .expect("EditorState mutex poisoned by a panicking tool call");
        let mut root_seed_guard = self
            .root_seed_guard
            .lock()
            .expect("RootSeedGuard mutex poisoned by a panicking tool call");
        let (result, _mutated) = execute_agent_tool_with_root_seed_guard(
            &mut state,
            name,
            args_json,
            None,
            Some(&mut *root_seed_guard),
        );
        result
    }

    fn finalize(&self) -> FinalizeReport {
        let mut state = self
            .state
            .lock()
            .expect("EditorState mutex poisoned before finalize");
        let finalized = finalize_agent_run(&mut state, &self.run_baseline);
        let committed = agent_run_screens(&state, &self.run_baseline).committed;
        let report = FinalizeReport {
            screens: UnfilledScreensReport {
                committed,
                unfilled: finalized.unfilled,
            },
            quality: crate::quality_credential::quality_summary_from_repairs(&finalized.quality),
        };
        *self
            .finalize_report
            .lock()
            .expect("finalize report mutex poisoned") = Some(report.clone());
        report
    }

    fn check_unfilled_screens(&self) -> UnfilledScreensReport {
        let state = self
            .state
            .lock()
            .expect("EditorState mutex poisoned before check_unfilled_screens");
        let screens = agent_run_screens(&state, &self.run_baseline);
        UnfilledScreensReport {
            committed: screens.committed,
            unfilled: screens.unfilled,
        }
    }
}
