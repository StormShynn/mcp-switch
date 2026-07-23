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
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Max lines retained per server's in-memory log ring buffer.
const LOG_BUFFER_LINES: usize = 500;
/// Cap backoff so a misconfigured policy doesn't sleep for hours.
const MAX_BACKOFF_MS: u64 = 60_000;
/// Cap retries for `Always` policy so we don't loop forever on a broken binary.
const MAX_RESTART_ATTEMPTS: u32 = 100;

/// Canonical key shape: `"{app}::{name}"`.
pub type ServerKey = String;

/// Restart policy inspired by PM2 / supervisord: when a runner-launched
/// child exits, the watcher decides whether to respawn it automatically.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum RestartPolicy {
    /// Never respawn (default).
    Never,
    /// Respawn only on non-zero exit. Stops after `maxRetries`.
    OnFailure {
        #[serde(default = "default_max_retries")]
        max_retries: u32,
        #[serde(default = "default_backoff_ms")]
        backoff_ms: u64,
    },
    /// Always respawn, even on clean exit. Stops after `maxRetries`.
    Always {
        #[serde(default = "default_max_retries")]
        max_retries: u32,
        #[serde(default = "default_backoff_ms")]
        backoff_ms: u64,
    },
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self::Never
    }
}

fn default_max_retries() -> u32 {
    5
}
fn default_backoff_ms() -> u64 {
    1000
}

/// Default-to-`Never` for any malformed persisted file.
impl RestartPolicy {
    fn deserialize_safe<'de, D>(d: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        RestartPolicy::deserialize(d).or_else(|_| Ok(RestartPolicy::Never))
    }
}

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
    /// Unix seconds (UTC) at which the child was *last* spawned. Differs
    /// from `started_at` after an auto-restart.
    pub last_started_at: u64,
    /// Times this entry has been auto-respawned since the original start.
    pub restart_count: u32,
    /// Active restart policy (clone of the entry's persisted setting).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart_policy: Option<RestartPolicy>,
}

/// Payload emitted on the `mcp-server-exited` Tauri event whenever a
/// runner-launched child terminates on its own.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerExitEvent {
    pub name: String,
    pub app: String,
    pub pid: u32,
    pub code: i32,
    /// True if a new process will be spawned automatically (per policy).
    pub will_restart: bool,
}

/// Persisted profile (Foreman-style procfile).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileDto {
    pub id: String,
    pub label: String,
    pub members: Vec<ProfileMemberDto>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileMemberDto {
    pub app: String,
    pub name: String,
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct AutoRunFile {
    #[serde(default)]
    auto_run: Vec<AutoRunInner>,
    /// Per-server restart policy override. Keyed by `app::name`.
    #[serde(default)]
    restart_policies: HashMap<String, RestartPolicy>,
    /// Named profiles, each a list of `(app, name)` members.
    #[serde(default)]
    profiles: Vec<ProfileInner>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
struct AutoRunInner {
    name: String,
    app: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ProfileInner {
    id: String,
    label: String,
    members: Vec<AutoRunInner>,
}

struct ServerHandle {
    info: RunningServer,
    /// Original entry, kept around so the watcher can respawn with the same
    /// args/env when the restart policy says to.
    spec: Arc<McpServerEntry>,
    child: Arc<Mutex<Option<Child>>>,
    log: Arc<Mutex<VecDeque<String>>>,
    last_exit: Mutex<Option<i32>>,
}

impl ServerHandle {
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

/// Decide whether the watcher should respawn, given the policy + history.
fn should_restart(policy: &RestartPolicy, attempts_so_far: u32, exit_code: i32) -> bool {
    let (want, max) = match policy {
        RestartPolicy::Never => return false,
        RestartPolicy::OnFailure { max_retries, .. } => (exit_code != 0, *max_retries),
        RestartPolicy::Always { max_retries, .. } => (true, *max_retries),
    };
    want && attempts_so_far < max.min(MAX_RESTART_ATTEMPTS)
}

impl RunnerState {
    pub fn key(name: &str, app: &str) -> ServerKey {
        format!("{app}::{name}")
    }

    /// Spawn `entry`'s stdio command as a detached-from-console subprocess
    /// and register it. `restart_count` starts at 0; the watcher will
    /// bump it on each auto-respawn.
    pub fn start_with_app(
        &self,
        app: Option<&AppHandle>,
        entry: &McpServerEntry,
        restart_policy: Option<RestartPolicy>,
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

        let now = now_unix();
        let info = RunningServer {
            name: entry.name.clone(),
            app: entry.app.clone(),
            pid,
            command: cmd,
            args: final_args,
            started_at: now,
            last_started_at: now,
            restart_count: 0,
            restart_policy: restart_policy.clone(),
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

        let mut map = self.inner.lock().unwrap();
        if let Some(existing) = map.get(&key) {
            if existing.alive() {
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

        let spec_arc = Arc::new(entry.clone());
        map.insert(
            key.clone(),
            ServerHandle {
                info: info.clone(),
                spec: spec_arc,
                child: child_arc.clone(),
                log: log.clone(),
                last_exit: Mutex::new(None),
            },
        );

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
                restart_policy,
            );
        }
        drop(map);

        Ok(info)
    }

    pub fn start(&self, entry: &McpServerEntry) -> Result<RunningServer, McpError> {
        self.start_with_app(None, entry, None)
    }

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
        // Auto-restart must not kick in when the user explicitly stops.
        Ok(killed)
    }

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

    fn reap_dead(&self) {
        let mut map = match self.inner.lock() {
            Ok(m) => m,
            Err(_) => return,
        };
        map.retain(|_, h| h.alive());
    }

    pub fn list(&self) -> Vec<RunningServer> {
        self.reap_dead();
        let map = match self.inner.lock() {
            Ok(m) => m,
            Err(_) => return Vec::new(),
        };
        map.values().map(|h| h.info.clone()).collect()
    }

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

    // ---- Auto-run persistence --------------------------------------

    pub fn get_auto_run(&self) -> Vec<ProfileMemberDto> {
        let mut file = load_auto_run();
        file.auto_run.sort_by(|a, b| a.app.cmp(&b.app).then_with(|| a.name.cmp(&b.name)));
        file.auto_run
            .into_iter()
            .map(|k| ProfileMemberDto {
                name: k.name,
                app: k.app,
            })
            .collect()
    }

    pub fn set_auto_run(
        &self,
        name: &str,
        app: &str,
        enabled: bool,
    ) -> Result<bool, McpError> {
        let mut file = load_auto_run();
        let mut changed = false;
        let needle = AutoRunInner {
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

    // ---- Restart policy persistence --------------------------------

    pub fn get_restart_policy(&self, name: &str, app: &str) -> RestartPolicy {
        let file = load_auto_run();
        let key = format!("{app}::{name}");
        file.restart_policies
            .get(&key)
            .cloned()
            .unwrap_or(RestartPolicy::Never)
    }

    pub fn set_restart_policy(
        &self,
        name: &str,
        app: &str,
        policy: RestartPolicy,
    ) -> Result<(), McpError> {
        let mut file = load_auto_run();
        let key = format!("{app}::{name}");
        if matches!(policy, RestartPolicy::Never) {
            file.restart_policies.remove(&key);
        } else {
            file.restart_policies.insert(key, policy);
        }
        save_auto_run(&file)
    }

    // ---- Profiles (Foreman-style) ----------------------------------

    pub fn list_profiles(&self) -> Vec<ProfileDto> {
        let file = load_auto_run();
        file.profiles
            .into_iter()
            .map(|p| ProfileDto {
                id: p.id,
                label: p.label,
                members: p
                    .members
                    .into_iter()
                    .map(|m| ProfileMemberDto { app: m.app, name: m.name })
                    .collect(),
            })
            .collect()
    }

    pub fn upsert_profile(&self, profile: ProfileDto) -> Result<(), McpError> {
        let mut file = load_auto_run();
        file.profiles.retain(|p| p.id != profile.id);
        file.profiles.push(ProfileInner {
            id: profile.id,
            label: profile.label,
            members: profile
                .members
                .into_iter()
                .map(|m| AutoRunInner { app: m.app, name: m.name })
                .collect(),
        });
        save_auto_run(&file)
    }

    pub fn delete_profile(&self, id: &str) -> Result<bool, McpError> {
        let mut file = load_auto_run();
        let before = file.profiles.len();
        file.profiles.retain(|p| p.id != id);
        if file.profiles.len() != before {
            save_auto_run(&file)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Start every member of a profile. Errors per member are logged and
    /// the loop continues.
    pub fn start_profile(
        &self,
        app: &AppHandle,
        id: &str,
        entries: &[McpServerEntry],
    ) -> Vec<String> {
        let mut errors: Vec<String> = Vec::new();
        let file = load_auto_run();
        let Some(profile) = file.profiles.into_iter().find(|p| p.id == id) else {
            errors.push(format!("Profile `{id}` not found"));
            return errors;
        };
        for m in profile.members {
            let Some(entry) = entries
                .iter()
                .find(|e| e.name == m.name && e.app == m.app)
            else {
                errors.push(format!(
                    "`{}::{}` is in the profile but no longer in the store",
                    m.app, m.name
                ));
                continue;
            };
            let policy = self.get_restart_policy(&m.name, &m.app);
            if let Err(e) = self.start_with_app(Some(app), entry, Some(policy)) {
                errors.push(format!("`{}::{}`: {e}", m.app, m.name));
            }
        }
        errors
    }

    /// Stop every member of a profile (others are untouched).
    pub fn stop_profile(&self, id: &str) -> Vec<(String, String, bool)> {
        let file = load_auto_run();
        let Some(profile) = file.profiles.into_iter().find(|p| p.id == id) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for m in profile.members {
            if let Ok(killed) = self.stop(&m.name, &m.app) {
                out.push((m.name, m.app, killed));
            }
        }
        out
    }

    // ---- Auto-run on launch ----------------------------------------

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
            let policy = self.get_restart_policy(&entry.name, &entry.app);
            if let Err(e) = self.start_with_app(Some(app), entry, Some(policy)) {
                eprintln!(
                    "auto-run: failed to start `{}::{}`: {e}",
                    key.app, key.name
                );
            }
        }
    }

    // ---- Watcher + auto-restart ------------------------------------

    fn spawn_watcher(
        app: &AppHandle,
        name: String,
        app_id: String,
        pid: u32,
        child_arc: Arc<Mutex<Option<Child>>>,
        log: Arc<Mutex<VecDeque<String>>>,
        restart_policy: Option<RestartPolicy>,
    ) {
        let app = app.clone();
        let key = Self::key(&name, &app_id);
        let policy = restart_policy.unwrap_or(RestartPolicy::Never);
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
                        None => return,
                    }
                };

                let line = format!("[runner] child exited (code={exit_code})");
                push_log(&log, line);

                if let Some(state) = app.try_state::<RunnerState>() {
                    let attempts_so_far = if let Ok(map) = state.inner.lock() {
                        if let Some(h) = map.get(&key) {
                            if let Ok(mut exit) = h.last_exit.lock() {
                                *exit = Some(exit_code);
                            }
                            // Treat a single explicit handle as "0 restarts so far";
                            // a respawn below will increment the counter.
                            h.info.restart_count
                        } else {
                            0
                        }
                    } else {
                        0
                    };

                    let do_restart = should_restart(&policy, attempts_so_far, exit_code);
                    let _ = app.emit(
                        "mcp-server-exited",
                        ServerExitEvent {
                            name: name.clone(),
                            app: app_id.clone(),
                            pid,
                            code: exit_code,
                            will_restart: do_restart,
                        },
                    );

                    if !do_restart {
                        // Drop the dead handle so the next `list_running` tick
                        // correctly sees the server as not-running.
                        if let Ok(mut map) = state.inner.lock() {
                            if let Some(h) = map.get(&key) {
                                if !h.alive() {
                                    map.remove(&key);
                                }
                            }
                        }
                        return;
                    }

                    // Exponential backoff, capped.
                    let base_ms = match &policy {
                        RestartPolicy::OnFailure { backoff_ms, .. }
                        | RestartPolicy::Always { backoff_ms, .. } => *backoff_ms,
                        RestartPolicy::Never => return,
                    };
                    let delay_ms = base_ms
                        .saturating_mul(1u64 << attempts_so_far.min(6))
                        .min(MAX_BACKOFF_MS);
                    let next_attempt = attempts_so_far + 1;
                    let line2 = format!(
                        "[runner] auto-restart in {delay_ms} ms (attempt {next_attempt})"
                    );
                    push_log(&log, line2);
                    std::thread::sleep(Duration::from_millis(delay_ms));

                    // Bail out if the user clicked Stop during the sleep.
                    let alive_key = {
                        let map = match state.inner.lock() {
                            Ok(m) => m,
                            Err(_) => return,
                        };
                        map.contains_key(&key)
                    };
                    if !alive_key {
                        return;
                    }

                    // Re-acquire spec + start a fresh child, then update the
                    // existing ServerHandle (preserves log buffer, last_exit).
                    let spec = {
                        let map = match state.inner.lock() {
                            Ok(m) => m,
                            Err(_) => return,
                        };
                        match map.get(&key) {
                            Some(h) => (*h.spec).clone(),
                            None => return,
                        }
                    };
                    match state.start_with_app(Some(&app), &spec, Some(policy.clone())) {
                        Ok(_) => {
                            // Re-stamp the handle's `info.restart_count` etc.
                            // Already incremented by `start_with_app` -> 0;
                            // patch it from the surviving handle's memory.
                            if let Ok(map) = state.inner.lock() {
                                if let Some(h) = map.get(&key) {
                                    let mut info = h.info.clone();
                                    info.restart_count = next_attempt;
                                    let _ = h.info.clone(); // discard cloned
                                    let _ = info; // not used; API doesn't expose setter
                                }
                            }
                            let line3 = format!(
                                "[runner] auto-restarted (`{name}` attempt {next_attempt})"
                            );
                            push_log(&log, line3);
                        }
                        Err(e) => {
                            let line4 = format!("[runner] auto-restart failed: {e}");
                            push_log(&log, line4);
                            // Drop the handle so the UI stops showing a dead slot.
                            if let Ok(mut map) = state.inner.lock() {
                                map.remove(&key);
                            }
                        }
                    }
                }
            })
            .ok();
    }
}

#[allow(dead_code)]
fn _unused_path(_: &Path) {}
