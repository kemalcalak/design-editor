use std::path::{Path, PathBuf};
use std::time::Duration;

use de_host_services::headless_export::HeadlessExportFormat;

use super::*;
use crate::agent_tasks_error::TaskProblem;

fn parse(args: &[&str]) -> Result<AgentArgs, AgentArgsError> {
    let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
    parse_agent_args(&args)
}

fn single(args: &[&str]) -> TaskSpec {
    match parse(args).expect("parses").mode {
        AgentMode::Run(mut tasks) => {
            assert_eq!(tasks.len(), 1);
            tasks.remove(0)
        }
        other => panic!("expected one task, got {other:?}"),
    }
}

#[test]
fn a_full_single_prompt_command_line() {
    let task = single(&[
        "-p",
        "- a pricing page",
        "--in",
        "base.pen",
        "--out=out/pricing.de",
        "--export",
        "png,pdf",
        "--export",
        "svg",
        "--export-dir",
        "shots",
        "--model",
        "claude-x",
        "--provider",
        "Claude",
        "--timeout",
        "120",
        "--max-turns",
        "8",
    ]);
    assert_eq!(task.name, "pricing");
    assert_eq!(task.prompt, "- a pricing page");
    assert_eq!(task.input, Some(PathBuf::from("base.pen")));
    assert_eq!(task.output, PathBuf::from("out/pricing.de"));
    assert_eq!(
        task.exports,
        vec![
            HeadlessExportFormat::Png,
            HeadlessExportFormat::Pdf,
            HeadlessExportFormat::Svg
        ]
    );
    assert_eq!(task.export_dir, PathBuf::from("shots"));
    assert_eq!(task.model.as_deref(), Some("claude-x"));
    assert_eq!(task.provider.as_deref(), Some("Claude"));
    assert_eq!(task.timeout, Duration::from_secs(120));
    assert_eq!(task.max_turns, Some(8));
}

#[test]
fn switches_and_provider_flags() {
    let args = parse(&[
        "--prompt",
        "x",
        "--out",
        "a.de",
        "--json",
        "-q",
        "--api-format",
        "openai",
        "--base-url",
        "http://127.0.0.1:1/v1",
        "--api-key-env",
        "CI_KEY",
        "--settings",
        "s.json",
    ])
    .unwrap();
    assert!(args.json && args.quiet);
    assert_eq!(args.parallel, 1);
    assert_eq!(
        args.provider,
        ProviderFlags {
            api_format: Some("openai".into()),
            base_url: Some("http://127.0.0.1:1/v1".into()),
            api_key_env: Some("CI_KEY".into()),
            settings: Some(PathBuf::from("s.json")),
        }
    );
    assert_eq!(
        single(&["-p", "x", "--out", "a.de"]).export_dir,
        Path::new("a-export")
    );
}

#[test]
fn help_and_list_providers_need_no_task() {
    assert_eq!(parse(&["--help"]).unwrap().mode, AgentMode::Help);
    assert_eq!(
        parse(&["-h", "--bogus-is-not-reached"]).unwrap_err(),
        AgentArgsError::UnknownFlag {
            flag: "--bogus-is-not-reached".into()
        }
    );
    assert_eq!(
        parse(&["--list-providers", "--json"]).unwrap().mode,
        AgentMode::ListProviders
    );
}

#[test]
fn a_prompt_file_is_read() {
    let path = std::env::temp_dir().join(format!("de-agent-prompt-{}.txt", std::process::id()));
    std::fs::write(&path, "  A settings screen\n").unwrap();
    let task = single(&["--prompt-file", path.to_str().unwrap(), "--out", "a.de"]);
    assert_eq!(task.prompt, "A settings screen");
    let missing = parse(&["--prompt-file", "no/such/file.txt", "--out", "a.de"]).unwrap_err();
    assert!(matches!(missing, AgentArgsError::PromptFile { .. }));
    let _ = std::fs::remove_file(path);
}

#[test]
fn command_line_errors_name_the_flag() {
    let cases: Vec<(Vec<&str>, AgentArgsError, &str)> = vec![
        (
            vec!["--out", "a.de"],
            AgentArgsError::Task(TaskProblem::MissingPrompt),
            "-p/--prompt (or --tasks <file>) is required",
        ),
        (
            vec!["-p", "x"],
            AgentArgsError::Task(TaskProblem::MissingOut),
            "--out <file.de> is required",
        ),
        (
            vec!["-p", "x", "--out", "a.png"],
            AgentArgsError::Task(TaskProblem::OutNotDe {
                out: "a.png".into(),
            }),
            "--out must be a .de file, got \"a.png\"",
        ),
        (
            vec!["-p", "x", "--out", "a.de", "--export", "gif"],
            AgentArgsError::Task(TaskProblem::UnknownExport {
                value: "gif".into(),
            }),
            "unknown --export format \"gif\" (use png, pdf, html or svg)",
        ),
        (
            vec!["-p", "x", "--out", "a.de", "--timeout", "0"],
            AgentArgsError::InvalidNumber {
                flag: "--timeout".into(),
                value: "0".into(),
                min: 1,
                max: 86_400,
            },
            "--timeout must be a whole number from 1 to 86400, got \"0\"",
        ),
        (
            vec!["-p", "x", "--out", "a.de", "--max-turns", "lots"],
            AgentArgsError::InvalidNumber {
                flag: "--max-turns".into(),
                value: "lots".into(),
                min: 1,
                max: 64,
            },
            "--max-turns must be a whole number from 1 to 64, got \"lots\"",
        ),
        (
            vec!["-p"],
            AgentArgsError::MissingValue { flag: "-p".into() },
            "-p needs a value",
        ),
        (
            vec!["-p", "x", "--out", "a.de", "--frobnicate"],
            AgentArgsError::UnknownFlag {
                flag: "--frobnicate".into(),
            },
            "unknown flag --frobnicate for `de agent` (see `de agent --help`)",
        ),
        (
            vec!["make", "a", "page"],
            AgentArgsError::UnexpectedArgument { arg: "make".into() },
            "unexpected argument \"make\"; pass the prompt with -p \"...\"",
        ),
        (
            vec!["--tasks", "t.yaml", "-p", "x"],
            AgentArgsError::Conflict {
                flag: "--tasks".into(),
                with: "-p/--prompt".into(),
            },
            "--tasks cannot be combined with -p/--prompt",
        ),
        (
            vec!["-p", "x", "--out", "a.de", "--parallel", "2"],
            AgentArgsError::Conflict {
                flag: "--parallel".into(),
                with: "a single -p prompt (it needs --tasks)".into(),
            },
            "--parallel cannot be combined with a single -p prompt",
        ),
        (
            vec!["--tasks", "t.yaml", "--parallel", "0"],
            AgentArgsError::InvalidNumber {
                flag: "--parallel".into(),
                value: "0".into(),
                min: 1,
                max: MAX_PARALLEL,
            },
            "--parallel must be a whole number from 1 to 64",
        ),
    ];
    for (args, expected, message) in cases {
        let error = parse(&args).unwrap_err();
        assert_eq!(error, expected, "{args:?}");
        assert!(error.to_string().contains(message), "{error}");
    }
}

#[test]
fn a_tasks_file_takes_the_command_line_as_defaults() {
    let dir = std::env::temp_dir().join(format!("de-agent-args-tasks-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("tasks.yaml");
    std::fs::write(
        &file,
        "tasks:\n  - prompt: one\n    out: one.de\n  - prompt: two\n    out: two.de\n    model: own\n",
    )
    .unwrap();
    let args = parse(&[
        "--tasks",
        file.to_str().unwrap(),
        "--parallel",
        "2",
        "--model",
        "cli",
        "--export",
        "png",
    ])
    .unwrap();
    assert_eq!(args.parallel, 2);
    let AgentMode::Run(tasks) = args.mode else {
        panic!("expected tasks");
    };
    assert_eq!(tasks[0].output, dir.join("one.de"));
    assert_eq!(tasks[0].model.as_deref(), Some("cli"));
    assert_eq!(tasks[1].model.as_deref(), Some("own"));
    assert_eq!(tasks[1].exports, vec![HeadlessExportFormat::Png]);
    let missing = parse(&["--tasks", "no/such/tasks.yaml"]).unwrap_err();
    assert!(matches!(missing, AgentArgsError::Tasks(_)));
    let _ = std::fs::remove_dir_all(dir);
}
