import { useCallback, useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { openUrl } from "@tauri-apps/plugin-opener";
import { REPO_URL } from "../lib/urlSafety";
import type { UpdateStatus } from "../components/AboutModal";

export function useUpdater() {
  const [updateStatus, setUpdateStatus] = useState<UpdateStatus>("idle");
  const [pendingUpdate, setPendingUpdate] = useState<Update | null>(null);
  const [updateError, setUpdateError] = useState("");

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

  const handleDownloadUpdate = useCallback(async () => {
    // No in-app signature-verified install: just hand the user to the
    // releases page and let them download/run the installer themselves.
    await openUrl(`${REPO_URL}/releases/latest`);
  }, []);

  return {
    updateStatus,
    pendingUpdate,
    updateError,
    handleCheckForUpdates,
    handleDownloadUpdate,
  };
}
