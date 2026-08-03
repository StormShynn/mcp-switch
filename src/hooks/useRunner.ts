import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AppId,
  AutoRunKey,
  McpServerEntry,
  ProfileDto,
  RestartPolicy,
  RunningServer,
  ServerExitEvent,
} from "../lib/types";

export function useRunner(notify: (message: string, type: "success" | "error") => void) {
  const [running, setRunning] = useState<Record<string, RunningServer>>({});
  const [busyRunning, setBusyRunning] = useState<Set<string>>(new Set());
  const [logPeek, setLogPeek] = useState<Record<string, string[]>>({});
  const [logVisible, setLogVisible] = useState<Set<string>>(new Set());
  const [autoRun, setAutoRun] = useState<Set<string>>(new Set());
  const [lastExits, setLastExits] = useState<Record<string, number>>({});
  const [restartPolicies, setRestartPolicies] = useState<Record<string, RestartPolicy>>({});
  const [profiles, setProfiles] = useState<ProfileDto[]>([]);
  const [editingProfile, setEditingProfile] = useState<ProfileDto | null>(null);

  const keyOf = useCallback((serverName: string, appId: AppId) => `${appId}::${serverName}`, []);

  const refreshRunning = useCallback(async () => {
    try {
      const list = await invoke<RunningServer[]>("list_running");
      const next: Record<string, RunningServer> = {};
      for (const r of list) next[keyOf(r.name, r.app)] = r;
      setRunning(next);
      // refresh any visible log previews so newly-emitted lines show up
      setLogVisible((prev) => {
        if (prev.size === 0) return prev;
        const keys = Array.from(prev);
        Promise.all(
          keys.map((k) => {
            const [app, ...rest] = k.split("::");
            const name = rest.join("::");
            return invoke<string[]>("read_log", { serverName: name, appId: app, tail: 100 })
              .then((lines) => setLogPeek((p) => ({ ...p, [k]: lines })))
              .catch(() => {});
          })
        );
        return prev;
      });
    } catch {
      // backend not running -- silent, like everywhere else in this file
    }
  }, [keyOf]);

  useEffect(() => {
    refreshRunning();
    const t = window.setInterval(refreshRunning, 3000);
    return () => window.clearInterval(t);
  }, [refreshRunning]);

  const handleRun = useCallback(
    async (serverName: string, appId: AppId) => {
      const k = keyOf(serverName, appId);
      setBusyRunning((prev) => new Set(prev).add(k));
      try {
        const info = await invoke<RunningServer>("start_server", { serverName, appId });
        setRunning((prev) => ({ ...prev, [k]: info }));
        notify(`Started ${serverName} (pid ${info.pid})`, "success");
      } catch (err) {
        notify(err instanceof Error ? err.message : String(err), "error");
      } finally {
        setBusyRunning((prev) => {
          const next = new Set(prev);
          next.delete(k);
          return next;
        });
      }
    },
    [keyOf, notify]
  );

  const handleStop = useCallback(
    async (serverName: string, appId: AppId) => {
      const k = keyOf(serverName, appId);
      setBusyRunning((prev) => new Set(prev).add(k));
      try {
        const killed = await invoke<boolean>("stop_server", { serverName, appId });
        setRunning((prev) => {
          const next = { ...prev };
          delete next[k];
          return next;
        });
        notify(
          killed ? `Stopped ${serverName}` : `${serverName} wasn't running`,
          killed ? "success" : "error"
        );
      } catch (err) {
        notify(err instanceof Error ? err.message : String(err), "error");
      } finally {
        setBusyRunning((prev) => {
          const next = new Set(prev);
          next.delete(k);
          return next;
        });
      }
    },
    [keyOf, notify]
  );

  const handleToggleLog = useCallback(
    async (serverName: string, appId: AppId) => {
      const k = keyOf(serverName, appId);
      const wasOpen = logVisible.has(k);
      setLogVisible((prev) => {
        const next = new Set(prev);
        if (next.has(k)) next.delete(k);
        else next.add(k);
        return next;
      });
      if (!wasOpen) {
        try {
          const lines = await invoke<string[]>("read_log", { serverName, appId, tail: 100 });
          setLogPeek((prev) => ({ ...prev, [k]: lines }));
        } catch {
          setLogPeek((prev) => ({ ...prev, [k]: [] }));
        }
      }
    },
    [keyOf, logVisible]
  );

  const refreshAutoRun = useCallback(async () => {
    try {
      const list = await invoke<AutoRunKey[]>("get_auto_run");
      setAutoRun(new Set(list.map((k) => `${k.app}::${k.name}`)));
    } catch {
      // backend not running; leave autoRun empty
    }
  }, []);

  const handleToggleAutoRun = useCallback(
    async (serverName: string, appId: AppId) => {
      const k = `${appId}::${serverName}`;
      const next = !autoRun.has(k);
      try {
        await invoke<boolean>("set_auto_run", { serverName, appId, enabled: next });
        setAutoRun((prev) => {
          const out = new Set(prev);
          if (next) out.add(k);
          else out.delete(k);
          return out;
        });
        notify(next ? `Auto-run on for ${serverName}` : `Auto-run off for ${serverName}`, "success");
      } catch (err) {
        notify(err instanceof Error ? err.message : String(err), "error");
      }
    },
    [autoRun, notify]
  );

  const refreshPolicies = useCallback(async () => {
    try {
      const servers = await invoke<{ servers: McpServerEntry[] }>("list_servers");
      const next: Record<string, RestartPolicy> = {};
      for (const s of servers.servers) {
        if (s.transport !== "stdio") continue;
        try {
          const p = await invoke<RestartPolicy>("get_restart_policy", {
            serverName: s.name,
            appId: s.app,
          });
          if (p) next[`${s.app}::${s.name}`] = p;
        } catch {
          /* ignore per-server failures */
        }
      }
      setRestartPolicies(next);
    } catch {
      /* backend not running */
    }
  }, []);

  const refreshProfiles = useCallback(async () => {
    try {
      const list = await invoke<ProfileDto[]>("list_profiles");
      setProfiles(list);
    } catch {
      /* ignore */
    }
  }, []);

  const handleChangePolicy = useCallback(
    async (serverName: string, appId: AppId, policy: RestartPolicy) => {
      const k = `${appId}::${serverName}`;
      const prev = restartPolicies[k] ?? { mode: "never" };
      setRestartPolicies((p) => ({ ...p, [k]: policy }));
      try {
        await invoke("set_restart_policy", {
          serverName,
          appId,
          policy: { mode: policy.mode, maxRetries: "maxRetries" in policy ? policy.maxRetries : undefined, backoffMs: "backoffMs" in policy ? policy.backoffMs : undefined },
        });
      } catch (err) {
        setRestartPolicies((p) => ({ ...p, [k]: prev }));
        notify(err instanceof Error ? err.message : String(err), "error");
      }
    },
    [restartPolicies, notify]
  );

  const handleStartProfile = useCallback(
    async (id: string) => {
      try {
        const errs = await invoke<string[]>("start_profile", { id });
        if (errs.length > 0) {
          notify(`Profile started with ${errs.length} error(s)`, "error");
        } else {
          notify(`Profile \`${id}\` started`, "success");
        }
        refreshRunning();
      } catch (err) {
        notify(err instanceof Error ? err.message : String(err), "error");
      }
    },
    [notify, refreshRunning]
  );

  const handleStopProfile = useCallback(
    async (id: string) => {
      try {
        const results = await invoke<[string, string, boolean][]>("stop_profile", { id });
        const killed = results.filter((r) => r[2]).length;
        notify(`Stopped ${killed} of ${results.length} in profile \`${id}\``, "success");
        refreshRunning();
      } catch (err) {
        notify(err instanceof Error ? err.message : String(err), "error");
      }
    },
    [notify, refreshRunning]
  );

  const handleSaveProfile = useCallback(
    async (profile: ProfileDto) => {
      try {
        await invoke("upsert_profile", { profile });
        notify(`Saved profile \`${profile.label}\``, "success");
        setEditingProfile(null);
        refreshProfiles();
      } catch (err) {
        notify(err instanceof Error ? err.message : String(err), "error");
      }
    },
    [notify, refreshProfiles]
  );

  const handleDeleteProfile = useCallback(
    async (id: string) => {
      try {
        const ok = await invoke<boolean>("delete_profile", { id });
        if (ok) {
          notify(`Deleted profile`, "success");
          refreshProfiles();
        }
      } catch (err) {
        notify(err instanceof Error ? err.message : String(err), "error");
      }
    },
    [notify, refreshProfiles]
  );

  // Load the persisted auto-run list once on mount.
  useEffect(() => {
    refreshAutoRun();
  }, [refreshAutoRun]);

  // Load each stdio server's persisted restart policy once on mount, so the
  // per-row policy control reflects what was actually saved in a previous
  // session instead of defaulting every row to "never" until changed.
  useEffect(() => {
    refreshPolicies();
  }, [refreshPolicies]);

  // Listen for `mcp-server-exited` so a child that crashed unexpectedly
  // shows up as a transient toast + a stale `lastExits` entry that the row
  // can surface as "(crashed, rc=N)" until reaped.
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    listen<ServerExitEvent>("mcp-server-exited", (event) => {
      const { name, app, code } = event.payload;
      const k = `${app}::${name}`;
      setLastExits((prev) => ({ ...prev, [k]: code }));
      notify(
        code === 0
          ? `${name} exited cleanly`
          : `${name} crashed (rc=${code})`,
        code === 0 ? "success" : "error"
      );
      // Optimistically drop the entry from `running` so the UI flips back
      // to a Stop-less state before the next polling tick.
      setRunning((prev) => {
        if (!(k in prev)) return prev;
        const next = { ...prev };
        delete next[k];
        return next;
      });
      setTimeout(() => {
        setLastExits((prev) => {
          if (!(k in prev)) return prev;
          const next = { ...prev };
          delete next[k];
          return next;
        });
      }, 12000);
      // Refresh the log so the latest lines (incl. the [runner] exit line)
      // are visible if the user had the peek open.
      refreshRunning();
    })
      .then((fn) => (unlisten = fn))
      .catch(() => {});
    return () => {
      if (unlisten) unlisten();
    };
  }, [notify, refreshRunning]);

  return {
    running,
    busyRunning,
    logPeek,
    logVisible,
    autoRun,
    lastExits,
    restartPolicies,
    profiles,
    editingProfile,
    setEditingProfile,
    keyOf,
    refreshRunning,
    handleRun,
    handleStop,
    handleToggleLog,
    refreshAutoRun,
    handleToggleAutoRun,
    refreshPolicies,
    refreshProfiles,
    handleChangePolicy,
    handleStartProfile,
    handleStopProfile,
    handleSaveProfile,
    handleDeleteProfile,
  };
}
