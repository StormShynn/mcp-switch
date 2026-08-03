import { useState } from "react";
import type { AppId, McpServerEntry, ServerInput, Transport } from "../lib/types";
import { APPS, APP_COLORS } from "../lib/types";
import { SERVER_TEMPLATES } from "../lib/templates";
import { extractServerConfig } from "../lib/parseJson";
import { keyValueMapToLines, parseKeyValueLines, parseLines } from "../lib/format";

export interface ServerFormState {
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

export function emptyServerForm(): ServerFormState {
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

export function formFromServer(server: McpServerEntry): ServerFormState {
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

export function ServerFormModal({
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
  const [templatesOpen, setTemplatesOpen] = useState(false);

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

        <div className="form-field">
          <span>
            Templates
            <button
              type="button"
              className="btn btn-sm btn-template-toggle"
              onClick={() => setTemplatesOpen((p) => !p)}
            >
              {templatesOpen ? "Hide" : "Browse"}
            </button>
          </span>
          {templatesOpen && (
            <div className="template-grid">
              {SERVER_TEMPLATES.map((t) => (
                <button
                  key={t.label}
                  type="button"
                  className="template-chip"
                  title={t.description}
                  onClick={() => {
                    setForm((f) => ({
                      ...f,
                      name: t.label.toLowerCase().replace(/\s+/g, "-"),
                      transport: t.transport,
                      command: t.command,
                      argsText: t.args,
                      envText: t.envNotes ? `# ${t.envNotes}` : "",
                      url: "",
                      headersText: "",
                    }));
                    setTemplatesOpen(false);
                  }}
                >
                  <span className="template-chip-name">{t.label}</span>
                  <span className="template-chip-desc">{t.description}</span>
                </button>
              ))}
            </div>
          )}
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
