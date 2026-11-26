use std::path::{Path, PathBuf};
use std::time::Duration;

use de_host_services::headless_export::HeadlessExportFormat;

use super::*;
use crate::agent_tasks_error::TaskProblem;

fn parse(text: &str) -> Result<Vec<TaskSpec>, TasksFileError> {
    parse_tasks(
        text,
        "tasks.yaml",
        Path::new("base"),
        &TaskOptions::default(),
    )
}

fn base(path: &str) -> PathBuf {
    Path::new("base").join(path)
}

#[test]
fn a_minimal_task_gets_its_defaults() {
    let tasks = parse("tasks:\n  - prompt: A landing page\n    out: out/landing.de\n").unwrap();
    assert_eq!(tasks.len(), 1);
    let task = &tasks[0];
    assert_eq!(task.name, "landing");
    assert_eq!(task.prompt, "A landing page");
    assert_eq!(task.input, None);
    assert_eq!(task.output, base("out/landing.de"));
    assert_eq!(task.export_dir, base("out/exports/landing"));
    assert!(task.exports.is_empty());
    assert_eq!(task.timeout, Duration::from_secs(600));
    assert_eq!(task.max_turns, None);
}

#[test]
fn task_values_beat_the_command_line_which_beats_file_defaults() {
    let cli = TaskOptions {
        model: Some("cli-model".into()),
        timeout_secs: Some(30),
        ..TaskOptions::default()
    };
    let text = r#"
defaults:
  model: file-model
  provider: file-provider
  timeout: 900
  export: [png, pdf]
  exportDir: exports
  maxTurns: 12
tasks:
  - name: one
    prompt: first
    in: base.pen
    out: one.de
    model: task-model
    export: html
  - prompt: second
    out: two.de
"#;
    let tasks = parse_tasks(text, "t.yaml", Path::new("base"), &cli).unwrap();
    let (one, two) = (&tasks[0], &tasks[1]);
    // The task's own key wins over everything.
    assert_eq!(one.model.as_deref(), Some("task-model"));
    assert_eq!(one.exports, vec![HeadlessExportFormat::Html]);
    assert_eq!(one.input, Some(base("base.pen")));
    // Each task exports into its own folder under the root.
    assert_eq!(one.export_dir, base("exports/one"));
    assert_eq!(two.export_dir, base("exports/two"));
    assert_eq!(one.max_turns, Some(12));
    // A flag beats the file's defaults...
    assert_eq!(two.model.as_deref(), Some("cli-model"));
    assert_eq!(two.timeout, Duration::from_secs(30));
    // ...which still fill what no flag sets.
    assert_eq!(two.provider.as_deref(), Some("file-provider"));
    assert_eq!(
        two.exports,
        vec![HeadlessExportFormat::Png, HeadlessExportFormat::Pdf]
    );
}

#[test]
fn outs_are_compared_after_dots_and_case_folding() {
    let error = parse(
        "tasks:\n  - prompt: x\n    out: out/a.de\n  - prompt: y\n    out: out/sub/../a.de\n",
    )
    .unwrap_err();
    assert!(
        matches!(
            error,
            TasksFileError::DuplicateOut {
                first: 1,
                second: 2,
                ..
            }
        ),
        "{error}"
    );
    let cased = parse("tasks:\n  - prompt: x\n    out: A.de\n  - prompt: y\n    out: a.de\n");
    if crate::agent_paths::CASE_INSENSITIVE_FS {
        assert!(
            matches!(cased, Err(TasksFileError::DuplicateOut { .. })),
            "{cased:?}"
        );
    } else {
        assert!(cased.is_ok(), "{cased:?}");
    }
}

#[test]
fn names_must_be_folder_names_and_unique_ignoring_case() {
    let error = parse("tasks:\n  - {name: a/b, prompt: x, out: a.de}\n").unwrap_err();
    assert!(
        matches!(
            &error,
            TasksFileError::Task {
                problem: TaskProblem::InvalidName { .. },
                ..
            }
        ),
        "{error}"
    );
    assert_eq!(
        parse("tasks:\n  - {name: Home, prompt: x, out: a.de}\n  - {name: home, prompt: y, out: b.de}\n")
            .unwrap_err(),
        TasksFileError::DuplicateName { name: "home".into() }
    );
    // An out stem that cannot be a folder name falls back to task-<n>.
    let tasks = parse("tasks:\n  - {prompt: x, out: 'CON.de'}\n").unwrap();
    assert_eq!(tasks[0].name, "task-1");
}

#[test]
fn export_folders_may_not_overlap_or_land_on_documents() {
    // Task 2's export root is task 1's export folder: one holds the other.
    let error = parse(
        "tasks:\n  - {name: a, prompt: x, out: a.de, export: png, exportDir: shots}\n  - {name: b, prompt: y, out: b.de, export: png, exportDir: shots/a}\n",
    )
    .unwrap_err();
    assert!(
        matches!(
            error,
            TasksFileError::ExportDirClash {
                first: 1,
                second: 2,
                ..
            }
        ),
        "{error}"
    );
    // Overlap only matters when both export.
    assert!(parse(
        "tasks:\n  - {name: a, prompt: x, out: a.de, export: png, exportDir: shots}\n  - {name: b, prompt: y, out: b.de, exportDir: shots/a}\n",
    )
    .is_ok());
    // A task named after another document's asset folder.
    let error = parse(
        "tasks:\n  - {name: page.assets, prompt: x, out: other.de, export: png, exportDir: .}\n  - {prompt: y, out: page.de}\n",
    )
    .unwrap_err();
    assert!(
        matches!(
            error,
            TasksFileError::ExportDirOverDocument { index: 1, .. }
        ),
        "{error}"
    );
    assert!(error.to_string().contains("asset folder"), "{error}");
}

#[test]
fn an_oversized_prompt_is_refused_before_anything_runs() {
    let prompt = "x".repeat(de_host_services::design_agent_run::MAX_BRIEF_BYTES + 1);
    let error = parse(&format!("tasks:\n  - prompt: {prompt}\n    out: a.de\n")).unwrap_err();
    assert!(
        matches!(
            &error,
            TasksFileError::Task {
                problem: TaskProblem::PromptTooLarge { .. },
                ..
            }
        ),
        "{error}"
    );
    assert!(
        error.to_string().contains("the prompt is too large"),
        "{error}"
    );
}

#[test]
fn json_is_accepted_and_absolute_paths_stay_absolute() {
    let absolute = std::env::temp_dir().join("abs.de");
    let text = format!(
        r#"{{"tasks":[{{"prompt":"x","out":{}}}]}}"#,
        serde_json::to_string(&absolute.display().to_string()).unwrap()
    );
    let tasks = parse(&text).unwrap();
    assert_eq!(tasks[0].output, absolute);
}

#[test]
fn comma_separated_exports_and_duplicates_collapse() {
    let tasks = parse("tasks:\n  - prompt: x\n    out: a.de\n    export: png, PNG,svg\n").unwrap();
    assert_eq!(
        tasks[0].exports,
        vec![HeadlessExportFormat::Png, HeadlessExportFormat::Svg]
    );
}

#[test]
fn unnamed_tasks_with_the_same_stem_get_numbered_names() {
    let tasks = parse(
        "tasks:\n  - prompt: x\n    out: a/home.de\n  - prompt: y\n    out: b/home.de\n  - name: home2\n    prompt: z\n    out: c/home2.de\n",
    )
    .unwrap();
    let names: Vec<&str> = tasks.iter().map(|task| task.name.as_str()).collect();
    assert_eq!(names, vec!["home", "task-2", "home2"]);
}

#[test]
fn unknown_keys_are_parse_errors_with_a_location() {
    let error = parse("tasks:\n  - prompt: x\n    out: a.de\n    exprot: png\n").unwrap_err();
    let TasksFileError::Parse { path, detail } = &error else {
        panic!("expected a parse error, got {error:?}");
    };
    assert_eq!(path, "tasks.yaml");
    assert!(detail.contains("exprot"), "{detail}");
    assert!(error
        .to_string()
        .starts_with("invalid tasks file tasks.yaml: "));
}

#[test]
fn a_file_without_tasks_is_refused() {
    assert_eq!(
        parse("tasks: []\n").unwrap_err(),
        TasksFileError::NoTasks {
            path: "tasks.yaml".into()
        }
    );
    assert!(matches!(
        parse("defaults: {}\n").unwrap_err(),
        TasksFileError::Parse { .. }
    ));
}

#[test]
fn task_problems_name_the_task() {
    let cases: Vec<(&str, TaskProblem, &str)> = vec![
        (
            "tasks:\n  - out: a.de\n",
            TaskProblem::MissingPrompt,
            "task 1: `prompt` is required",
        ),
        (
            "tasks:\n  - name: hero\n    prompt: '  '\n    out: a.de\n",
            TaskProblem::MissingPrompt,
            "task 1 (hero): `prompt` is required",
        ),
        (
            "tasks:\n  - prompt: x\n",
            TaskProblem::MissingOut,
            "task 1: `out` is required",
        ),
        (
            "tasks:\n  - prompt: x\n    out: a.pen\n",
            TaskProblem::OutNotDe {
                out: base("a.pen").display().to_string(),
            },
            "must be a .de file",
        ),
        (
            "tasks:\n  - prompt: x\n    in: a.fig\n    out: a.de\n",
            TaskProblem::UnsupportedInput {
                input: base("a.fig").display().to_string(),
            },
            "`in` must be a .de, .pen or .pptx file",
        ),
        (
            "tasks:\n  - prompt: x\n    out: a.de\n    export: [png, gif]\n",
            TaskProblem::UnknownExport {
                value: "gif".into(),
            },
            "unknown export format \"gif\" (use png, pdf, html or svg)",
        ),
        (
            "tasks:\n  - prompt: x\n    out: a.de\n    timeout: 0\n",
            TaskProblem::InvalidTimeout { value: 0 },
            "`timeout` must be 1 to 86400 seconds, got 0",
        ),
        (
            "tasks:\n  - prompt: x\n    out: a.de\n    maxTurns: 65\n",
            TaskProblem::InvalidMaxTurns { value: 65 },
            "`maxTurns` must be 1 to 64, got 65",
        ),
    ];
    for (text, expected, message) in cases {
        let error = parse(text).unwrap_err();
        let TasksFileError::Task { index, problem, .. } = &error else {
            panic!("expected a task error for {text:?}, got {error:?}");
        };
        assert_eq!(*index, 1);
        assert_eq!(problem, &expected);
        assert!(error.to_string().contains(message), "{error}");
    }
}

#[test]
fn second_task_errors_carry_its_index() {
    let error = parse("tasks:\n  - prompt: x\n    out: a.de\n  - prompt: y\n").unwrap_err();
    assert_eq!(error.to_string(), "task 2: `out` is required");
}

#[test]
fn two_tasks_cannot_write_one_file_or_share_a_name() {
    assert_eq!(
        parse("tasks:\n  - prompt: x\n    out: a.de\n  - prompt: y\n    out: ./a.de\n")
            .unwrap_err(),
        TasksFileError::DuplicateOut {
            out: base("./a.de").display().to_string(),
            first: 1,
            second: 2,
        }
    );
    assert_eq!(
        parse("tasks:\n  - {name: n, prompt: x, out: a.de}\n  - {name: n, prompt: y, out: b.de}\n")
            .unwrap_err(),
        TasksFileError::DuplicateName { name: "n".into() }
    );
}

#[test]
fn a_missing_file_is_a_read_error() {
    let error = load_tasks_file(
        Path::new("definitely/not/here.yaml"),
        &TaskOptions::default(),
    )
    .unwrap_err();
    assert!(matches!(error, TasksFileError::Read { .. }));
    assert!(error
        .to_string()
        .starts_with("cannot read tasks file definitely"));
}
