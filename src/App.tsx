import { useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { APPS, APP_COLORS } from "./lib/types";
import { AboutModal } from "./components/AboutModal";
import { ServerFormModal, emptyServerForm, formFromServer } from "./components/ServerFormModal";
import { ServerRow } from "./components/ServerRow";
import { TrashRow } from "./components/TrashRow";
import { EmptyState } from "./components/EmptyState";
import { ToolBar } from "./components/ToolBar";
import { useNotifications } from "./hooks/useNotifications";
import { useServers, RESTARTABLE_APPS } from "./hooks/useServers";
import { useRunner } from "./hooks/useRunner";
import { useUpdater } from "./hooks/useUpdater";

let nextId = 1000;
function uniqueId(): number {
  return nextId++;
}

/* ── Main App ────────────────────────────────────── */
export default function App() {
  const [showAbout, setShowAbout] = useState(false);
  const [version, setVersion] = useState("");
  const [storePath, setStorePath] = useState("");

  const { notification, notify } = useNotifications();
  const servers = useServers(notify);
  const runner = useRunner(notify);
  const updater = useUpdater();

  const handleShowAbout = useCallback(async () => {
    setShowAbout(true);
    if (!version) {
      getVersion().then(setVersion).catch(() => setVersion("unknown"));
    }
    if (!storePath) {
      invoke<string>("get_store_path").then(setStorePath).catch(() => {});
    }
  }, [version, storePath]);

  return (
    <div className="app-container">
      {/* Header */}
      <header className="app-header">
        <div className="header-left">
          <h1 className="app-title">MCP Switch</h1>
          <span className="app-subtitle">
            {servers.servers.length} server{servers.servers.length !== 1 ? "s" : ""}
          </span>
        </div>
        <div className="header-actions">
          <button className="btn" onClick={handleShowAbout} title="About MCP Switch">
            About
          </button>
          <button className="btn" onClick={() => servers.setEditingServer("new")} title="Ctrl+N">
            Add server
          </button>
          <button
            className="btn btn-primary"
            onClick={servers.handleImport}
            disabled={servers.importing}
          >
            {servers.importing ? "Syncing…" : "Import"}
          </button>
          <button className="btn" onClick={servers.handleExport} title="Export servers to JSON file">
            Export
          </button>
          <button className="btn" onClick={servers.handleImportFromFile} title="Import servers from JSON file">
            Import file
          </button>
        </div>
      </header>

      {showAbout && (
        <AboutModal
          version={version}
          storePath={storePath}
          onClose={() => setShowAbout(false)}
          updateStatus={updater.updateStatus}
          updateVersion={updater.pendingUpdate?.version ?? ""}
          updateError={updater.updateError}
          onCheckForUpdates={updater.handleCheckForUpdates}
          onDownloadUpdate={updater.handleDownloadUpdate}
        />
      )}

      {servers.editingServer !== null && (
        <ServerFormModal
          initial={
            servers.editingServer === "new"
              ? emptyServerForm()
              : formFromServer(servers.editingServer)
          }
          onClose={() => servers.setEditingServer(null)}
          onSave={servers.handleSaveServer}
        />
      )}

      {/* Restart reminder */}
      {servers.pendingRestarts.size > 0 && (
        <div className="restart-banner">
          <span className="restart-banner-label">Restart to apply:</span>
          <div className="restart-banner-chips">
            {APPS.filter((a) => servers.pendingRestarts.has(a.id)).map((a) =>
              RESTARTABLE_APPS.has(a.id) ? (
                <span key={a.id} className="restart-chip restart-chip-actionable">
                  <button
                    className="restart-chip-action"
                    onClick={() => servers.handleRestartApp(a.id)}
                    title={`Kill and relaunch ${a.label}`}
                  >
                    <span style={{ color: APP_COLORS[a.id] }}>{a.label}</span>
                    <span> — Restart</span>
                  </button>
                  <button
                    className="restart-chip-x"
                    onClick={() => servers.dismissPendingRestart(a.id)}
                    title={`Dismiss — I already restarted ${a.label}`}
                  >
                    ×
                  </button>
                </span>
              ) : (
                <button
                  key={a.id}
                  className="restart-chip"
                  onClick={() => servers.dismissPendingRestart(a.id)}
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
      <ToolBar
        searchQuery={servers.searchQuery}
        onSearchChange={servers.setSearchQuery}
        searchInputRef={servers.searchRef}
        filter={servers.filter}
        onFilterChange={servers.setFilter}
        trashCount={servers.trashCount}
        sortKey={servers.sortKey}
        sortDir={servers.sortDir}
        onToggleSort={servers.toggleSort}
      />

      {/* Content */}
      <div className="app-content">
        {servers.loading ? (
          <div className="loading-state">
            <div className="spinner" />
            <p>Loading servers…</p>
          </div>
        ) : servers.sorted.length === 0 ? (
          servers.filter === "trash" ? (
            <div className="empty-state fade-in">
              <h2>Trash is empty</h2>
              <p>Servers removed from every tool that used to define them show up here.</p>
            </div>
          ) : (
            <EmptyState onImport={servers.handleImport} />
          )
        ) : (
          <div className="server-list">
            {servers.filter === "trash"
              ? servers.sorted.map((server, i) => (
                  <TrashRow
                    key={`${server.name}::${server.app}`}
                    server={server}
                    index={i}
                    onRestore={servers.handleRestore}
                    onDeleteForever={servers.handleDeleteForever}
                  />
                ))
              : servers.sorted.map((server, i) => (
                  <ServerRow
                    key={`${server.name}::${server.app}`}
                    server={server}
                    index={i}
                    onToggle={servers.handleToggle}
                    onEdit={servers.setEditingServer}
                    onTrash={servers.handleTrash}
                    onTest={servers.handleTestConnection}
                    onRun={runner.handleRun}
                    onStop={runner.handleStop}
                    onClone={servers.handleClone}
                    testResult={servers.testResults[`${server.name}::${server.app}`] ?? null}
                    runningInfo={runner.running[`${server.name}::${server.app}`] ?? null}
                    busy={runner.busyRunning.has(`${server.name}::${server.app}`)}
                    logOpen={runner.logVisible.has(`${server.name}::${server.app}`)}
                    logLines={runner.logPeek[`${server.name}::${server.app}`] ?? []}
                    onToggleLog={runner.handleToggleLog}
                    autoRunOn={runner.autoRun.has(`${server.name}::${server.app}`)}
                    lastExit={runner.lastExits[`${server.name}::${server.app}`] ?? null}
                    onToggleAutoRun={runner.handleToggleAutoRun}
                    restartPolicy={runner.restartPolicies[`${server.app}::${server.name}`] ?? { mode: "never" }}
                    onChangePolicy={runner.handleChangePolicy}
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
