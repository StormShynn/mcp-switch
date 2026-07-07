import { useState, useEffect, useCallback } from "react";

/** Detect Tauri invoke errors that occur when the Rust backend isn't running */
function isBackendError(err: Error): boolean {
  const msg = err.message ?? "";
  // Tauri throws these when the Rust backend isn't available
  return msg.includes("Invoke not available") || msg.includes("backend is not running");
}
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { confirm } from "@tauri-apps/plugin-dialog";
import type { McpServerEntry, AppId, ServerInput, SyncSummary, Transport } from "./lib/types";
import { APPS, APP_COLORS } from "./lib/types";

type UpdateStatus =
  | "idle"
  | "checking"
  | "up-to-date"
  | "available"
  | "downloading"
  | "installing"
  | "error";

const REPO_URL = "https://github.com/StormShynn/mcp-switch";

/** Apps MCP Switch can kill-and-relaunch itself (both are Windows Store
 * packages with a discoverable launch id). CLI tools run interactively in
 * whatever terminal the user already has open for them — there's no
 * single well-defined process to restart — so they only ever get the
 * plain "I already restarted it myself" dismiss action. */
const RESTARTABLE_APPS: ReadonlySet<AppId> = new Set(["claude-desktop", "antigravity"]);

type SortKey = "name" | "status";
type SortDir = "asc" | "desc";
type FilterKey = "all" | AppId | "trash";

function sortServers(
  servers: McpServerEntry[],
  key: SortKey,
  dir: SortDir,
  filter: FilterKey
): McpServerEntry[] {
  let filtered: McpServerEntry[];
  if (filter === "trash") {
    filtered = servers.filter((s) => s.deleted);
  } else if (filter === "all") {
    filtered = servers.filter((s) => !s.deleted);
  } else {
    filtered = servers.filter((s) => !s.deleted && s.app === filter);
  }

  return [...filtered].sort((a, b) => {
    let cmp: number;
    if (key === "name") {
      cmp = a.name.localeCompare(b.name);
    } else {
      cmp = Number(b.enabled) - Number(a.enabled);
    }
    return dir === "asc" ? cmp : -cmp;
  });
}

/* ── Server row component ────────────────────────── */
function ServerRow({
  server,
  index,
  onToggle,
  onEdit,
  onTrash,
}: {
  server: McpServerEntry;
  index: number;
  onToggle: (serverName: string, appId: AppId, enabled: boolean) => void;
  onEdit: (server: McpServerEntry) => void;
  onTrash: (serverName: string, appId: AppId) => void;
}) {
  const appLabel = APPS.find((a) => a.id === server.app)?.label ?? server.app;

  return (
    <div
      className="server-row fade-in server-row-clickable"
      style={{ animationDelay: `${index * 30}ms` }}
      onClick={() => onEdit(server)}
      title="Click to edit"
    >
      <div className="server-info">
        <div className="server-name">{server.name}</div>
        <div className="server-command">
          {server.transport === "stdio" ? server.command : server.url}
        </div>
        <div className="server-meta">
          <span className="badge" style={{ color: APP_COLORS[server.app] }}>
            {appLabel}
          </span>
        </div>
      </div>

      <div className="server-toggles">
        <label
          className="toggle app-toggle"
          title={`${appLabel} — ${server.enabled ? "enabled" : "disabled"}`}
          onClick={(e) => e.stopPropagation()}
        >
          <input
            type="checkbox"
            checked={server.enabled}
            onChange={(e) => onToggle(server.name, server.app, e.target.checked)}
          />
          <span className="toggle-track" />
        </label>
        <button
          className="btn btn-sm btn-danger"
          title="Move to Trash"
          onClick={(e) => {
            e.stopPropagation();
            onTrash(server.name, server.app);
          }}
        >
          Delete
        </button>
      </div>
    </div>
  );
}

/* ── Trash row component ─────────────────────────── */
function TrashRow({
  server,
  index,
  onRestore,
  onDeleteForever,
}: {
  server: McpServerEntry;
  index: number;
  onRestore: (serverName: string, appId: AppId) => void;
  onDeleteForever: (serverName: string, appId: AppId) => void;
}) {
  const appLabel = APPS.find((a) => a.id === server.app)?.label ?? server.app;

  return (
    <div className="server-row fade-in" style={{ animationDelay: `${index * 30}ms` }}>
      <div className="server-info">
        <div className="server-name">{server.name}</div>
        <div className="server-command">
          {server.transport === "stdio" ? server.command : server.url}
        </div>
        <div className="server-meta">
          <span className="badge" style={{ color: APP_COLORS[server.app] }}>
            {appLabel}
          </span>
          <span className="badge badge-trash">No longer found</span>
        </div>
      </div>

      <div className="server-toggles">
        <button className="btn btn-sm" onClick={() => onRestore(server.name, server.app)}>
          Restore
        </button>
        <button
          className="btn btn-sm btn-danger"
          onClick={() => onDeleteForever(server.name, server.app)}
        >
          Delete forever
        </button>
      </div>
    </div>
  );
}

/* ── Empty state ─────────────────────────────────── */
function EmptyState({ onImport }: { onImport: () => void }) {
  return (
    <div className="empty-state fade-in">
      <div className="empty-icon">
        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
          <path d="M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z" />
          <polyline points="13 2 13 9 20 9" />
          <line x1="9" y1="13" x2="15" y2="13" />
          <line x1="12" y1="10" x2="12" y2="16" />
        </svg>
      </div>
      <h2>No MCP servers yet</h2>
      <p>
        Import existing servers from your installed coding tools, or add a new
        server manually.
      </p>
      <button className="btn btn-primary" onClick={onImport}>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
          <polyline points="7 10 12 15 17 10" />
          <line x1="12" y1="15" x2="12" y2="3" />
        </svg>
        Import from tools
      </button>
    </div>
  );
}

/* ── Add/Edit server form ────────────────────────── */
interface ServerFormState {
  originalName: string | null;
  name: string;
  app: AppId | "";
  enabled: boolean;
  transport: Transport;
  command: string;
  argsText: string;
  envText: string;
  url: string;
  headersText: string;
}

function emptyServerForm(): ServerFormState {
  return {
    originalName: null,
    name: "",
    app: "",
    enabled: true,
    transport: "stdio",
    command: "",
    argsText: "",
    envText: "",
    url: "",
    headersText: "",
  };
}

function keyValueMapToLines(map: Record<string, string> | undefined): string {
  return Object.entries(map ?? {})
    .map(([k, v]) => `${k}=${v}`)
    .join("\n");
}

function formFromServer(server: McpServerEntry): ServerFormState {
  return {
    originalName: server.name,
    name: server.name,
    app: server.app,
    enabled: server.enabled,
    transport: server.transport,
    command: server.command ?? "",
    argsText: (server.args ?? []).join("\n"),
    envText: keyValueMapToLines(server.env),
    url: server.url ?? "",
    headersText: keyValueMapToLines(server.headers),
  };
}

function parseLines(text: string): string[] | undefined {
  const lines = text
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean);
  return lines.length > 0 ? lines : undefined;
}

function parseKeyValueLines(text: string): Record<string, string> | undefined {
  const out: Record<string, string> = {};
  for (const line of text.split("\n").map((l) => l.trim()).filter(Boolean)) {
    const idx = line.indexOf("=");
    if (idx <= 0) continue;
    out[line.slice(0, idx).trim()] = line.slice(idx + 1).trim();
  }
  return Object.keys(out).length > 0 ? out : undefined;
}

/* ── Paste-JSON-to-autofill ──────────────────────── */
interface ParsedServerJson {
  name?: string;
  transport?: Transport;
  command?: string;
  argsText?: string;
  envText?: string;
  url?: string;
  headersText?: string;
}

function asStringArray(value: unknown): string[] | undefined {
  if (!Array.isArray(value)) return undefined;
  const items = value.filter((v): v is string => typeof v === "string");
  return items.length > 0 ? items : undefined;
}

function asStringRecord(value: unknown): Record<string, string> | undefined {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return undefined;
  const out: Record<string, string> = {};
  for (const [k, v] of Object.entries(value as Record<string, unknown>)) {
    if (typeof v === "string") out[k] = v;
    else if (typeof v === "number" || typeof v === "boolean") out[k] = String(v);
  }
  return Object.keys(out).length > 0 ? out : undefined;
}

function looksLikeServerEntry(value: unknown): value is Record<string, unknown> {
  return (
    typeof value === "object" &&
    value !== null &&
    !Array.isArray(value) &&
    ("command" in value || "url" in value || "httpUrl" in value || "serverUrl" in value)
  );
}

/** Parses `raw` as JSON. If that fails and `raw` doesn't already look like a
 * complete object/array (e.g. the user copied just `"name": { ... }` from
 * inside their real config's `mcpServers` block, braces and all left
 * behind), retries after wrapping it in `{ }` — stripping a trailing comma
 * first, since a copied middle-of-object entry often has one. Throws the
 * *original* error when even that doesn't parse, since it's more likely to
 * point at the real problem than an error from the synthetic wrapper. */
function tryParseJson(raw: string): unknown {
  try {
    return JSON.parse(raw);
  } catch (firstErr) {
    const trimmed = raw.trim();
    if (trimmed.startsWith("{") || trimmed.startsWith("[")) throw firstErr;
    try {
      return JSON.parse(`{${trimmed.replace(/,\s*$/, "")}}`);
    } catch {
      throw firstErr instanceof Error ? firstErr : new Error("Invalid JSON");
    }
  }
}

/** Recognizes the JSON shapes MCP servers are commonly documented in — a
 * bare entry (`{"command": "npx", ...}`), a full `{"mcpServers": {name:
 * {...}}}` block copied from another tool's config, or a single `{name:
 * {...}}` pair — and pulls out the fields the form needs. Returns null when
 * `raw` doesn't look like a server config at all. */
function extractServerConfig(raw: string): ParsedServerJson | null {
  const parsed: unknown = tryParseJson(raw);
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
    throw new Error("Expected a JSON object");
  }

  let name: string | undefined;
  let body: Record<string, unknown> = parsed as Record<string, unknown>;

  const mcpServers = body.mcpServers;
  if (typeof mcpServers === "object" && mcpServers !== null && !Array.isArray(mcpServers)) {
    const [firstName, firstValue] = Object.entries(mcpServers)[0] ?? [];
    if (firstValue !== undefined && looksLikeServerEntry(firstValue)) {
      name = firstName;
      body = firstValue;
    }
  } else if (!looksLikeServerEntry(body)) {
    const [firstName, firstValue] = Object.entries(body)[0] ?? [];
    if (firstValue !== undefined && looksLikeServerEntry(firstValue)) {
      name = firstName;
      body = firstValue;
    }
  }

  if (!looksLikeServerEntry(body)) return null;

  let command: string | undefined;
  let args: string[] | undefined;
  if (Array.isArray(body.command)) {
    const [cmd, ...rest] = body.command;
    if (typeof cmd === "string") command = cmd;
    args = asStringArray(rest);
  } else if (typeof body.command === "string") {
    command = body.command;
    args = asStringArray(body.args);
  }

  const url = [body.url, body.httpUrl, body.serverUrl].find(
    (v): v is string => typeof v === "string"
  );
  const rawType =
    typeof body.type === "string"
      ? body.type
      : typeof body.transport === "string"
      ? body.transport
      : undefined;
  const transport: Transport | undefined = command
    ? "stdio"
    : url
    ? rawType === "http" || rawType === "streamable-http"
      ? "http"
      : "sse"
    : undefined;

  const env = asStringRecord(body.env ?? body.environment);
  const headers = asStringRecord(body.headers ?? body.http_headers);

  return {
    name,
    transport,
    command,
    argsText: args ? args.join("\n") : undefined,
    envText: env ? keyValueMapToLines(env) : undefined,
    url,
    headersText: headers ? keyValueMapToLines(headers) : undefined,
  };
}

function ServerFormModal({
  initial,
  onClose,
  onSave,
}: {
  initial: ServerFormState;
  onClose: () => void;
  onSave: (input: ServerInput) => Promise<void>;
}) {
  const [form, setForm] = useState<ServerFormState>(initial);
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const [jsonPaste, setJsonPaste] = useState("");
  const [jsonNotice, setJsonNotice] = useState<{
    type: "success" | "error";
    message: string;
  } | null>(null);
  const isEditing = initial.originalName !== null;

  const applyJsonPasteText = (text: string) => {
    const trimmed = text.trim();
    if (!trimmed) return;
    try {
      const parsed = extractServerConfig(trimmed);
      if (!parsed) {
        setJsonNotice({ type: "error", message: "Couldn't find a server config in that JSON" });
        return;
      }
      setForm((f) => ({
        ...f,
        name: !isEditing && parsed.name ? parsed.name : f.name,
        transport: parsed.transport ?? f.transport,
        command: parsed.command ?? f.command,
        argsText: parsed.argsText ?? f.argsText,
        envText: parsed.envText ?? f.envText,
        url: parsed.url ?? f.url,
        headersText: parsed.headersText ?? f.headersText,
      }));
      setJsonNotice({ type: "success", message: "Filled from JSON" });
      setJsonPaste("");
    } catch (err) {
      setJsonNotice({
        type: "error",
        message: err instanceof Error ? err.message : "Invalid JSON",
      });
    }
  };

  const handleJsonPaste = (e: React.ClipboardEvent<HTMLTextAreaElement>) => {
    const text = e.clipboardData.getData("text");
    if (!text.trim()) return;
    e.preventDefault();
    setJsonPaste(text);
    applyJsonPasteText(text);
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const name = form.name.trim();
    if (!name) {
      setFormError("Name is required");
      return;
    }
    if (!form.app) {
      setFormError("Choose which app this server belongs to");
      return;
    }
    if (form.transport === "stdio" && !form.command.trim()) {
      setFormError("Command is required for a stdio server");
      return;
    }
    if (form.transport !== "stdio" && !form.url.trim()) {
      setFormError("URL is required for a remote server");
      return;
    }

    const input: ServerInput =
      form.transport === "stdio"
        ? {
            name,
            app: form.app,
            enabled: form.enabled,
            transport: "stdio",
            command: form.command.trim(),
            args: parseLines(form.argsText),
            env: parseKeyValueLines(form.envText),
          }
        : {
            name,
            app: form.app,
            enabled: form.enabled,
            transport: form.transport,
            url: form.url.trim(),
            headers: parseKeyValueLines(form.headersText),
          };

    setSaving(true);
    setFormError(null);
    try {
      await onSave(input);
      onClose();
    } catch (err) {
      setFormError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <form
        className="modal fade-in server-form"
        onClick={(e) => e.stopPropagation()}
        onSubmit={handleSubmit}
      >
        <h2>{isEditing ? "Edit server" : "Add server"}</h2>

        <div className="form-field">
          <span>Paste JSON (optional)</span>
          <textarea
            className="form-input"
            value={jsonPaste}
            onChange={(e) => setJsonPaste(e.target.value)}
            onPaste={handleJsonPaste}
            rows={3}
            placeholder={'e.g. {"command": "npx", "args": ["-y", "@scope/server"]}'}
          />
          <div className="json-paste-row">
            <button
              type="button"
              className="btn btn-sm"
              onClick={() => applyJsonPasteText(jsonPaste)}
            >
              Fill from JSON
            </button>
            {jsonNotice && (
              <span className={`json-paste-notice json-paste-notice-${jsonNotice.type}`}>
                {jsonNotice.message}
              </span>
            )}
          </div>
        </div>

        <label className="form-field">
          <span>Name</span>
          <input
            className="form-input"
            value={form.name}
            onChange={(e) => setForm((f) => ({ ...f, name: e.target.value }))}
            disabled={isEditing}
            placeholder="filesystem"
            autoFocus
          />
        </label>

        <div className="form-field">
          <span>App</span>
          <div className="app-checkboxes">
            {APPS.map((appInfo) => (
              <label key={appInfo.id} className="app-checkbox">
                <input
                  type="radio"
                  name="app"
                  checked={form.app === appInfo.id}
                  disabled={isEditing}
                  onChange={() => setForm((f) => ({ ...f, app: appInfo.id }))}
                />
                <span style={{ color: APP_COLORS[appInfo.id] }}>{appInfo.label}</span>
              </label>
            ))}
          </div>
        </div>

        <div className="form-field">
          <span>Transport</span>
          <div className="transport-toggle">
            <button
              type="button"
              className={form.transport === "stdio" ? "active" : ""}
              onClick={() => setForm((f) => ({ ...f, transport: "stdio" }))}
            >
              stdio (command)
            </button>
            <button
              type="button"
              className={form.transport !== "stdio" ? "active" : ""}
              onClick={() => setForm((f) => ({ ...f, transport: "sse" }))}
            >
              remote (URL)
            </button>
          </div>
        </div>

        {form.transport === "stdio" ? (
          <>
            <label className="form-field">
              <span>Command</span>
              <input
                className="form-input"
                value={form.command}
                onChange={(e) => setForm((f) => ({ ...f, command: e.target.value }))}
                placeholder="npx"
              />
            </label>
            <label className="form-field">
              <span>Args (one per line)</span>
              <textarea
                className="form-input"
                value={form.argsText}
                onChange={(e) => setForm((f) => ({ ...f, argsText: e.target.value }))}
                rows={3}
                placeholder={"-y\n@modelcontextprotocol/server-filesystem"}
              />
            </label>
            <label className="form-field">
              <span>Env (KEY=VALUE, one per line)</span>
              <textarea
                className="form-input"
                value={form.envText}
                onChange={(e) => setForm((f) => ({ ...f, envText: e.target.value }))}
                rows={2}
              />
            </label>
          </>
        ) : (
          <>
            <label className="form-field">
              <span>URL</span>
              <input
                className="form-input"
                value={form.url}
                onChange={(e) => setForm((f) => ({ ...f, url: e.target.value }))}
                placeholder="https://example.com/mcp"
              />
            </label>
            <label className="form-field">
              <span>Headers (KEY=VALUE, one per line)</span>
              <textarea
                className="form-input"
                value={form.headersText}
                onChange={(e) => setForm((f) => ({ ...f, headersText: e.target.value }))}
                rows={2}
              />
            </label>
          </>
        )}

        <div className="form-field">
          <span>Status</span>
          <label className="app-checkbox">
            <input
              type="checkbox"
              checked={form.enabled}
              onChange={(e) => setForm((f) => ({ ...f, enabled: e.target.checked }))}
            />
            <span>Enabled</span>
          </label>
        </div>

        {formError && <div className="form-error">{formError}</div>}

        <div className="modal-actions">
          <button type="button" className="btn" onClick={onClose}>
            Cancel
          </button>
          <button type="submit" className="btn btn-primary" disabled={saving}>
            {saving ? "Saving…" : "Save"}
          </button>
        </div>
      </form>
    </div>
  );
}

/* ── About modal ─────────────────────────────────── */
function AboutModal({
  version,
  storePath,
  onClose,
  updateStatus,
  updateVersion,
  updateProgress,
  updateError,
  onCheckForUpdates,
  onInstallUpdate,
}: {
  version: string;
  storePath: string;
  onClose: () => void;
  updateStatus: UpdateStatus;
  updateVersion: string;
  updateProgress: number;
  updateError: string;
  onCheckForUpdates: () => void;
  onInstallUpdate: () => void;
}) {
  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal fade-in" onClick={(e) => e.stopPropagation()}>
        <h2>MCP Switch</h2>
        <div className="modal-row">
          <span>Version</span>
          <span>{version || "…"}</span>
        </div>
        <div className="modal-row">
          <span>Repository</span>
          <a
            className="modal-link"
            onClick={() => openUrl(REPO_URL)}
          >
            StormShynn/mcp-switch
          </a>
        </div>
        <div className="modal-row">
          <span>License</span>
          <span>MIT</span>
        </div>
        <div className="modal-row modal-row-path">
          <span>Store file</span>
          <span className="modal-path">{storePath || "…"}</span>
        </div>
        <div className="modal-row">
          <span>Updates</span>
          {updateStatus === "idle" && (
            <button className="btn modal-link-btn" onClick={onCheckForUpdates}>
              Check for updates
            </button>
          )}
          {updateStatus === "checking" && <span>Checking…</span>}
          {updateStatus === "up-to-date" && <span>Up to date</span>}
          {updateStatus === "available" && (
            <button className="btn btn-primary" onClick={onInstallUpdate}>
              Update to v{updateVersion}
            </button>
          )}
          {updateStatus === "downloading" && (
            <span>Downloading… {updateProgress}%</span>
          )}
          {updateStatus === "installing" && <span>Installing…</span>}
          {updateStatus === "error" && (
            <span className="modal-update-error" title={updateError}>
              Check failed
            </span>
          )}
        </div>
        <button className="btn modal-close" onClick={onClose}>
          Close
        </button>
      </div>
    </div>
  );
}

/* ── Main App ────────────────────────────────────── */
export default function App() {
  const [servers, setServers] = useState<McpServerEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [sortKey, setSortKey] = useState<SortKey>("name");
  const [sortDir, setSortDir] = useState<SortDir>("asc");
  const [filter, setFilter] = useState<FilterKey>("all");
  const [importing, setImporting] = useState(false);
  const [notification, setNotification] = useState<{
    message: string;
    type: "success" | "error";
  } | null>(null);
  const [showAbout, setShowAbout] = useState(false);
  const [version, setVersion] = useState("");
  const [storePath, setStorePath] = useState("");
  const [pendingRestarts, setPendingRestarts] = useState<Set<AppId>>(new Set());
  const [updateStatus, setUpdateStatus] = useState<UpdateStatus>("idle");
  const [pendingUpdate, setPendingUpdate] = useState<Update | null>(null);
  const [updateProgress, setUpdateProgress] = useState(0);
  const [updateError, setUpdateError] = useState("");
  const [editingServer, setEditingServer] = useState<McpServerEntry | "new" | null>(null);

  const dismissPendingRestart = useCallback((appId: AppId) => {
    setPendingRestarts((prev) => {
      if (!prev.has(appId)) return prev;
      const next = new Set(prev);
      next.delete(appId);
      return next;
    });
  }, []);

  const notify = useCallback((message: string, type: "success" | "error") => {
    setNotification({ message, type });
    setTimeout(() => setNotification(null), 3000);
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

  const handleShowAbout = useCallback(async () => {
    setShowAbout(true);
    if (!version) {
      getVersion().then(setVersion).catch(() => setVersion("unknown"));
    }
    if (!storePath) {
      invoke<string>("get_store_path").then(setStorePath).catch(() => {});
    }
  }, [version, storePath]);

  const handleCheckForUpdates = useCallback(async () => {
    setUpdateStatus("checking");
    setUpdateError("");
    try {
      const update = await check();
      if (update) {
        setPendingUpdate(update);
        setUpdateStatus("available");
      } else {
        setUpdateStatus("up-to-date");
      }
    } catch (err) {
      setUpdateError(err instanceof Error ? err.message : String(err));
      setUpdateStatus("error");
    }
  }, []);

  const handleInstallUpdate = useCallback(async () => {
    if (!pendingUpdate) return;
    setUpdateStatus("downloading");
    setUpdateProgress(0);
    try {
      let downloaded = 0;
      let total = 0;
      await pendingUpdate.downloadAndInstall((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength ?? 0;
        } else if (event.event === "Progress") {
          downloaded += event.data.chunkLength;
          setUpdateProgress(total > 0 ? Math.min(100, Math.round((downloaded / total) * 100)) : 0);
        } else if (event.event === "Finished") {
          setUpdateStatus("installing");
        }
      });
      await relaunch();
    } catch (err) {
      setUpdateError(err instanceof Error ? err.message : String(err));
      setUpdateStatus("error");
    }
  }, [pendingUpdate]);

  const toggleSort = (key: SortKey) => {
    if (sortKey === key) {
      setSortDir((d) => (d === "asc" ? "desc" : "asc"));
    } else {
      setSortKey(key);
      setSortDir("asc");
    }
  };

  const sorted = sortServers(servers, sortKey, sortDir, filter);
  const sortArrow = (key: SortKey) =>
    sortKey === key ? (sortDir === "asc" ? " ▲" : " ▼") : "";
  const trashCount = servers.filter((s) => s.deleted).length;

  return (
    <div className="app-container">
      {/* Header */}
      <header className="app-header">
        <div className="header-left">
          <h1 className="app-title">MCP Switch</h1>
          <span className="app-subtitle">
            {servers.length} server{servers.length !== 1 ? "s" : ""}
          </span>
        </div>
        <div className="header-actions">
          <button className="btn" onClick={handleShowAbout} title="About MCP Switch">
            About
          </button>
          <button className="btn" onClick={() => setEditingServer("new")}>
            Add server
          </button>
          <button
            className="btn btn-primary"
            onClick={handleImport}
            disabled={importing}
          >
            {importing ? "Syncing…" : "Import"}
          </button>
        </div>
      </header>

      {showAbout && (
        <AboutModal
          version={version}
          storePath={storePath}
          onClose={() => setShowAbout(false)}
          updateStatus={updateStatus}
          updateVersion={pendingUpdate?.version ?? ""}
          updateProgress={updateProgress}
          updateError={updateError}
          onCheckForUpdates={handleCheckForUpdates}
          onInstallUpdate={handleInstallUpdate}
        />
      )}

      {editingServer !== null && (
        <ServerFormModal
          initial={editingServer === "new" ? emptyServerForm() : formFromServer(editingServer)}
          onClose={() => setEditingServer(null)}
          onSave={handleSaveServer}
        />
      )}

      {/* Restart reminder */}
      {pendingRestarts.size > 0 && (
        <div className="restart-banner">
          <span className="restart-banner-label">Restart to apply:</span>
          <div className="restart-banner-chips">
            {APPS.filter((a) => pendingRestarts.has(a.id)).map((a) =>
              RESTARTABLE_APPS.has(a.id) ? (
                <span key={a.id} className="restart-chip restart-chip-actionable">
                  <button
                    className="restart-chip-action"
                    onClick={() => handleRestartApp(a.id)}
                    title={`Kill and relaunch ${a.label}`}
                  >
                    <span style={{ color: APP_COLORS[a.id] }}>{a.label}</span>
                    <span> — Restart</span>
                  </button>
                  <button
                    className="restart-chip-x"
                    onClick={() => dismissPendingRestart(a.id)}
                    title={`Dismiss — I already restarted ${a.label}`}
                  >
                    ×
                  </button>
                </span>
              ) : (
                <button
                  key={a.id}
                  className="restart-chip"
                  onClick={() => dismissPendingRestart(a.id)}
                  title={`Dismiss — I already restarted ${a.label}`}
                >
                  <span style={{ color: APP_COLORS[a.id] }}>{a.label}</span>
                  <span className="restart-chip-x">×</span>
                </button>
              )
            )}
          </div>
        </div>
      )}

      {/* Notification */}
      {notification && (
        <div className={`notification notification-${notification.type} slide-in`}>
          {notification.message}
        </div>
      )}

      {/* Toolbar */}
      <div className="toolbar">
        <div className="filter-group">
          {(["all", ...APPS.map((a) => a.id), "trash"] as FilterKey[]).map((f) => (
            <button
              key={f}
              className={`filter-chip ${filter === f ? "active" : ""} ${f === "trash" ? "filter-chip-trash" : ""}`}
              onClick={() => setFilter(f)}
            >
              {f === "all"
                ? "All"
                : f === "trash"
                ? `Trash${trashCount > 0 ? ` (${trashCount})` : ""}`
                : APPS.find((a) => a.id === f)?.label.split(" ")[0] ?? f}
            </button>
          ))}
        </div>
        <div className="sort-group">
          <button
            className={`btn btn-sort ${sortKey === "name" ? "active" : ""}`}
            onClick={() => toggleSort("name")}
          >
            Name{sortArrow("name")}
          </button>
          <button
            className={`btn btn-sort ${sortKey === "status" ? "active" : ""}`}
            onClick={() => toggleSort("status")}
          >
            Status{sortArrow("status")}
          </button>
        </div>
      </div>

      {/* Content */}
      <div className="app-content">
        {loading ? (
          <div className="loading-state">
            <div className="spinner" />
            <p>Loading servers…</p>
          </div>
        ) : sorted.length === 0 ? (
          filter === "trash" ? (
            <div className="empty-state fade-in">
              <h2>Trash is empty</h2>
              <p>Servers removed from every tool that used to define them show up here.</p>
            </div>
          ) : (
            <EmptyState onImport={handleImport} />
          )
        ) : (
          <div className="server-list">
            {filter === "trash"
              ? sorted.map((server, i) => (
                  <TrashRow
                    key={`${server.name}::${server.app}`}
                    server={server}
                    index={i}
                    onRestore={handleRestore}
                    onDeleteForever={handleDeleteForever}
                  />
                ))
              : sorted.map((server, i) => (
                  <ServerRow
                    key={`${server.name}::${server.app}`}
                    server={server}
                    index={i}
                    onToggle={handleToggle}
                    onEdit={setEditingServer}
                    onTrash={handleTrash}
                  />
                ))}
          </div>
        )}
      </div>

      {/* Legend */}
      <footer className="app-footer">
        <div className="legend">
          {APPS.map((app) => (
            <span key={app.id} className="legend-item">
              <span
                className="legend-dot"
                style={{ backgroundColor: APP_COLORS[app.id] }}
              />
              {app.label}
            </span>
          ))}
        </div>
      </footer>

    </div>
  );
}
