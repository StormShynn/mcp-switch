import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { confirm, save, open } from "@tauri-apps/plugin-dialog";
import type {
  AppId,
  ConnectionTestResult,
  McpServerEntry,
  ServerInput,
  SyncSummary,
} from "../lib/types";
import { APPS } from "../lib/types";
import { type FilterKey, type SortDir, type SortKey, sortServers } from "../lib/sort";

/** Detect Tauri invoke errors that occur when the Rust backend isn't running */
function isBackendError(err: Error): boolean {
  const msg = err.message ?? "";
  // Tauri throws these when the Rust backend isn't available
  return msg.includes("Invoke not available") || msg.includes("backend is not running");
}

/** Apps MCP Switch can kill-and-relaunch itself (both are Windows Store
 * packages with a discoverable launch id). CLI tools run interactively in
 * whatever terminal the user already has open for them — there's no
 * single well-defined process to restart — so they only ever get the
 * plain "I already restarted it myself" dismiss action. */
export const RESTARTABLE_APPS: ReadonlySet<AppId> = new Set(["claude-desktop", "antigravity"]);

export function useServers(notify: (message: string, type: "success" | "error") => void) {
  const [servers, setServers] = useState<McpServerEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [sortKey, setSortKey] = useState<SortKey>("name");
  const [sortDir, setSortDir] = useState<SortDir>("asc");
  const [filter, setFilter] = useState<FilterKey>("all");
  const [importing, setImporting] = useState(false);
  const [pendingRestarts, setPendingRestarts] = useState<Set<AppId>>(new Set());
  const [editingServer, setEditingServer] = useState<McpServerEntry | "new" | null>(null);
  const [testResults, setTestResults] = useState<Record<string, { status: "testing" } | ConnectionTestResult>>({});
  const [searchQuery, setSearchQuery] = useState("");
  const searchRef = useRef<HTMLInputElement>(null);

  const dismissPendingRestart = useCallback((appId: AppId) => {
    setPendingRestarts((prev) => {
      if (!prev.has(appId)) return prev;
      const next = new Set(prev);
      next.delete(appId);
      return next;
    });
  }, []);

  const loadServers = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const store = await invoke<{ servers: McpServerEntry[] }>("list_servers");
      setServers(store.servers);
    } catch (err) {
      // Fallback: show sample data when backend is unavailable. "playwright"
      // appears twice on purpose — once per app — to illustrate that each
      // app's definition is independent even when the name matches.
      setServers([
        {
          name: "playwright",
          app: "claude",
          transport: "stdio",
          command: "npx @anthropic-ai/claude-code-mcp",
          args: [],
          enabled: true,
          deleted: false,
        },
        {
          name: "playwright",
          app: "codex",
          transport: "stdio",
          command: "npx @anthropic-ai/claude-code-mcp",
          args: [],
          enabled: true,
          deleted: false,
        },
        {
          name: "filesystem",
          app: "gemini",
          transport: "stdio",
          command: "npx @modelcontextprotocol/server-filesystem",
          args: ["/workspace"],
          enabled: true,
          deleted: false,
        },
        {
          name: "filesystem",
          app: "hermes",
          transport: "stdio",
          command: "npx @modelcontextprotocol/server-filesystem",
          args: ["/workspace"],
          enabled: true,
          deleted: false,
        },
        {
          name: "github",
          app: "opencode",
          transport: "stdio",
          command: "npx @modelcontextprotocol/server-github",
          args: [],
          enabled: true,
          deleted: false,
        },
      ]);
      // Only surface non-runtime errors (backend unavailable is expected)
      if (err instanceof Error && !isBackendError(err)) {
        setError(err.message);
      }
    } finally {
      setLoading(false);
    }
  }, []);

  // Re-sync from every tool's live config on every launch, so new/changed/
  // removed servers are reflected without needing a manual Import click.
  // loadServers() runs first so something (cache or dev-mode mock data)
  // paints immediately; the sync then quietly refreshes it.
  useEffect(() => {
    (async () => {
      await loadServers();
      try {
        await invoke<SyncSummary>("import_servers");
        await loadServers();
      } catch {
        // Backend unavailable (e.g. dev mode) — loadServers() already
        // populated fallback data above, so fail silently here.
      }
    })();
  }, [loadServers]);

  // Keyboard shortcuts
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement;
      const isInput = target.tagName === "INPUT" || target.tagName === "TEXTAREA";

      if (e.key === "Escape" && editingServer) {
        setEditingServer(null);
        return;
      }

      if ((e.ctrlKey || e.metaKey) && e.key === "f") {
        e.preventDefault();
        searchRef.current?.focus();
        return;
      }

      if ((e.ctrlKey || e.metaKey) && e.key === "n" && !isInput) {
        e.preventDefault();
        setEditingServer("new");
        return;
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [editingServer]);

  const handleToggle = useCallback(
    async (serverName: string, appId: AppId, enabled: boolean) => {
      // Optimistic update. Matches on (name, app) together, since the same
      // name can exist as a separate entry for another app.
      setServers((prev) =>
        prev.map((s) =>
          s.name === serverName && s.app === appId ? { ...s, enabled } : s
        )
      );

      try {
        await invoke("toggle_server", { serverName, appId, enabled });
        const appLabel = APPS.find((a) => a.id === appId)?.label ?? appId;
        notify(`${enabled ? "Enabled" : "Disabled"} for ${appLabel}`, "success");
        setPendingRestarts((prev) => {
          if (prev.has(appId)) return prev;
          return new Set(prev).add(appId);
        });
      } catch {
        // Revert on failure
        setServers((prev) =>
          prev.map((s) =>
            s.name === serverName && s.app === appId ? { ...s, enabled: !enabled } : s
          )
        );
        notify("Failed to toggle server", "error");
      }
    },
    [notify]
  );

  const handleImport = useCallback(async () => {
    try {
      setImporting(true);
      const summary = await invoke<SyncSummary>("import_servers");
      await loadServers();
      const parts: string[] = [];
      if (summary.added > 0) parts.push(`${summary.added} new`);
      if (summary.flaggedDeleted > 0) parts.push(`${summary.flaggedDeleted} moved to Trash`);
      notify(parts.length > 0 ? `Synced: ${parts.join(", ")}` : "Already up to date", "success");
    } catch {
      notify("Sync failed", "error");
    } finally {
      setImporting(false);
    }
  }, [loadServers, notify]);

  const handleSaveServer = useCallback(
    async (input: ServerInput) => {
      await invoke("save_server", { input });
      notify(`Saved "${input.name}"`, "success");
      await loadServers();
    },
    [loadServers, notify]
  );

  const handleExport = useCallback(async () => {
    try {
      const filePath = await save({
        filters: [{ name: "MCP Switch Config", extensions: ["json"] }],
        defaultPath: "mcp-switch-servers.json",
      });
      if (!filePath) return;
      await invoke("export_servers", { path: filePath });
      notify("Exported successfully", "success");
    } catch (err) {
      notify(err instanceof Error ? err.message : "Export failed", "error");
    }
  }, [notify]);

  const handleImportFromFile = useCallback(async () => {
    try {
      const filePath = await open({
        filters: [{ name: "MCP Switch Config", extensions: ["json"] }],
        multiple: false,
      });
      if (!filePath) return;
      const added = await invoke<number>("import_servers_from_file", { path: filePath });
      await loadServers();
      notify(
        added > 0 ? `Imported ${added} server${added > 1 ? "s" : ""}` : "No new servers to import",
        "success"
      );
    } catch (err) {
      notify(err instanceof Error ? err.message : "Import failed", "error");
    }
  }, [loadServers, notify]);

  const handleTrash = useCallback(
    async (serverName: string, appId: AppId) => {
      try {
        await invoke("trash_server", { serverName, appId });
        notify(`Moved "${serverName}" to Trash`, "success");
        await loadServers();
      } catch {
        notify("Failed to move to Trash", "error");
      }
    },
    [loadServers, notify]
  );

  const handleRestartApp = useCallback(
    async (appId: AppId) => {
      const appLabel = APPS.find((a) => a.id === appId)?.label ?? appId;
      const ok = await confirm(
        `Restart ${appLabel}? Any unsaved state in it will be lost.`,
        { title: "Restart app", kind: "warning" }
      );
      if (!ok) return;
      try {
        await invoke("restart_app", { appId });
        notify(`Restarted ${appLabel}`, "success");
        dismissPendingRestart(appId);
      } catch (err) {
        notify(err instanceof Error ? err.message : String(err), "error");
      }
    },
    [dismissPendingRestart, notify]
  );

  const handleRestore = useCallback(
    async (serverName: string, appId: AppId) => {
      try {
        await invoke("restore_server", { serverName, appId });
        notify(`Restored "${serverName}"`, "success");
        await loadServers();
      } catch {
        notify("Restore failed", "error");
      }
    },
    [loadServers, notify]
  );

  const handleDeleteForever = useCallback(
    async (serverName: string, appId: AppId) => {
      const ok = await confirm(
        `Permanently delete "${serverName}"? This cannot be undone.`,
        { title: "Delete forever", kind: "warning" }
      );
      if (!ok) return;
      try {
        await invoke("delete_server_forever", { serverName, appId });
        notify(`Deleted "${serverName}" forever`, "success");
        await loadServers();
      } catch {
        notify("Delete failed", "error");
      }
    },
    [loadServers, notify]
  );

  const handleTestConnection = useCallback(
    async (serverName: string, appId: AppId) => {
      const key = `${serverName}::${appId}`;
      setTestResults((prev) => ({ ...prev, [key]: { status: "testing" } }));
      try {
        const result = await invoke<ConnectionTestResult>("test_server_connection", { serverName, appId });
        setTestResults((prev) => ({ ...prev, [key]: result }));
        setTimeout(() => {
          setTestResults((prev) => {
            const next = { ...prev };
            delete next[key];
            return next;
          });
        }, 6000);
      } catch (err) {
        setTestResults((prev) => ({
          ...prev,
          [key]: { success: false, message: err instanceof Error ? err.message : String(err), serverInfo: null },
        }));
        setTimeout(() => {
          setTestResults((prev) => {
            const next = { ...prev };
            delete next[key];
            return next;
          });
        }, 6000);
      }
    },
    []
  );

  const handleClone = useCallback((server: McpServerEntry) => {
    setEditingServer({
      ...server,
      name: `${server.name}-copy`,
    });
  }, []);

  const toggleSort = (key: SortKey) => {
    if (sortKey === key) {
      setSortDir((d) => (d === "asc" ? "desc" : "asc"));
    } else {
      setSortKey(key);
      setSortDir("asc");
    }
  };

  const filtered = searchQuery
    ? servers.filter((s) => s.name.toLowerCase().includes(searchQuery.toLowerCase()))
    : servers;
  const sorted = sortServers(filtered, sortKey, sortDir, filter);
  const trashCount = servers.filter((s) => s.deleted).length;

  return {
    servers,
    loading,
    error,
    sortKey,
    sortDir,
    filter,
    setFilter,
    importing,
    pendingRestarts,
    dismissPendingRestart,
    editingServer,
    setEditingServer,
    testResults,
    searchQuery,
    setSearchQuery,
    searchRef,
    sorted,
    trashCount,
    loadServers,
    handleToggle,
    handleImport,
    handleSaveServer,
    handleExport,
    handleImportFromFile,
    handleTrash,
    handleRestartApp,
    handleRestore,
    handleDeleteForever,
    handleTestConnection,
    handleClone,
    toggleSort,
  };
}
