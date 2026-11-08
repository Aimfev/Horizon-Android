//! Crash reports, laid out like vanilla's CrashReport.getFriendlyReport: a comment, the time,
//! a description, the error with its stack, then the details of the game and the machine.
//! Written to `crash-reports/crash-<yyyy-MM-dd_HH.mm.ss>-client.txt` in the game folder when
//! the main thread panics or `main` gets an error back. It leaves the machine only scrubbed, and
//! only if the player allows crash reports (`telemetry`).
use std::cell::{Cell, RefCell};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{DateTime, Local};

/// What the game is doing, set as it goes: graphics adapter, settings, packs, world.
static DETAILS: Mutex<Vec<(&'static str, String)>> = Mutex::new(Vec::new());
static REPORTED: AtomicBool = AtomicBool::new(false);

/// A running native trait-mod hook call on the current thread.
#[derive(Clone, Copy, Debug)]
pub struct NativeModCall {
    pub id: Option<&'static str>,
    pub index: usize,
    pub hook: &'static str,
}

pub struct NativeModGuard {
    previous: Option<NativeModCall>,
}

impl Drop for NativeModGuard {
    fn drop(&mut self) {
        NATIVE_MOD_CALL.with(|c| c.set(self.previous));
    }
}

pub fn enter_native_mod(call: NativeModCall) -> NativeModGuard {
    let previous = NATIVE_MOD_CALL.with(|c| {
        let prev = c.get();
        c.set(Some(call));
        prev
    });
    NativeModGuard { previous }
}

thread_local! {
    /// How many `contain` calls this thread is inside.
    static CONTAINED: Cell<u32> = const { Cell::new(0) };
    /// The message of the last panic `contain` caught on this thread, from the hook.
    static CAUGHT: RefCell<Option<String>> = const { RefCell::new(None) };
    /// The native trait-mod hook currently running on this thread, if any.
    static NATIVE_MOD_CALL: Cell<Option<NativeModCall>> = const { Cell::new(None) };
}

/// Runs `f`, and returns a panic in it as an error with the panic's message instead of letting
/// it end the game. For work that is not the game's own and whose failure the caller recovers
/// from, such as handing a shader pack's programs to the GPU driver's compiler: the panic goes
/// to the log with its stack, and no crash report is made for it.
pub fn contain<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    CONTAINED.with(|c| c.set(c.get() + 1));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    CONTAINED.with(|c| c.set(c.get() - 1));
    result.map_err(|payload| {
        CAUGHT
            .with(|c| c.borrow_mut().take())
            .unwrap_or_else(|| payload_text(&*payload))
    })
}

/// For the hook: a panic inside `contain` is logged with its stack and kept for `contain` to
/// return. False for any other panic.
fn contained_panic(info: &std::panic::PanicHookInfo) -> bool {
    if CONTAINED.with(Cell::get) == 0 {
        return false;
    }
    let message = panic_message(info);
    let stack = stack_lines(
        &std::backtrace::Backtrace::force_capture().to_string(),
        true,
    );
    log::error!(
        "Recovered from an error: {message}\n{}",
        stack
            .lines()
            .map(|l| format!("\tat {l}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    CAUGHT.with(|c| *c.borrow_mut() = Some(message));
    true
}

/// Sets one line of the report's details (replacing the last value under `key`).
pub fn set_detail(key: &'static str, value: impl Into<String>) {
    let mut details = DETAILS.lock().unwrap_or_else(|e| e.into_inner());
    let value = value.into();
    match details.iter_mut().find(|(k, _)| *k == key) {
        Some(entry) => entry.1 = value,
        None => details.push((key, value)),
    }
}

pub fn clear_detail(key: &'static str) {
    DETAILS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|(k, _)| *k != key);
}

fn details() -> Vec<(&'static str, String)> {
    DETAILS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Vanilla picks a line from a list of jokes; these only say what happened.
const COMMENTS: &[&str] = &[
    "Something went wrong.",
    "The game stopped here.",
    "This should not have happened.",
    "Details below.",
];

/// One crash, ready to be written.
pub struct Report {
    pub time: DateTime<Local>,
    pub description: String,
    /// The error's message, first line of the stack section.
    pub error: String,
    /// Where it happened and how it got there: one frame a line (`function (file:line:col)`).
    pub stack: String,
    pub thread: String,
    /// `client` or `server`, the end of the file name.
    pub side: &'static str,
    pub details: Vec<(String, String)>,
    pub log: Vec<String>,
}

/// `crash-2026-10-14_09.05.07-client.txt`
pub fn file_name(time: &DateTime<Local>, side: &str) -> String {
    format!("crash-{}-{side}.txt", time.format("%Y-%m-%d_%H.%M.%S"))
}

impl Report {
    /// The report's text.
    pub fn text(&self) -> String {
        let comment = COMMENTS[self.time.timestamp().rem_euclid(COMMENTS.len() as i64) as usize];
        let mut out = String::new();
        let _ = writeln!(out, "---- Horizon Crash Report ----");
        let _ = writeln!(out, "// {comment}\n");
        let _ = writeln!(out, "Time: {}", self.time.format("%Y-%m-%d %H:%M:%S"));
        let _ = writeln!(out, "Description: {}\n", self.description);
        let _ = writeln!(out, "{}", self.error);
        for line in self.stack.lines() {
            let _ = writeln!(out, "\tat {line}");
        }
        let _ = writeln!(out, "\n-- System Details --");
        let _ = writeln!(out, "Details:");
        let _ = writeln!(out, "\tThread: {}", self.thread);
        for (k, v) in &self.details {
            let _ = writeln!(out, "\t{k}: {v}");
        }
        if !self.log.is_empty() {
            let _ = writeln!(out, "\n-- Recent Log --");
            for line in &self.log {
                let _ = writeln!(out, "\t{line}");
            }
        }
        out
    }

    /// Writes the report into `home/crash-reports/` and returns the file's path.
    pub fn save(&self, home: &Path) -> std::io::Result<PathBuf> {
        let dir = home.join("crash-reports");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(file_name(&self.time, self.side));
        std::fs::write(&path, self.text())?;
        Ok(path)
    }
}

/// `0.1.0` or `0.1.0 (abc1234)` when built with a commit id.
pub fn version() -> String {
    let base = env!("CARGO_PKG_VERSION");
    match option_env!("HORIZON_GIT_SHA") {
        Some(sha) if !sha.is_empty() => format!("{base} ({sha})"),
        _ => base.to_owned(),
    }
}

pub fn release_tag() -> Option<String> {
    option_env!("HORIZON_GIT_SHA").map(|sha| sha.to_owned())
}

fn read(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new(program).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .filter(|s| !s.is_empty())
}

/// `linux (x86_64) version Ubuntu 24.04 LTS, kernel 6.8.0-45-generic`
pub fn operating_system() -> String {
    let arch = std::env::consts::ARCH;
    let version = if cfg!(target_os = "linux") {
        let name = read("/etc/os-release").and_then(|t| {
            t.lines()
                .find_map(|l| l.strip_prefix("PRETTY_NAME="))
                .map(|v| v.trim_matches('"').to_owned())
        });
        let kernel = read("/proc/sys/kernel/osrelease").map(|k| k.trim().to_owned());
        match (name, kernel) {
            (Some(n), Some(k)) => Some(format!("{n}, kernel {k}")),
            (n, k) => n.or(k),
        }
    } else if cfg!(target_os = "macos") {
        command_output("sw_vers", &["-productVersion"]).map(|v| format!("macOS {v}"))
    } else if cfg!(windows) {
        command_output("cmd", &["/C", "ver"])
    } else {
        None
    };
    format!(
        "{} ({arch}) version {}",
        std::env::consts::OS,
        version.unwrap_or_else(|| "unknown".into())
    )
}

/// Logical processors and the CPU's name (`12x AMD Ryzen 5 5600X ...`).
pub fn cpu() -> String {
    let threads = std::thread::available_parallelism().map_or(0, |n| n.get());
    let name = if cfg!(target_os = "linux") {
        read("/proc/cpuinfo").and_then(|t| {
            t.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split_once(':'))
                .map(|(_, v)| v.trim().to_owned())
        })
    } else if cfg!(target_os = "macos") {
        command_output("sysctl", &["-n", "machdep.cpu.brand_string"])
    } else {
        std::env::var("PROCESSOR_IDENTIFIER").ok()
    };
    format!(
        "{threads}x {}",
        name.unwrap_or_else(|| std::env::consts::ARCH.into())
    )
}

/// Total memory and what this process uses, in MiB where the system says.
fn memory() -> String {
    const MIB: u64 = 1024 * 1024;
    let kib = |text: &str, key: &str| -> Option<u64> {
        text.lines()
            .find(|l| l.starts_with(key))?
            .split_whitespace()
            .nth(1)?
            .parse()
            .ok()
    };
    let (total, used) = if cfg!(target_os = "linux") {
        (
            read("/proc/meminfo")
                .and_then(|t| kib(&t, "MemTotal:"))
                .map(|k| k * 1024),
            read("/proc/self/status")
                .and_then(|t| kib(&t, "VmRSS:"))
                .map(|k| k * 1024),
        )
    } else if cfg!(target_os = "macos") {
        (
            command_output("sysctl", &["-n", "hw.memsize"]).and_then(|v| v.parse().ok()),
            None,
        )
    } else if cfg!(windows) {
        (
            command_output(
                "powershell",
                &[
                    "-NoProfile",
                    "-Command",
                    "(Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory",
                ],
            )
            .and_then(|v| v.parse().ok()),
            None,
        )
    } else {
        (None, None)
    };
    match (total, used) {
        (Some(t), Some(u)) => format!(
            "{} MiB used by the game, {} MiB installed",
            u / MIB,
            t / MIB
        ),
        (Some(t), None) => format!("{} MiB installed", t / MIB),
        _ => "unknown".into(),
    }
}

/// The report's details: Horizon's and the machine's, then what the game set as it ran. A dedicated
/// server renders nothing: it leaves out the render settings it read from options.txt.
pub fn system_details(side: &str) -> Vec<(String, String)> {
    let mut out = vec![
        ("Horizon Version".to_owned(), version()),
        (
            "Minecraft Version".to_owned(),
            horizon_core::VANILLA_VERSION.to_owned(),
        ),
        ("Operating System".to_owned(), operating_system()),
        ("CPU".to_owned(), cpu()),
        ("Memory".to_owned(), memory()),
    ];
    out.extend(
        details()
            .into_iter()
            .filter(|(k, _)| side != "server" || *k != "Render Settings")
            .map(|(k, v)| (k.to_owned(), v)),
    );
    out
}

/// The thread's name as vanilla's reports say it: the main thread is the client's "Render thread"
/// and the dedicated server's "Server thread".
fn thread_name(side: &str) -> String {
    let current = std::thread::current();
    match current.name() {
        Some("main") | None if side == "server" => "Server thread".into(),
        Some("main") | None => "Render thread".into(),
        Some(name) => name.into(),
    }
}

/// What starts the stderr line naming a saved report, the path follows: the Launch API's
/// promise to launchers (docs/api/launch.md), so it never changes within a line of that API.
pub const SAVED_LINE: &str = "Horizon: crash report saved to ";

/// Writes the report and tells where it is; the log gets the error too.
fn deliver(home: &Path, report: Report) -> Option<PathBuf> {
    if REPORTED.swap(true, Ordering::SeqCst) {
        return None;
    }
    // The panic is on stderr already: the stack goes to the log only.
    crate::logging::write_only(
        log::Level::Error,
        &format!(
            "Horizon crashed: {}\n{}",
            report.error,
            report
                .stack
                .lines()
                .map(|l| format!("\tat {l}"))
                .collect::<Vec<_>>()
                .join("\n")
        ),
    );
    crate::logging::flush();
    crate::telemetry::keep_crash(home, &report);
    match report.save(home) {
        Ok(path) => {
            eprintln!("{SAVED_LINE}{}", path.display());
            Some(path)
        }
        Err(e) => {
            eprintln!("Horizon: could not save a crash report: {e}");
            eprintln!("{}", report.text());
            None
        }
    }
}

/// The report for a hung server tick, written by the watchdog thread
/// (`ServerWatchdog`): the stuck thread's stack is not available to Rust, so
/// the report carries none.
pub fn report_watchdog(home: &Path, side: &'static str, message: String) -> Option<PathBuf> {
    deliver(
        home,
        Report {
            time: Local::now(),
            description: "Watching Server".into(),
            error: format!("{message} (Watching Server)"),
            stack: String::new(),
            thread: thread_name(side),
            side,
            details: system_details(side),
            log: crate::logging::recent_lines(),
        },
    )
}

fn payload_text(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "Box<dyn Any>".into())
}

fn panic_message(info: &std::panic::PanicHookInfo) -> String {
    let text = payload_text(info.payload());
    let mut msg = match info.location() {
        Some(at) => format!("panicked at {at}:\n{text}"),
        None => format!("panicked:\n{text}"),
    };
    if let Some(call) = NATIVE_MOD_CALL.with(Cell::get) {
        match call.id {
            Some(id) => msg.push_str(&format!("\nin native mod {id}, hook {}", call.hook)),
            None => msg.push_str(&format!("\nin native mod #{}, hook {}", call.index, call.hook)),
        }
    }
    msg
}

/// A `std::backtrace` as one line a frame, `function (file:line:col)`. Captured in a panic hook
/// (`from_panic`), it starts at the frame that panicked: the hook's and the panic machinery's
/// frames go. Paths inside the workspace lose the build machine's prefix.
pub fn stack_lines(backtrace: &str, from_panic: bool) -> String {
    let mut frames: Vec<(String, Option<String>)> = Vec::new();
    for line in backtrace.lines() {
        let line = line.trim();
        if let Some(at) = line.strip_prefix("at ") {
            if let Some(frame) = frames.last_mut() {
                frame.1 = Some(short_path(at));
            }
        } else if let Some((n, name)) = line.split_once(": ")
            && n.chars().all(|c| c.is_ascii_digit())
        {
            frames.push((name.to_owned(), None));
        }
    }
    let machinery = |name: &str| {
        [
            "std::panicking::",
            "core::panicking::",
            "std::sys::backtrace::",
            "__rustc::",
        ]
        .iter()
        .any(|p| name.starts_with(p))
            || name.contains("rust_begin_unwind")
    };
    // The hook's frames, then the panic machinery's, then the code that panicked.
    let top = frames.iter().take(30).position(|(name, _)| machinery(name));
    let first = match top.filter(|_| from_panic) {
        Some(start) => {
            start
                + frames[start..]
                    .iter()
                    .take_while(|(name, _)| machinery(name))
                    .count()
        }
        None => 0,
    };
    // Some platforms keep std's own capture frames (`<std::backtrace::Backtrace>::create`).
    let first = first
        + frames[first..]
            .iter()
            .take_while(|(name, _)| name.trim_start_matches('<').starts_with("std::backtrace"))
            .count();
    frames[first..]
        .iter()
        .map(|(name, at)| match at {
            Some(at) => format!("{name} ({at})\n"),
            None => format!("{name}\n"),
        })
        .collect()
}

/// A source path without the build machine's folders: relative to the workspace, or from the
/// crate's folder for dependencies (`winit-0.30.13/src/event_loop.rs:642:56`).
fn short_path(path: &str) -> String {
    // Recognize source structure rather than embedding the builder's private
    // CARGO_MANIFEST_DIR in every distributed binary. Normalize Windows too.
    let path = path.replace('\\', "/");
    if let Some(i) = path.rfind("/crates/") {
        return path[i + 1..].to_owned();
    }
    if let Some(i) = path.find("/registry/src/") {
        let rest = &path[i + "/registry/src/".len()..];
        if let Some((_index, krate)) = rest.split_once('/') {
            return krate.to_owned();
        }
    }
    path
}

/// A report for a panic, with the stack captured now (RUST_BACKTRACE or not).
pub fn panic_report(info: &std::panic::PanicHookInfo, side: &'static str) -> Report {
    Report {
        time: Local::now(),
        description: "Unexpected error".into(),
        error: panic_message(info),
        stack: stack_lines(
            &std::backtrace::Backtrace::force_capture().to_string(),
            true,
        ),
        thread: thread_name(side),
        side,
        details: system_details(side),
        log: crate::logging::recent_lines(),
    }
}

/// Whether `name` is one of the streaming pool's threads, vanilla's `Util` worker threads.
fn is_worker_thread(name: Option<&str>) -> bool {
    name.is_some_and(|n| n.starts_with("horizon-worker-"))
}

/// `Util.onThreadException`: a panic on a worker thread is made a report titled "Exception on
/// worker thread", with the thread's name, as `BlockableEventLoop.relayDelayCrash` hands it to
/// the main thread.
pub fn worker_panic_report(info: &std::panic::PanicHookInfo, side: &'static str) -> Report {
    Report {
        description: "Exception on worker thread".into(),
        ..panic_report(info, side)
    }
}

/// Reports panics: on the main thread a crash report (the panic then ends the game, exit code
/// 101); on a worker thread (`Util.onThreadException`) a crash report too, and the game ends
/// with it; on other threads the error goes to the log, as vanilla's uncaught exception handler
/// does. The default hook still prints the panic. A panic inside `contain` is only logged.
pub fn install(home: PathBuf, side: &'static str) {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if contained_panic(info) {
            return;
        }
        default(info);
        let main = std::thread::current().name() == Some("main");
        if main {
            deliver(&home, panic_report(info, side));
        } else if is_worker_thread(std::thread::current().name()) {
            deliver(&home, worker_panic_report(info, side));
            // The crash ends the game, as the relayed report does on vanilla's main thread.
            std::process::exit(101);
        } else {
            log::error!(
                "Caught previously unhandled exception in thread {}: {}",
                thread_name(side),
                panic_message(info)
            );
        }
    }));
}

/// Vanilla's debug crash (`key.debug.crash` held for ten seconds): the error `main` gets back,
/// with the stack where it was raised.
#[derive(Debug)]
pub struct DebugCrash {
    stack: String,
}

impl DebugCrash {
    pub const MESSAGE: &str = "Manually triggered debug crash";
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            stack: stack_lines(
                &std::backtrace::Backtrace::force_capture().to_string(),
                false,
            ),
        }
    }
}

impl std::fmt::Display for DebugCrash {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(Self::MESSAGE)
    }
}

impl std::error::Error for DebugCrash {}

/// When `err` is a [`DebugCrash`], writes its crash report (`"Manually triggered debug crash"`,
/// as vanilla's `Minecraft.run` does when the debug crash throws) and returns true.
pub fn report_if_debug_crash(
    home: &Path,
    side: &'static str,
    err: &(dyn std::error::Error + 'static),
) -> bool {
    let Some(crash) = err.downcast_ref::<DebugCrash>() else {
        return false;
    };
    deliver(
        home,
        Report {
            time: Local::now(),
            description: DebugCrash::MESSAGE.into(),
            error: DebugCrash::MESSAGE.into(),
            stack: crash.stack.clone(),
            thread: thread_name(side),
            side,
            details: system_details(side),
            log: crate::logging::recent_lines(),
        },
    );
    true
}

/// A report for an error `main` got back: the game ends with it.
pub fn report_error(
    home: &Path,
    side: &'static str,
    error: &(dyn std::error::Error + 'static),
) -> Option<PathBuf> {
    if let Some(debug) = error.downcast_ref::<DebugCrash>() {
        return deliver(
            home,
            Report {
                time: Local::now(),
                description: DebugCrash::MESSAGE.into(),
                error: DebugCrash::MESSAGE.into(),
                stack: debug.stack.clone(),
                thread: thread_name(side),
                side,
                details: system_details(side),
                log: crate::logging::recent_lines(),
            },
        );
    }
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(e) = source {
        let _ = write!(message, "\nCaused by: {e}");
        source = e.source();
    }
    deliver(
        home,
        Report {
            time: Local::now(),
            description: "Fatal error".into(),
            error: message,
            stack: String::new(),
            thread: thread_name(side),
            side,
            details: system_details(side),
            log: crate::logging::recent_lines(),
        },
    )
}

/// Called every frame: panics once `HORIZON_DEBUG_CRASH` asks (development builds only).
pub fn debug_crash_tick() {
    static AT: std::sync::OnceLock<Option<(std::time::Instant, std::time::Duration)>> =
        std::sync::OnceLock::new();
    if let Some((first, after)) =
        AT.get_or_init(|| debug_crash_after().map(|d| (std::time::Instant::now(), d)))
        && first.elapsed() >= *after
    {
        panic!("Manually triggered debug crash");
    }
}

/// Development builds crash on purpose when `HORIZON_DEBUG_CRASH` is set (vanilla has F3+C held
/// for ten seconds): the number is seconds after the first frame (default 5).
pub fn debug_crash_after() -> Option<std::time::Duration> {
    if !cfg!(debug_assertions) {
        return None;
    }
    let value = std::env::var("HORIZON_DEBUG_CRASH").ok()?;
    let seconds = value.parse::<f64>().unwrap_or(5.0).max(0.0);
    Some(std::time::Duration::from_secs_f64(seconds))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample() -> Report {
        Report {
            time: Local.with_ymd_and_hms(2026, 10, 14, 9, 5, 7).unwrap(),
            description: "Unexpected error".into(),
            error: "panicked at crates/horizon-client/src/game.rs:42:5:\nboom".into(),
            stack: "horizon_client::game::tick (crates/horizon-client/src/game.rs:42:5)\n\
                    horizon_client::main (crates/horizon-client/src/main.rs:10:5)\n"
                .into(),
            thread: "Render thread".into(),
            side: "client",
            details: vec![
                ("Horizon Version".into(), "0.1.0 (abc1234)".into()),
                ("Minecraft Version".into(), "26.1.1".into()),
                ("Graphics Adapter".into(), "AMD Radeon RX 6700 XT".into()),
            ],
            log: vec!["12:00:01 [INFO] Started".into()],
        }
    }

    #[test]
    fn report_has_the_sections_and_saves_under_crash_reports() {
        let r = sample();
        assert_eq!(
            file_name(&r.time, r.side),
            "crash-2026-10-14_09.05.07-client.txt"
        );
        let text = r.text();
        assert!(text.starts_with("---- Horizon Crash Report ----\n// "));
        assert!(text.contains("Time: 2026-10-14 09:05:07\nDescription: Unexpected error\n"));
        assert!(text.contains(
            "panicked at crates/horizon-client/src/game.rs:42:5:\nboom\n\
             \tat horizon_client::game::tick (crates/horizon-client/src/game.rs:42:5)\n"
        ));
        assert!(text.contains(
            "-- System Details --\nDetails:\n\
             \tThread: Render thread\n\
             \tHorizon Version: 0.1.0 (abc1234)\n\
             \tMinecraft Version: 26.1.1\n\
             \tGraphics Adapter: AMD Radeon RX 6700 XT\n"
        ));
        assert!(text.contains("-- Recent Log --\n\t12:00:01 [INFO] Started\n"));

        let dir = std::env::temp_dir().join(format!("horizon-crash-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let saved = r.save(&dir).unwrap();
        assert_eq!(saved, dir.join("crash-reports/crash-2026-10-14_09.05.07-client.txt"));
        assert_eq!(std::fs::read_to_string(&saved).unwrap(), text);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stack_lines_drop_panic_machinery_and_trim_paths() {
        let raw = "   0: horizon_client_render::crash::panic_report\n\
                   \x20            at /home/dev/horizon/crates/horizon-client-render/src/crash.rs:200:10\n\
                      1: core::panicking::panic_fmt\n\
                   \x20            at /rustc/abc/library/core/src/panicking.rs:75:14\n\
                      2: horizon_client::game::tick\n\
                   \x20            at /home/dev/horizon/crates/horizon-client/src/game.rs:42:5\n\
                      3: winit::event_loop::EventLoop::run\n\
                   \x20            at /home/dev/.cargo/registry/src/index.crates.io-6f17d22bba15001f/winit-0.30.13/src/event_loop.rs:642:56\n";
        assert_eq!(
            stack_lines(raw, true),
            "horizon_client::game::tick (crates/horizon-client/src/game.rs:42:5)\n\
             winit::event_loop::EventLoop::run (winit-0.30.13/src/event_loop.rs:642:56)\n"
        );
        // Windows backslashes normalize to forward slashes too.
        assert_eq!(
            short_path(r"C:\Users\runner\work\horizon\crates\horizon-client\src\main.rs:1:1"),
            "crates/horizon-client/src/main.rs:1:1"
        );
    }

    #[test]
    fn details_replace_in_place_and_server_skips_render_settings() {
        set_detail("Render Settings", "12 chunks");
        set_detail("World", "New World");
        set_detail("Render Settings", "16 chunks");
        let client = system_details("client");
        assert_eq!(
            client
                .iter()
                .filter(|(k, _)| k == "Render Settings")
                .map(|(_, v)| v.as_str())
                .collect::<Vec<_>>(),
            ["16 chunks"]
        );
        let server = system_details("server");
        assert!(!server.iter().any(|(k, _)| k == "Render Settings"));
        assert!(server.iter().any(|(k, v)| k == "World" && v == "New World"));
        clear_detail("World");
        clear_detail("Render Settings");
    }

    #[test]
    fn debug_crash_carries_its_stack_past_its_own_new() {
        let err: Box<dyn std::error::Error> = Box::new(DebugCrash::new());
        assert_eq!(err.to_string(), DebugCrash::MESSAGE);
        let crash = err.downcast_ref::<DebugCrash>().unwrap();
        assert!(!crash.stack.contains("DebugCrash::new"));
        assert!(crash.stack.contains("debug_crash_carries_its_stack"));
    }

    #[test]
    fn worker_threads_are_recognized_by_prefix() {
        assert!(is_worker_thread(Some("horizon-worker-0")));
        assert!(is_worker_thread(Some("horizon-worker-7")));
        assert!(!is_worker_thread(Some("main")));
        assert!(!is_worker_thread(Some("horizon-io")));
        assert!(!is_worker_thread(None));
    }

    #[test]
    fn contain_catches_a_panic_and_returns_its_message() {
        assert_eq!(contain(|| 2 + 2), Ok(4));
        let err = contain(|| -> i32 { panic!("bad shader") }).unwrap_err();
        assert!(err.contains("bad shader"), "{err}");
        // After `contain` returns, this thread is no longer inside it.
        assert_eq!(CONTAINED.with(Cell::get), 0);
        // Nested `contain` works too.
        let outer = contain(|| {
            let inner = contain(|| -> i32 { panic!("inner") });
            assert!(inner.is_err());
            7
        });
        assert_eq!(outer, Ok(7));
        assert_eq!(CONTAINED.with(Cell::get), 0);
    }
}
