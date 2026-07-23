use crate::atomic::{atomic_write, read_file_optional};
use crate::paths;
use crate::types::{McpError, McpServerEntry};
use crate::winshim::wrap_for_windows;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Max lines retained per server's in-memory log ring buffer.
const LOG_BUFFER_LINES: usize = 500;

/// Canonical key shape: `"{app}::{name}"`.
pub type ServerKey = String;

/// Snapshot of a running server, safe to send to the frontend.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningServer {
    pub name: String,
    pub app: String,
    pub pid: u32,
    pub command: String,
    pub args: Vec<String>,
    pub started_at: u64,
}

/// Payload emitted on the `mcp-server-exited` Tauri event whenever a
/// runner-launched child terminates on its own (i.e. not via `Stop`).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerExitEvent {
    pub name: String,
    pub app: String,
    pub pid: u32,
    pub code: i32,
}

/// Frontend-facing auto-run shape (so the TS side never sees Rust
/// private types).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoRunKeyDto {
    pub name: String,
    pub app: String,
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct AutoRunFile {
    #[serde(default)]
    auto_run: Vec<AutoRunKeyInner>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
struct AutoRunKeyInner {
    name: String,
    app: String,
}

/// Holds the OS child handle (shared with the waiter thread via `Arc`),
/// a shared ring buffer the stdout/stderr reader threads append to, and
/// the last observed exit status (populated by the waiter thread).
struct ServerHandle {
    info: RunningServer,
    child: Arc<Mutex<Option<Child>>>,
    log: Arc<Mutex<VecDeque<String>>>,
    last_exit: Mutex<Option<i32>>,
}

impl ServerHandle {
    /// True iff the OS process is still around AND we haven't observed
    /// its exit.
    fn alive(&self) -> bool {
        let mut guard = match self.child.lock() {
            Ok(g) => g,
            Err(_) => return false,
        };
        match guard.as_mut() {
            Some(child) => matches!(child.try_wait(), Ok(None)),
            None => false,
        }
    }
}

/// Tauri-managed state.
pub struct RunnerState {
    inner: Mutex<HashMap<ServerKey, ServerHandle>>,
}

impl Default for RunnerState {
    fn default() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }
}

fn push_log(log: &Arc<Mutex<VecDeque<String>>>, line: String) {
    if let Ok(mut buf) = log.lock() {
        if buf.len() == LOG_BUFFER_LINES {
            buf.pop_front();
        }
        buf.push_back(line);
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn load_auto_run() -> AutoRunFile {
    let path = paths::runner_state_path();
    match read_file_optional(&path) {
        Ok(Some(content)) => serde_json::from_str(&content).unwrap_or_default(),
        _ => AutoRunFile::default(),
    }
}

fn save_auto_run(file: &AutoRunFile) -> Result<(), McpError> {
    let content = serde_json::to_string_pretty(file).map_err(McpError::from)?;
    atomic_write(&paths::runner_state_path(), &content)
}

impl RunnerState {
    pub fn key(name: &str, app: &str) -> ServerKey {
        format!("{app}::{name}")
    }

    /// Spawn `entry`'s stdio command as a detached-from-console subprocess
    /// and register it. Spawns reader threads for stdout/stderr plus a
    /// watcher thread that logs and emits an event on natural exit.
    pub fn start_with_app(
        &self,
        app: Option<&AppHandle>,
        entry: &McpServerEntry,
    ) -> Result<RunningServer, McpError> {
        if entry.transport != "stdio" {
            return Err(McpError::RestartFailed(format!(
                "Server '{}' uses '{}' transport -- only stdio servers can be run from MCP Switch (URL-based servers talk to an existing endpoint, nothing to spawn locally).",
                entry.name, entry.transport
            )));
        }

        let command_str = entry
            .command
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                McpError::RestartFailed(format!(
                    "Server '{}' has no command configured",
                    entry.name
                ))
            })?
            .to_string();

        let args_vec = entry.args.clone().unwrap_or_default();
        let (cmd, wrapped) = wrap_for_windows(&command_str, Some(args_vec.clone()));
        let final_args = wrapped.unwrap_or_default();

        let mut cmd_builder = Command::new(&cmd);
        cmd_builder.args(&final_args);
        if let Some(env) = &entry.env {
            for (k, v) in env {
                cmd_builder.env(k, v);
            }
        }
        cmd_builder.stdin(Stdio::null());
        cmd_builder.stdout(Stdio::piped());
        cmd_builder.stderr(Stdio::piped());

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd_builder.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = cmd_builder.spawn().map_err(|e| {
            McpError::RestartFailed(format!(
                "Could not start `{}` for `{}`: {e}",
                entry.name, entry.app
            ))
        })?;

        let pid = child.id();
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| McpError::RestartFailed("Could not capture stdout".into()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| McpError::RestartFailed("Could not capture stderr".into()))?;

        let info = RunningServer {
            name: entry.name.clone(),
            app: entry.app.clone(),
            pid,
            command: cmd,
            args: final_args,
            started_at: now_unix(),
        };

        let log: Arc<Mutex<VecDeque<String>>> =
            Arc::new(Mutex::new(VecDeque::with_capacity(LOG_BUFFER_LINES)));
        let key = Self::key(&entry.name, &entry.app);
        let child_arc = Arc::new(Mutex::new(Some(child)));

        // stdout reader thread
        {
            let log = log.clone();
            let _ = thread::Builder::new()
                .name(format!("mcp-out::{key}"))
                .spawn(move || {
                    let reader = BufReader::new(stdout);
                    for line in reader.lines() {
                        match line {
                            Ok(text) => push_log(&log, format!("[out] {text}")),
                            Err(_) => break,
                        }
                    }
                });
        }

        // stderr reader thread
        {
            let log = log.clone();
            let _ = thread::Builder::new()
                .name(format!("mcp-err::{key}"))
                .spawn(move || {
                    let reader = BufReader::new(stderr);
                    for line in reader.lines() {
                        match line {
                            Ok(text) => push_log(&log, format!("[err] {text}")),
                            Err(_) => break,
                        }
                    }
                });
        }

        // Combine info+handle
        let mut map = self.inner.lock().unwrap();
        if let Some(existing) = map.get(&key) {
            if existing.alive() {
                // Roll back: kill the about-to-be-leaked child.
                if let Some(mut c) = child_arc.lock().unwrap().take() {
                    let _ = c.kill();
                    let _ = c.wait();
                }
                return Err(McpError::RestartFailed(format!(
                    "{} for {} is already running (pid {})",
                    entry.name, entry.app, existing.info.pid
                )));
            }
        }

        let handle = ServerHandle {
            info: info.clone(),
            child: child_arc.clone(),
            log: log.clone(),
            last_exit: Mutex::new(None),
        };

        map.insert(key, handle);

        // Spawn the watcher AFTER the handle is in the map so its update
        // of `last_exit` is observable.
        if let Some(app_handle) = app {
            Self::spawn_watcher(
                app_handle,
                entry.name.clone(),
                entry.app.clone(),
                pid,
                child_arc,
                log,
            );
        }
        // Drop the map lock before returning so the caller can re-acquire.
        drop(map);

        Ok(info)
    }

    /// Backwards-compatible entry point for callers without an AppHandle
    /// (notably unit tests). No watcher is spawned, so no crash event.
    #[allow(dead_code)] // used only by unit tests; production goes via start_with_app
    pub fn start(&self, entry: &McpServerEntry) -> Result<RunningServer, McpError> {
        self.start_with_app(None, entry)
    }

    /// Stop a running server. Returns true iff a live process was actually
    /// killed (vs. already dead, or never started).
    pub fn stop(&self, name: &str, app: &str) -> Result<bool, McpError> {
        let key = Self::key(name, app);
        let mut map = self.inner.lock().unwrap();
        let Some(handle) = map.remove(&key) else {
            return Ok(false);
        };
        let killed = {
            let mut guard = handle.child.lock().unwrap();
            match guard.as_mut() {
                Some(child) => match child.try_wait() {
                    Ok(None) => {
                        let _ = child.kill();
                        let _ = child.wait();
                        true
                    }
                    _ => false,
                },
                None => false,
            }
        };
        Ok(killed)
    }

    /// Stop every child currently tracked. Used by the tray `Quit` path
    /// so we don't leave orphan MCP servers running across restarts.
    pub fn stop_all(&self) -> Vec<(String, String, bool)> {
        let handles: Vec<(String, String, ServerHandle)> = {
            let mut map = match self.inner.lock() {
                Ok(m) => m,
                Err(_) => return Vec::new(),
            };
            map.drain()
                .map(|(k, h)| {
                    let mut parts = k.splitn(2, "::");
                    let app = parts.next().unwrap_or("").to_string();
                    let name = parts.next().unwrap_or("").to_string();
                    (name, app, h)
                })
                .collect()
        };
        handles
            .into_iter()
            .map(|(name, app, h)| {
                let killed = {
                    let mut guard = h.child.lock().unwrap();
                    match guard.as_mut() {
                        Some(child) => match child.try_wait() {
                            Ok(None) => {
                                let _ = child.kill();
                                let _ = child.wait();
                                true
                            }
                            _ => false,
                        },
                        None => false,
                    }
                };
                (name, app, killed)
            })
            .collect()
    }

    /// Drop entries whose process has already exited. Called by `list`
    /// so the frontend never sees stale rows.
    fn reap_dead(&self) {
        let mut map = match self.inner.lock() {
            Ok(m) => m,
            Err(_) => return,
        };
        map.retain(|_, h| h.alive());
    }

    /// Snapshot of every currently tracked server.
    pub fn list(&self) -> Vec<RunningServer> {
        self.reap_dead();
        let map = match self.inner.lock() {
            Ok(m) => m,
            Err(_) => return Vec::new(),
        };
        map.values().map(|h| h.info.clone()).collect()
    }

    /// Last `tail` lines of captured stdout/stderr.
    pub fn read_log(&self, name: &str, app: &str, tail: usize) -> Vec<String> {
        let key = Self::key(name, app);
        let map = match self.inner.lock() {
            Ok(m) => m,
            Err(_) => return Vec::new(),
        };
        let Some(handle) = map.get(&key) else {
            return Vec::new();
        };
        let buf = match handle.log.lock() {
            Ok(b) => b,
            Err(_) => return Vec::new(),
        };
        let n = tail.min(buf.len());
        let mut out: Vec<String> = buf.iter().rev().take(n).cloned().collect();
        out.reverse();
        out
    }

    pub fn get_auto_run(&self) -> Vec<AutoRunKeyDto> {
        let mut file = load_auto_run();
        file.auto_run.sort_by(|a, b| a.app.cmp(&b.app).then_with(|| a.name.cmp(&b.name)));
        file.auto_run
            .into_iter()
            .map(|k| AutoRunKeyDto { name: k.name, app: k.app })
            .collect()
    }

    pub fn set_auto_run(&self, name: &str, app: &str, enabled: bool) -> Result<bool, McpError> {
        let mut file = load_auto_run();
        let mut changed = false;
        let needle = AutoRunKeyInner {
            name: name.to_string(),
            app: app.to_string(),
        };
        let had = file.auto_run.iter().any(|k| k == &needle);
        if enabled && !had {
            file.auto_run.push(needle);
            changed = true;
        } else if !enabled && had {
            file.auto_run.retain(|k| k != &needle);
            changed = true;
        }
        if changed {
            save_auto_run(&file)?;
        }
        Ok(changed)
    }

    /// For every persisted auto-run entry that has a matching enabled
    /// server in `entries`, spawn it. Best-effort: failures are logged and
    /// the loop continues.
    pub fn start_auto_run(&self, app: &AppHandle, entries: &[McpServerEntry]) {
        let file = load_auto_run();
        for key in file.auto_run {
            let Some(entry) = entries
                .iter()
                .find(|e| e.name == key.name && e.app == key.app)
            else {
                eprintln!(
                    "auto-run: no live enabled server found for `{}::{}`; skipping",
                    key.app, key.name
                );
                continue;
            };
            if let Err(e) = self.start_with_app(Some(app), entry) {
                eprintln!(
                    "auto-run: failed to start `{}::{}`: {e}",
                    key.app, key.name
                );
            }
        }
    }

    /// Wait until the child exits, log the code, emit a Tauri event so
    /// the UI can surface a crash toast, and update `last_exit` on the
    /// handle for callers that read it later.
    fn spawn_watcher(
        app: &AppHandle,
        name: String,
        app_id: String,
        pid: u32,
        child_arc: Arc<Mutex<Option<Child>>>,
        log: Arc<Mutex<VecDeque<String>>>,
    ) {
        let app = app.clone();
        let key = Self::key(&name, &app_id);
        thread::Builder::new()
            .name(format!("mcp-wait::{app_id}::{name}"))
            .spawn(move || {
                let exit_code = {
                    let mut g = match child_arc.lock() {
                        Ok(g) => g,
                        Err(_) => return,
                    };
                    match g.as_mut() {
                        Some(c) => c.wait().ok().and_then(|s| s.code()).unwrap_or(-1),
                        None => return, // stop() drained it; not a crash
                    }
                };

                let line = format!("[runner] child exited (code={exit_code})");
                push_log(&log, line);

                // Update last_exit on the live handle (if still present).
                if let Some(state) = app.try_state::<RunnerState>() {
                    if let Ok(map) = state.inner.lock() {
                        if let Some(h) = map.get(&key) {
                            if let Ok(mut exit) = h.last_exit.lock() {
                                *exit = Some(exit_code);
                            }
                        }
                    }
                }

                let _ = app.emit(
                    "mcp-server-exited",
                    ServerExitEvent {
                        name: name.clone(),
                        app: app_id.clone(),
                        pid,
                        code: exit_code,
                    },
                );
            })
            .ok();
    }
}

// Silence "unused import" on platforms that never read the Path.
#[allow(dead_code)]
fn _unused_path(_: &Path) {}
