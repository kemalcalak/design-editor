//! `execute_effects` judges a script by what it runs, not by its text
//! (#398's permission gate).

use de_editor_core::shared_state::SharedState;

use super::{execute_effects, ExecuteEffects};
use crate::test_fixtures::sample;

fn effects(script: &str) -> Option<ExecuteEffects> {
    execute_effects(SharedState::new(sample()), script, None)
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
