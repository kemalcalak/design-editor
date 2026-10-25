//! Shared argv dispatcher for the headless CLI server modes.
//!
//! Both binaries expose the same three modes — `--mcp <path>` (JSON-RPC
//! stdio MCP server), `--mcp-http <port> <path>` (Streamable-HTTP MCP
//! server), and `--serve-web <port> [doc] [--host <addr>]` (headless
//! web-canvas daemon) — differing only in the program name their
//! diagnostics carry. `de-host-desktop` falls back to the GUI on an
//! unknown mode; `de-host-web-server` treats it as a usage error. Both
//! decisions stay with the caller.
//!
//! Every mode runs on its own thread with [`MODE_STACK_BYTES`] of stack,
//! not on the caller's: Windows gives a process's main thread 1 MiB, and a
//! debug daemon loading a document already needs about that (#227).

use std::path::PathBuf;

/// Stack of the thread a server mode runs on. Reserved address space, not
/// committed memory, so a generous size costs nothing until it is used.
pub const MODE_STACK_BYTES: usize = 64 * 1024 * 1024;

/// Run `work` on a fresh thread with [`MODE_STACK_BYTES`] of stack and
/// wait for it. A panic in `work` resumes on the caller.
pub fn on_mode_stack<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    let spawned = std::thread::Builder::new()
        .name("de-server-mode".into())
        .stack_size(MODE_STACK_BYTES)
        .spawn(work);
    match spawned {
        Ok(handle) => handle
            .join()
            .unwrap_or_else(|panic| std::panic::resume_unwind(panic)),
        Err(err) => panic!("could not start the server-mode thread: {err}"),
    }
}

/// Run the headless server mode named by `mode` (the first argv token),
/// consuming the remaining `args`.
///
/// - `None` — `mode` is not one of the three server modes; the caller
///   decides what an unknown mode means.
/// - `Some(0)` — the mode ran to completion.
/// - `Some(code)` — the mode failed; the diagnostic (prefixed with
///   `"{prog} {mode}: "`) has already been printed to stderr and the
///   caller should exit with `code` (2 = malformed invocation, 1 =
///   runtime failure).
pub fn run_cli_mode(prog: &str, mode: &str, args: impl Iterator<Item = String>) -> Option<i32> {
    if !matches!(mode, "--mcp" | "--mcp-http" | "--serve-web") {
        return None;
    }
    let (prog, mode) = (prog.to_string(), mode.to_string());
    let args: Vec<String> = args.collect();
    on_mode_stack(move || run_mode(&prog, &mode, args.into_iter()))
}

fn run_mode(prog: &str, mode: &str, mut args: impl Iterator<Item = String>) -> Option<i32> {
    match mode {
        "--mcp" => {
            let Some(path) = args.next() else {
                eprintln!("{prog} --mcp: missing <path> arg");
                return Some(2);
            };
            crate::user_scene_template_store::initialize_user_scene_templates_once();
            crate::home_preview::enable();
            if let Err(e) = crate::mcp_serve::run(PathBuf::from(path)) {
                eprintln!("{prog} --mcp: {e}");
                return Some(1);
            }
            Some(0)
        }
        "--mcp-http" => {
            let Some(port_arg) = args.next() else {
                eprintln!("{prog} --mcp-http: missing <port> arg");
                return Some(2);
            };
            let Ok(port) = port_arg.parse::<u16>() else {
                eprintln!("{prog} --mcp-http: <port> must be a u16, got {port_arg:?}");
                return Some(2);
            };
            let Some(path) = args.next() else {
                eprintln!("{prog} --mcp-http: missing <path> arg");
                return Some(2);
            };
            crate::user_scene_template_store::initialize_user_scene_templates_once();
            crate::home_preview::enable();
            if let Err(e) = crate::mcp_serve::run_http(PathBuf::from(path), port) {
                eprintln!("{prog} --mcp-http: {e}");
                return Some(1);
            }
            Some(0)
        }
        "--serve-web" => {
            // `--serve-web <port> [doc] [--host <addr>]`: doc optional (empty
            // document otherwise); `--host` opts in to a non-loopback bind.
            // The same parser also accepts the managed flag form
            // (`--managed --port <n> ...`) for a supervising process that
            // wants the handshake-JSON / stdin-EOF contract.
            let options = match crate::web_canvas_server::parse_serve_web_args(args) {
                Ok(options) => options,
                Err(e) => {
                    eprintln!("{prog} --serve-web: {e}");
                    return Some(2);
                }
            };
            // Select the supervisor-owned store before settings or user
            // templates can initialize the process-global default store.
            if let Some(root) = options.config_root.as_ref() {
                if let Err(e) = de_config_store::configure_user_root(root) {
                    eprintln!("{prog} --serve-web: could not initialize private config root: {e}");
                    return Some(1);
                }
            }
            // Load the standard native user-template collection before
            // accepting any request.
            crate::user_scene_template_store::initialize_user_scene_templates_once();
            if let Err(e) = crate::web_canvas_server::run_web_canvas(options) {
                eprintln!("{prog} --serve-web: {e}");
                return Some(1);
            }
            Some(0)
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "cli_modes_tests.rs"]
mod tests;
