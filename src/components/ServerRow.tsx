import type { AppId, ConnectionTestResult, McpServerEntry, RestartPolicy, RunningServer } from "../lib/types";
import { APPS, APP_COLORS } from "../lib/types";

export function ServerRow({
  server,
  index,
  onToggle,
  onEdit,
  onTrash,
  onTest,
  onClone,
  testResult,
  runningInfo,
  onRun,
  onStop,
  busy,
  logOpen,
  logLines,
  onToggleLog,
  autoRunOn,
  lastExit,
  onToggleAutoRun,
  restartPolicy,
  onChangePolicy,
}: {
  server: McpServerEntry;
  index: number;
  onToggle: (serverName: string, appId: AppId, enabled: boolean) => void;
  onEdit: (server: McpServerEntry) => void;
  onTrash: (serverName: string, appId: AppId) => void;
  onTest: (serverName: string, appId: AppId) => void;
  onClone: (server: McpServerEntry) => void;
  testResult: { status: "testing" } | ConnectionTestResult | null;
  runningInfo: RunningServer | null;
  onRun: (serverName: string, appId: AppId) => void;
  onStop: (serverName: string, appId: AppId) => void;
  busy: boolean;
  logOpen: boolean;
  logLines: string[];
  onToggleLog: (serverName: string, appId: AppId) => void;
  autoRunOn: boolean;
  lastExit: number | null;
  onToggleAutoRun: (serverName: string, appId: AppId) => void;
  restartPolicy: RestartPolicy;
  onChangePolicy: (serverName: string, appId: AppId, policy: RestartPolicy) => void;
}) {
  const appLabel = APPS.find((a) => a.id === server.app)?.label ?? server.app;

  const configLines: string[] = [];
  if (server.transport === "stdio") {
    configLines.push(`Command: ${server.command ?? ""}`);
    if (server.args?.length) configLines.push(`Args: ${server.args.join(" ")}`);
    if (server.env) configLines.push(`Env: ${Object.keys(server.env).length} variables`);
  } else {
    configLines.push(`URL: ${server.url ?? ""}`);
    if (server.headers) configLines.push(`Headers: ${Object.keys(server.headers).length} headers`);
  }
  configLines.push(`Transport: ${server.transport}`);

  return (
    <div
      className="server-row fade-in server-row-clickable"
      style={{ animationDelay: `${index * 30}ms` }}
      onClick={() => onEdit(server)}
      title="Click to edit"
    >
      <div className="server-info">
        <div className="server-name">
          {server.name}
          <span className="server-info-icon" title={configLines.join("\n")}>ⓘ</span>
        </div>
        <div className="server-command">
          {server.transport === "stdio" ? server.command : server.url}
        </div>
        {lastExit !== null && (
          <span className="badge server-crashed-badge" title={`Last child exited with code ${lastExit}`}>
          </span>
        )}
        {runningInfo && runningInfo.restartCount > 0 && (
          <span
            className="badge server-restart-badge"
            title={`Auto-restarted ${runningInfo.restartCount}× — most recent spawn ${new Date(runningInfo.lastStartedAt * 1000).toLocaleString()}`}
          >
            ↻ {runningInfo.restartCount}
          </span>
        )}
        {server.transport === "stdio" && (
          <label
            className="auto-run-toggle"
            title="Auto-spawn when MCP Switch launches."
            onClick={(e) => e.stopPropagation()}
          >
            <input
              type="checkbox"
              checked={autoRunOn}
              onChange={() => onToggleAutoRun(server.name, server.app)}
            />
            <span className="auto-run-toggle-label">auto-run</span>
          </label>
        )}
        {server.transport === "stdio" && (
          <select
            className="restart-policy-select"
            title="Auto-restart policy when this server's process exits"
            value={restartPolicy.mode}
            onClick={(e) => e.stopPropagation()}
            onChange={(e) => {
              const mode = e.target.value as RestartPolicy["mode"];
              const policy: RestartPolicy =
                mode === "never" ? { mode } : { mode, maxRetries: 5, backoffMs: 1000 };
              onChangePolicy(server.name, server.app, policy);
            }}
          >
            <option value="never">No auto-restart</option>
            <option value="onFailure">Restart on failure</option>
            <option value="always">Always restart</option>
          </select>
        )}
        <div className="server-meta">
          <span className="badge" style={{ color: APP_COLORS[server.app] }}>
            {appLabel}
          </span>
          {testResult && "success" in testResult && (
            <span className={`badge test-badge ${testResult.success ? "test-badge-ok" : "test-badge-fail"}`}>
              {testResult.success ? "OK" : "FAIL"}
            </span>
          )}
        </div>
      </div>

      <div className="server-toggles">
        <button
          className="btn btn-sm btn-test"
          title="Test MCP connection"
          disabled={testResult !== null && "status" in testResult}
          onClick={(e) => {
            e.stopPropagation();
            onTest(server.name, server.app);
          }}
        >
          {testResult !== null && "status" in testResult ? (
            <span className="test-spinner" />
          ) : (
            "Test"
          )}
        </button>
        <button
          className={runningInfo ? "btn btn-sm btn-stop" : "btn btn-sm btn-run"}
          title={runningInfo ? "Stop pid " + runningInfo.pid : (server.transport === "stdio" ? "Run as detached child of MCP Switch" : "Only stdio servers can be run from MCP Switch")}
          disabled={busy || server.transport !== "stdio"}
          onClick={(e) => {
            e.stopPropagation();
            if (runningInfo !== null) {
              onStop(server.name, server.app);
            } else {
              onRun(server.name, server.app);
            }
          }}
        >
          {runningInfo ? "Stop" : busy ? "…" : "Run"}
        </button>
        <button
          className={"btn btn-sm btn-log" + (logOpen ? " btn-log-open" : "")}
          title="Show captured stdout/stderr (last 100 lines)"
          disabled={!runningInfo}
          onClick={(e) => {
            e.stopPropagation();
            onToggleLog(server.name, server.app);
          }}
        >
          Log
        </button>
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
          className="btn btn-sm"
          title="Clone to another app"
          onClick={(e) => {
            e.stopPropagation();
            onClone(server);
          }}
        >
          Clone
        </button>
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
      {logOpen && runningInfo && (
        <div className="server-log-peek" onClick={(e) => e.stopPropagation()}>
          <div className="server-log-peek-header">
            Captured stdout/stderr — pid {runningInfo.pid} {runningInfo.command} {runningInfo.args.join(" ")}
          </div>
          <pre className="server-log-peek-body">
            {logLines.length === 0 ? "(no output yet)" : logLines.join("\n")}
          </pre>
        </div>
      )}
    </div>
  );
}
