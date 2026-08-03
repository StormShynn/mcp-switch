import { openUrl } from "@tauri-apps/plugin-opener";
import { REPO_URL } from "../lib/urlSafety";

export type UpdateStatus = "idle" | "checking" | "up-to-date" | "available" | "error";

export function AboutModal({
  version,
  storePath,
  onClose,
  updateStatus,
  updateVersion,
  updateError,
  onCheckForUpdates,
  onDownloadUpdate,
}: {
  version: string;
  storePath: string;
  onClose: () => void;
  updateStatus: UpdateStatus;
  updateVersion: string;
  updateError: string;
  onCheckForUpdates: () => void;
  onDownloadUpdate: () => void;
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
          {updateStatus === "up-to-date" && (
            <span className="modal-update-actions">
              <span>Up to date</span>
              <button className="btn modal-link-btn" onClick={onCheckForUpdates}>
                Check again
              </button>
            </span>
          )}
          {updateStatus === "available" && (
            <button className="btn btn-primary" onClick={onDownloadUpdate}>
              Download v{updateVersion}
            </button>
          )}
          {updateStatus === "error" && (
            <span className="modal-update-actions">
              <span className="modal-update-error" title={updateError}>
                Check failed
              </span>
              <button className="btn modal-link-btn" onClick={onCheckForUpdates}>
                Retry
              </button>
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
