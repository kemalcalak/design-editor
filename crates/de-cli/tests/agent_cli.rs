//! `de agent` end to end: the real binary, the real engine and provider
//! client, against a loopback fake provider (`agent_cli/fake_provider.rs`).
#![cfg(feature = "agent")]

#[path = "agent_cli/fake_provider.rs"]
mod fake_provider;

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use fake_provider::{FakeProvider, TURN_INPUT_TOKENS};
use serde_json::Value;

const FAKE_KEY: &str = "sk-fake-397-never-printed";

/// A fresh folder per test.
fn temp_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir =
        std::env::temp_dir().join(format!("de-agent-cli-{tag}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

/// `de agent …` wired to `provider` through the environment, isolated from
/// the developer's own settings and variables.
fn agent(provider: &FakeProvider, dir: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_de"));
    command.arg("agent");
    for var in [
        "DE_AGENT_API_KEY",
        "DE_AGENT_API_FORMAT",
        "DE_AGENT_BASE_URL",
        "DE_AGENT_MODEL",
        "DE_AGENT_PROVIDER",
        "DE_AGENT_SETTINGS",
    ] {
        command.env_remove(var);
    }
    command
        .env("DE_AGENT_API_KEY", FAKE_KEY)
        .env("DE_AGENT_BASE_URL", &provider.base_url)
        .env("DE_AGENT_MODEL", "fake-model")
        .env("DE_AGENT_SETTINGS", dir.join("no-settings.json"))
        .stdin(Stdio::null());
    command
}

fn run(command: &mut Command) -> Output {
    let output = command.output().expect("run de agent");
    let (stdout, stderr) = texts(&output);
    assert!(
        !stdout.contains(FAKE_KEY) && !stderr.contains(FAKE_KEY),
        "the API key leaked into the output"
    );
    output
}

fn texts(output: &Output) -> (String, String) {
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn json(output: &Output) -> Value {
    let (stdout, stderr) = texts(output);
    serde_json::from_str(&stdout).unwrap_or_else(|error| {
        panic!("stdout is not JSON ({error}):\n{stdout}\nstderr:\n{stderr}")
    })
}

fn code(output: &Output) -> i32 {
    output.status.code().expect("exit code")
}

/// The document's top-level frame names.
fn frame_names(path: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(path).expect("read out document");
    let document: Value = serde_json::from_str(&text).expect("out is JSON");
    let roots = document["children"]
        .as_array()
        .cloned()
        .or_else(|| document["pages"][0]["children"].as_array().cloned())
        .unwrap_or_default();
    roots
        .iter()
        .filter_map(|node| node["name"].as_str().map(str::to_string))
        .collect()
}

fn leftover_temp_files(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| name.ends_with(".tmp"))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_single_prompt_saves_the_design_and_reports_json() {
    let provider = FakeProvider::start();
    let dir = temp_dir("single");
    let out = dir.join("out").join("hero.de");
    let output = run(agent(&provider, &dir).args([
        "-p",
        "A hero screen <<frame:Hero>>",
        "--out",
        out.to_str().unwrap(),
        "--json",
    ]));
    assert_eq!(code(&output), 0, "{:?}", texts(&output));
    assert_eq!(frame_names(&out), vec!["Hero"]);

    let report = json(&output);
    assert_eq!(report["ok"], true);
    assert_eq!(report["summary"]["succeeded"], 1);
    let task = &report["tasks"][0];
    assert_eq!(task["name"], "hero");
    assert_eq!(task["status"], "ok");
    assert_eq!(task["saved"], true);
    assert_eq!(task["model"], "fake-model");
    assert_eq!(task["provider"], "environment (DE_AGENT_API_KEY)");
    assert_eq!(task["toolCalls"], 1);
    assert_eq!(task["landedRoots"], 1);
    let requests = provider.requests() as u64;
    assert_eq!(task["usage"]["requests"], requests);
    assert_eq!(task["usage"]["inputTokens"], TURN_INPUT_TOKENS * requests);
    assert_eq!(report["summary"]["usage"], task["usage"]);
    assert!(task["error"].is_null());

    // The key reached the provider (and only there).
    assert!(provider.api_keys().iter().all(|key| key == FAKE_KEY));
    let (_, stderr) = texts(&output);
    assert!(
        stderr.contains("de agent [hero] started: fake-model"),
        "{stderr}"
    );
    assert!(
        stderr.contains("de agent [hero] tool batch_design"),
        "{stderr}"
    );
    assert!(stderr.contains("de agent [hero] ok in"), "{stderr}");
}

#[test]
fn an_input_document_is_read_not_written() {
    let provider = FakeProvider::start();
    let dir = temp_dir("input");
    let input = dir.join("base.de");
    let original = r##"{"version":"1.0.0","children":[{"type":"frame","id":"old","name":"Old","x":0,"y":0,"width":100,"height":100}]}"##;
    std::fs::write(&input, original).unwrap();
    let out = dir.join("next.de");
    let output = run(agent(&provider, &dir).args([
        "-p",
        "Redo it <<frame:New>>",
        "--in",
        input.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--quiet",
    ]));
    assert_eq!(code(&output), 0, "{:?}", texts(&output));
    assert_eq!(std::fs::read_to_string(&input).unwrap(), original);
    // The agent works on a copy of the input page: its frame joins the
    // board the input already had.
    let mut names = frame_names(&out);
    names.sort();
    assert_eq!(names, vec!["New", "Old"]);
    let (stdout, stderr) = texts(&output);
    assert!(stderr.is_empty(), "--quiet keeps stderr empty: {stderr}");
    assert!(stdout.contains("1/1 task(s) succeeded"), "{stdout}");
}

#[test]
fn a_pen_dev_file_is_imported_as_the_starting_point() {
    let provider = FakeProvider::start();
    let dir = temp_dir("pen");
    let input = dir.join("start.pen");
    let original = include_str!("../../de-pen-loader/tests/fixtures/pen-dev-variables.pen");
    std::fs::write(&input, original).unwrap();
    let out = dir.join("from-pen.de");
    let output = run(agent(&provider, &dir).args([
        "-p",
        "Add a screen <<frame:Added>>",
        "--in",
        input.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]));
    assert_eq!(code(&output), 0, "{:?}", texts(&output));
    assert_eq!(std::fs::read_to_string(&input).unwrap(), original);
    let names = frame_names(&out);
    assert!(names.contains(&"Added".to_string()), "{names:?}");
    assert!(names.contains(&"Chip".to_string()), "{names:?}");
}

#[test]
fn every_export_format_lands_in_the_export_folder() {
    let provider = FakeProvider::start();
    let dir = temp_dir("export");
    let out = dir.join("shop.de");
    let exports = dir.join("exports");
    let output = run(agent(&provider, &dir).args([
        "-p",
        "A shop <<frame:Shop>>",
        "--out",
        out.to_str().unwrap(),
        "--export",
        "png,pdf",
        "--export",
        "html",
        "--export",
        "svg",
        "--export-dir",
        exports.to_str().unwrap(),
        "--json",
    ]));
    assert_eq!(code(&output), 0, "{:?}", texts(&output));
    // The task's own folder under the export root.
    let folder = exports.join("shop");
    for file in ["01-Shop.png", "shop.pdf", "shop.html", "01-Shop.svg"] {
        assert!(folder.join(file).is_file(), "missing {file}");
    }
    let report = json(&output);
    let formats: Vec<&str> = report["tasks"][0]["exports"]
        .as_array()
        .unwrap()
        .iter()
        .map(|export| export["format"].as_str().unwrap())
        .collect();
    assert_eq!(formats, vec!["png", "pdf", "html", "svg"]);
    assert!(std::fs::read(folder.join("01-Shop.png"))
        .unwrap()
        .starts_with(b"\x89PNG"));
}

fn write_tasks(dir: &Path, body: &str) -> PathBuf {
    let path = dir.join("tasks.yaml");
    std::fs::write(&path, body).unwrap();
    path
}

const THREE_TASKS: &str = "\
defaults:
  timeout: 120
tasks:
  - name: home
    prompt: Home <<frame:Home>>
    out: out/home.de
  - prompt: Pricing <<frame:Pricing>>
    out: out/pricing.de
    export: png
  - prompt: About <<frame:About>>
    out: out/about.de
";

#[test]
fn a_tasks_file_runs_in_order_one_at_a_time() {
    let provider = FakeProvider::start();
    let dir = temp_dir("tasks-seq");
    let tasks = write_tasks(&dir, THREE_TASKS);
    let output = run(agent(&provider, &dir).args(["--tasks", tasks.to_str().unwrap(), "--json"]));
    assert_eq!(code(&output), 0, "{:?}", texts(&output));
    let report = json(&output);
    let names: Vec<&str> = report["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|task| task["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["home", "pricing", "about"]);
    assert_eq!(report["summary"]["total"], 3);
    assert_eq!(report["summary"]["succeeded"], 3);
    assert_eq!(frame_names(&dir.join("out/pricing.de")), vec!["Pricing"]);
    assert!(dir.join("out/exports/pricing/01-Pricing.png").is_file());
    // Sequential: each task starts after the previous one finished.
    let (_, stderr) = texts(&output);
    let position = |needle: &str| stderr.find(needle).unwrap_or_else(|| panic!("{needle}"));
    assert!(position("[home] ok in") < position("[pricing] started"));
    assert!(position("[pricing] ok in") < position("[about] started"));
}

#[test]
fn a_tasks_file_runs_in_parallel() {
    let provider = FakeProvider::start();
    let dir = temp_dir("tasks-par");
    let tasks = write_tasks(&dir, THREE_TASKS);
    let output = run(agent(&provider, &dir).args([
        "--tasks",
        tasks.to_str().unwrap(),
        "--parallel",
        "3",
        "--json",
    ]));
    assert_eq!(code(&output), 0, "{:?}", texts(&output));
    let report = json(&output);
    assert_eq!(report["summary"]["succeeded"], 3);
    let total: u64 = report["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|task| task["usage"]["inputTokens"].as_u64().unwrap())
        .sum();
    assert_eq!(report["summary"]["usage"]["inputTokens"], total);
    for (file, frame) in [("home", "Home"), ("pricing", "Pricing"), ("about", "About")] {
        assert_eq!(
            frame_names(&dir.join(format!("out/{file}.de"))),
            vec![frame]
        );
    }
    let (_, stderr) = texts(&output);
    assert!(stderr.contains("3 tasks, 3 at a time"), "{stderr}");
}

#[test]
fn a_failed_task_fails_the_run_but_not_the_other_tasks() {
    let provider = FakeProvider::start();
    let dir = temp_dir("tasks-fail");
    let tasks = write_tasks(
        &dir,
        "tasks:\n  - prompt: Good <<frame:Good>>\n    out: good.de\n  - prompt: Bad <<fail>>\n    out: bad.de\n",
    );
    let output = run(agent(&provider, &dir).args(["--tasks", tasks.to_str().unwrap(), "--json"]));
    assert_eq!(code(&output), 1, "{:?}", texts(&output));
    let report = json(&output);
    assert_eq!(report["ok"], false);
    assert_eq!(report["summary"]["failed"], 1);
    assert_eq!(report["tasks"][0]["status"], "ok");
    let bad = &report["tasks"][1];
    assert_eq!(bad["status"], "failed");
    assert_eq!(bad["saved"], false);
    assert_eq!(bad["error"]["stage"], "agent");
    assert!(
        bad["error"]["message"]
            .as_str()
            .unwrap()
            .contains("anthropic http 400"),
        "{bad}"
    );
    // A refused run still reports the requests it made.
    assert!(bad["usage"]["requests"].as_u64().unwrap() >= 1, "{bad}");
    assert!(dir.join("good.de").is_file());
    assert!(!dir.join("bad.de").exists());
}

#[test]
fn a_failed_run_leaves_an_existing_out_byte_for_byte() {
    let provider = FakeProvider::start();
    let dir = temp_dir("atomic");
    let out = dir.join("keep.de");
    std::fs::write(&out, "ORIGINAL BYTES").unwrap();
    let output =
        run(agent(&provider, &dir).args(["-p", "Break <<fail>>", "--out", out.to_str().unwrap()]));
    assert_eq!(code(&output), 1, "{:?}", texts(&output));
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "ORIGINAL BYTES");
    assert_eq!(leftover_temp_files(&dir), Vec::<String>::new());
    let (stdout, stderr) = texts(&output);
    assert!(stdout.contains("failed  keep  agent:"), "{stdout}");
    assert!(stderr.contains("[keep] failed in"), "{stderr}");
}

#[test]
fn the_timeout_stops_a_stalled_provider() {
    let provider = FakeProvider::start();
    let dir = temp_dir("timeout");
    let out = dir.join("slow.de");
    let output = run(agent(&provider, &dir).args([
        "-p",
        "Slow <<hang>>",
        "--out",
        out.to_str().unwrap(),
        "--timeout",
        "1",
        "--json",
    ]));
    assert_eq!(code(&output), 1, "{:?}", texts(&output));
    let report = json(&output);
    let message = report["tasks"][0]["error"]["message"].as_str().unwrap();
    assert!(message.contains("exceeded its 1s budget"), "{message}");
    assert!(!out.exists());
}

#[test]
fn argument_and_provider_mistakes_exit_2_before_anything_runs() {
    let provider = FakeProvider::start();
    let dir = temp_dir("usage");
    let cases: Vec<(Vec<String>, &str)> = vec![
        (vec!["-p".into(), "x".into()], "--out <file.de> is required"),
        (
            vec![
                "--tasks".into(),
                dir.join("missing.yaml").display().to_string(),
            ],
            "cannot read tasks file",
        ),
        (
            vec![
                "-p".into(),
                "x".into(),
                "--out".into(),
                dir.join("a.de").display().to_string(),
                "--provider".into(),
                "nope".into(),
            ],
            "task a: no saved provider matches \"nope\"; none are saved",
        ),
    ];
    for (args, message) in cases {
        let output = run(agent(&provider, &dir).args(&args));
        assert_eq!(code(&output), 2, "{args:?}");
        let (stdout, stderr) = texts(&output);
        assert!(stdout.is_empty(), "{stdout}");
        assert!(stderr.contains(message), "{stderr}");
    }
    let bad_tasks = write_tasks(
        &dir,
        "tasks:\n  - prompt: x\n    out: a.de\n    colour: red\n",
    );
    let output = run(agent(&provider, &dir).args(["--tasks", bad_tasks.to_str().unwrap()]));
    assert_eq!(code(&output), 2);
    assert!(texts(&output).1.contains("colour"), "{:?}", texts(&output));
    let no_key = run(agent(&provider, &dir).env_remove("DE_AGENT_API_KEY").args([
        "-p",
        "x",
        "--out",
        dir.join("a.de").to_str().unwrap(),
    ]));
    assert_eq!(code(&no_key), 2);
    assert!(texts(&no_key).1.contains("no agent provider is ready"));
    assert_eq!(provider.requests(), 0);
    assert!(!dir.join("a.de").exists());
}

#[test]
fn saved_providers_are_listed_without_their_keys() {
    let provider = FakeProvider::start();
    let dir = temp_dir("list");
    let settings = dir.join("settings.json");
    std::fs::write(
        &settings,
        format!(
            r#"{{"version":1,"builtin_agents":[{{"id":"builtin-1","display_name":"Loopback","kind":"anthropic","api_key":"{FAKE_KEY}","model":"fake-model","base_url":"{}","enabled":true}}]}}"#,
            provider.base_url
        ),
    )
    .unwrap();
    let output = run(agent(&provider, &dir).args([
        "--list-providers",
        "--json",
        "--settings",
        settings.to_str().unwrap(),
    ]));
    assert_eq!(code(&output), 0);
    let listing = json(&output);
    assert_eq!(listing["providers"][0]["id"], "builtin-1");
    assert_eq!(listing["providers"][0]["ready"], true);
    assert_eq!(listing["providers"][0]["hasKey"], true);

    // The saved provider runs when named, the environment key unused.
    let out = dir.join("saved.de");
    let output = run(agent(&provider, &dir).env_remove("DE_AGENT_MODEL").args([
        "-p",
        "Saved <<frame:Saved>>",
        "--out",
        out.to_str().unwrap(),
        "--provider",
        "loopback",
        "--settings",
        settings.to_str().unwrap(),
        "--json",
    ]));
    assert_eq!(code(&output), 0, "{:?}", texts(&output));
    assert_eq!(
        json(&output)["tasks"][0]["provider"],
        "Loopback (builtin-1)"
    );
    assert_eq!(frame_names(&out), vec!["Saved"]);
}

#[cfg(unix)]
#[test]
fn ctrl_c_cancels_the_run_and_leaves_out_untouched() {
    use std::io::{BufRead, BufReader};

    let provider = FakeProvider::start();
    let dir = temp_dir("cancel");
    let out = dir.join("cancel.de");
    std::fs::write(&out, "BEFORE").unwrap();
    let mut child = agent(&provider, &dir)
        .args([
            "-p",
            "Wait <<hang>>",
            "--out",
            out.to_str().unwrap(),
            "--json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn de agent");
    let stderr = child.stderr.take().unwrap();
    let mut lines = BufReader::new(stderr).lines();
    let started = lines
        .by_ref()
        .map_while(Result::ok)
        .any(|line| line.contains("started:"));
    assert!(started, "the task never started");
    // Wait until the request is in flight, then press Ctrl+C.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while provider.requests() == 0 && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let pid = child.id() as libc::pid_t;
    // SAFETY: plain kill(2) on the child this test spawned and still owns.
    assert_eq!(unsafe { libc::kill(pid, libc::SIGINT) }, 0);
    let rest: Vec<String> = lines.map_while(Result::ok).collect();
    let output = child.wait_with_output().expect("wait for de agent");
    assert_eq!(code(&output), 130, "stderr: {rest:?}");
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "BEFORE");
    assert_eq!(leftover_temp_files(&dir), Vec::<String>::new());
    let report: Value = serde_json::from_slice(&output.stdout).expect("JSON report");
    assert_eq!(report["cancelled"], true);
    assert_eq!(report["ok"], false);
    assert_eq!(report["tasks"][0]["status"], "cancelled");
    assert_eq!(report["tasks"][0]["saved"], false);
    assert!(rest
        .iter()
        .any(|line| line.contains("[cancel] cancelled after")));
}

/// A `.de` with one image whose blur-up thumbnail is `thumb`.
fn document_with_thumb(src: &str, thumb: &[u8]) -> String {
    use base64::Engine as _;
    let id = jian_ops_schema::node::image_src::paint_image_id(src);
    // A current document (release `version` + schema `formatVersion`), as
    // the app writes it.
    serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "formatVersion": "1.0",
        "children": [{
            "type": "frame", "id": "keep", "name": "Keep", "x": 0, "y": 0,
            "width": 200, "height": 200,
            "children": [{"type": "image", "id": "img", "src": src, "width": 100, "height": 100}]
        }],
        "imageThumbs": { id.to_string(): base64::engine::general_purpose::STANDARD.encode(thumb) }
    })
    .to_string()
}

fn thumbs_of(path: &Path) -> serde_json::Map<String, Value> {
    let document: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("read out")).expect("JSON");
    document["imageThumbs"]
        .as_object()
        .cloned()
        .unwrap_or_default()
}

#[test]
fn parallel_tasks_keep_their_own_image_thumbnails() {
    let provider = FakeProvider::start();
    let dir = temp_dir("thumbs");
    let sources = [
        (
            "one",
            "https://example.com/one.png",
            [0xff, 0xd8, 0x01, 0xd9],
        ),
        (
            "two",
            "https://example.com/two.png",
            [0xff, 0xd8, 0x02, 0xd9],
        ),
    ];
    // Round 1 starts from single-file documents (inline base64 thumbnails)
    // and saves folder-layout ones; round 2 starts from those (thumbnails
    // as `<name>.assets/<hash>.jpg` paths) and saves again.
    for (name, src, thumb) in &sources {
        std::fs::write(
            dir.join(format!("{name}-in.de")),
            document_with_thumb(src, thumb),
        )
        .unwrap();
    }
    for (round, input) in [(1, "{name}-in.de"), (2, "{name}-r1.de")] {
        let mut tasks = String::from("tasks:\n");
        for (name, _, _) in &sources {
            let input = input.replace("{name}", name);
            tasks.push_str(&format!(
                "  - {{name: {name}, prompt: 'Add <<frame:{name}{round}>>', in: {input}, out: {name}-r{round}.de}}\n"
            ));
        }
        let tasks = write_tasks(&dir, &tasks);
        let output = run(agent(&provider, &dir).args([
            "--tasks",
            tasks.to_str().unwrap(),
            "--parallel",
            "2",
            "--quiet",
        ]));
        assert_eq!(code(&output), 0, "round {round}: {:?}", texts(&output));
        for (name, src, thumb) in &sources {
            let saved = thumbs_of(&dir.join(format!("{name}-r{round}.de")));
            let id = jian_ops_schema::node::image_src::paint_image_id(src).to_string();
            assert_eq!(
                saved.len(),
                1,
                "round {round}: {name} keeps only its own thumbnail: {saved:?}"
            );
            // The save moves the thumbnail into the document's asset folder.
            let file = saved[&id].as_str().expect("thumbnail path");
            assert!(
                file.starts_with(&format!("{name}-r{round}.assets/")),
                "{file}"
            );
            assert_eq!(
                std::fs::read(dir.join(file)).expect("thumbnail file"),
                thumb.to_vec(),
                "round {round}: {name}"
            );
        }
    }
}

#[test]
fn parallel_tasks_share_one_request_pacer() {
    let provider = FakeProvider::start();
    let dir = temp_dir("pacer");
    let tasks = write_tasks(&dir, THREE_TASKS);
    let output = run(agent(&provider, &dir)
        .env(
            "DESIGN_EDITOR_BUILTIN_HTTP_MIN_GAP_MS",
            PACER_GAP_MS.to_string(),
        )
        .args([
            "--tasks",
            tasks.to_str().unwrap(),
            "--parallel",
            "3",
            "--quiet",
        ]));
    assert_eq!(code(&output), 0, "{:?}", texts(&output));
    let arrivals = provider.arrivals();
    let n = arrivals.len() as u32;
    assert!(n >= 6, "{n} requests");
    // Three workers with their own pacers would send their first requests
    // together. One shared pacer spaces all n requests at least one gap
    // apart, so they span (n - 1) gaps; 0.8 leaves room for scheduling and
    // accept-loop jitter (it only errs towards a longer span) without
    // letting three concurrent first requests pass.
    let span = arrivals[arrivals.len() - 1].duration_since(arrivals[0]);
    let floor = std::time::Duration::from_millis(PACER_GAP_MS) * (n - 1) * 8 / 10;
    assert!(
        span >= floor,
        "{n} requests in {span:?}, expected at least {floor:?}"
    );
}

/// The pacer gap the shared-pacer test sets.
const PACER_GAP_MS: u64 = 400;
