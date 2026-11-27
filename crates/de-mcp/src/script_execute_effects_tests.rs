//! `execute_effects` judges a script by what it runs, not by its text
//! (#398's permission gate).

use de_editor_core::shared_state::SharedState;

use std::collections::BTreeMap;

use super::{execute_effects, execute_snapshot, pinned, ExecuteClock, ExecuteEffects};
use crate::test_fixtures::sample;
use crate::{McpTool, ToolOutcome};

const CLOCK: ExecuteClock = ExecuteClock {
    now_ms: 1_700_000_000_000,
};

fn effects(script: &str) -> Option<ExecuteEffects> {
    effects_at(script, CLOCK)
}

fn effects_at(script: &str, clock: ExecuteClock) -> Option<ExecuteEffects> {
    execute_effects(SharedState::new(sample()), script, None, clock)
}

/// The real run of `script` on `sample()`, held to `allowed` on `clock`.
fn real_run(script: &str, clock: ExecuteClock, allowed: Option<ExecuteEffects>) -> ToolOutcome {
    let args = BTreeMap::from([("script".to_string(), script.to_string())]);
    pinned(clock, allowed, || execute_snapshot(&sample()).call(&args))
}

#[test]
fn deletes_are_seen_however_the_call_is_spelled() {
    for script in [
        "Delete('n13')",
        "globalThis.Delete('n13')",
        "this.Delete('n13')",
        "globalThis['Del' + 'ete']('n13')",
        "const d = Delete; d('n13')",
    ] {
        let found = effects(script).unwrap_or_else(|| panic!("{script} ran"));
        assert_eq!(found.deletes, 1, "{script}");
        assert!(found.is_destructive(), "{script}");
    }
}

#[test]
fn replaces_exports_and_a_variable_overwrite_are_destructive() {
    assert_eq!(
        effects("Replace('n13', {type: 'ellipse', name: 'Dot', width: 8, height: 8})")
            .unwrap()
            .replaces,
        1
    );
    assert_eq!(
        effects("Export(['n13'], 'png', '/tmp/out')")
            .unwrap()
            .exports,
        1
    );
    assert!(
        effects("SetVariables({}, true)")
            .unwrap()
            .replaces_variables
    );
    assert!(!effects("SetVariables({})").unwrap().replaces_variables);
}

#[test]
fn reads_and_plain_writes_are_not() {
    let found = effects("Get('n13'); Update('n13', {name: 'Renamed'}); Print('ok')").unwrap();
    assert!(!found.is_destructive());
}

#[test]
fn a_script_that_cannot_be_judged_is_none() {
    // A throw, an unknown name (`x` is undefined) or a failed write.
    assert_eq!(effects("throw new Error('no')"), None);
    assert_eq!(effects("x.Delete()"), None);
    assert_eq!(effects("Delete('missing-node')"), None);
    assert_eq!(effects("   "), None);
}

const RANDOM_DELETE: &str = "if (Math.random() < 0.5) Delete('n13'); else Print('kept')";

#[test]
fn a_random_branch_runs_the_same_way_on_one_clock() {
    let mut seen = [false, false];
    for now_ms in 0..16u64 {
        let clock = ExecuteClock {
            now_ms: 1_700_000_000_000 + now_ms * 7919,
        };
        let first = effects_at(RANDOM_DELETE, clock).expect("ran");
        let second = effects_at(RANDOM_DELETE, clock).expect("ran");
        assert_eq!(first, second, "one clock, one outcome");
        seen[usize::from(first.is_destructive())] = true;
        // The real run on that clock does what the dry run said.
        let ran = real_run(RANDOM_DELETE, clock, Some(first.clone()));
        assert!(!matches!(ran, ToolOutcome::Err(..)), "{ran:?}");
    }
    assert_eq!(
        seen,
        [true, true],
        "both branches are reachable across clocks"
    );
}

#[test]
fn the_clock_freezes_date() {
    let found = effects(
        "if (Date.now() === 1700000000000 && new Date().getTime() === Date.now()) Delete('n13')",
    )
    .unwrap();
    assert_eq!(found.deletes, 1);
    assert_eq!(
        effects("new Date(0).getTime() === 0 && Delete('n13')")
            .unwrap()
            .deletes,
        1
    );
}

#[test]
fn a_real_run_that_goes_past_its_prompt_applies_nothing() {
    // The dry run read nothing destructive; the real run deletes.
    let ran = real_run("Delete('n13')", CLOCK, Some(ExecuteEffects::default()));
    let ToolOutcome::Err(_, message) = ran else {
        panic!("refused: {ran:?}");
    };
    assert!(message.contains("permission prompt"), "{message}");
    // Another node than the one asked about is refused too.
    let asked = effects("Delete('n13')").unwrap();
    let other = real_run("Delete('n12')", CLOCK, Some(asked.clone()));
    assert!(matches!(other, ToolOutcome::Err(..)), "{other:?}");
    // What was asked about runs; an unjudged script the user allowed runs.
    assert!(!matches!(
        real_run("Delete('n13')", CLOCK, Some(asked)),
        ToolOutcome::Err(..)
    ));
    assert!(!matches!(
        real_run("Delete('n13')", CLOCK, None),
        ToolOutcome::Err(..)
    ));
}
