//! `de start` argument-parsing tests (sibling file per the 800-line-cap
//! convention; split from `tests.rs`).

use super::*;

#[test]
fn parse_args_maps_start_to_rust_mcp_server() {
    let p = parse_args(&[
        "--port".to_string(),
        "3200".to_string(),
        "start".to_string(),
        "--file".to_string(),
        "/tmp/session.de".to_string(),
    ])
    .expect("parse start");
    assert_eq!(p.port, 3200);
    assert!(p.port_explicit, "--port should mark the port explicit");
    assert_eq!(
        p.command,
        Command::StartMcp {
            document_path: Some("/tmp/session.de".to_string()),
            headless: false,
            web: false,
            host: None,
            wait_secs: None,
        }
    );
}

#[test]
fn parse_args_maps_start_headless_to_file_backed_server() {
    let p = parse_args(&["start".to_string(), "--headless".to_string()]).expect("parse start");
    assert_eq!(
        p.command,
        Command::StartMcp {
            document_path: None,
            headless: true,
            web: false,
            host: None,
            wait_secs: None,
        }
    );
}

#[test]
fn parse_args_maps_start_web_to_browser_editor() {
    let p = parse_args(&["start".to_string(), "--web".to_string()]).expect("parse start --web");
    assert_eq!(
        p.command,
        Command::StartMcp {
            document_path: None,
            headless: false,
            web: true,
            host: None,
            wait_secs: None,
        }
    );
}

#[test]
fn parse_args_start_web_threads_host_and_file() {
    let p = parse_args(&[
        "start".to_string(),
        "--web".to_string(),
        "--host".to_string(),
        "0.0.0.0".to_string(),
        "--file".to_string(),
        "/tmp/session.de".to_string(),
    ])
    .expect("parse start --web --host");
    assert_eq!(
        p.command,
        Command::StartMcp {
            document_path: Some("/tmp/session.de".to_string()),
            headless: false,
            web: true,
            host: Some("0.0.0.0".to_string()),
            wait_secs: None,
        }
    );
}

#[test]
fn parse_args_rejects_conflicting_or_dangling_start_flags() {
    // --web and --headless are different daemons — refuse the ambiguity.
    let conflict = parse_args(&[
        "start".to_string(),
        "--web".to_string(),
        "--headless".to_string(),
    ]);
    assert!(conflict.is_err(), "--web --headless must be rejected");
    // --host without --web would silently bind nothing — refuse it too.
    let dangling = parse_args(&[
        "start".to_string(),
        "--host".to_string(),
        "0.0.0.0".to_string(),
    ]);
    assert!(dangling.is_err(), "--host without --web must be rejected");
}

#[test]
fn parse_args_reads_the_headless_readiness_wait() {
    let args = |extra: &[&str]| {
        let mut all = vec!["start".to_string(), "--headless".to_string()];
        all.extend(extra.iter().map(|s| s.to_string()));
        parse_args(&all)
    };
    let p = args(&["--wait", "90"]).expect("parse --wait");
    assert!(matches!(
        p.command,
        Command::StartMcp {
            wait_secs: Some(90),
            ..
        }
    ));
    let p = args(&["--wait=5"]).expect("parse --wait=");
    assert!(matches!(
        p.command,
        Command::StartMcp {
            wait_secs: Some(5),
            ..
        }
    ));
    assert!(args(&["--wait"]).is_err(), "--wait needs a value");
    for bad in ["0", "601", "soon"] {
        assert!(args(&["--wait", bad]).is_err(), "--wait {bad} is refused");
    }
}

#[test]
fn parse_args_refuses_wait_without_headless() {
    let p = parse_args(&["start".to_string(), "--wait".to_string(), "30".to_string()]);
    assert!(p.is_err());
}
