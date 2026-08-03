import { useCallback, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { REPO_URL } from "../lib/urlSafety";
import type { UpdateStatus } from "../components/AboutModal";

const RELEASES_API =
  "https://api.github.com/repos/StormShynn/mcp-switch/releases/latest";

type PendingUpdate = { version: string };

function parseSemver(input: string): number[] {
  return input
    .replace(/^v/, "")
    .split(".")
    .map((part) => parseInt(part, 10) || 0);
}

function compareSemver(a: string, b: string): number {
  const pa = parseSemver(a);
  const pb = parseSemver(b);
  const len = Math.max(pa.length, pb.length);
  for (let i = 0; i < len; i++) {
    const diff = (pa[i] ?? 0) - (pb[i] ?? 0);
    if (diff !== 0) return diff;
  }
  return 0;
}

export function useUpdater() {
  const [updateStatus, setUpdateStatus] = useState<UpdateStatus>("idle");
  const [pendingUpdate, setPendingUpdate] = useState<PendingUpdate | null>(null);
  const [updateError, setUpdateError] = useState("");

  const handleCheckForUpdates = useCallback(async () => {
    setUpdateStatus("checking");
    setUpdateError("");
    try {
      const currentVersion = await getVersion();
      const response = await fetch(RELEASES_API, {
        headers: { Accept: "application/vnd.github+json" },
      });
      if (!response.ok) {
        throw new Error(`GitHub API responded ${response.status}`);
      }
      const release = (await response.json()) as { tag_name?: string };
      const latestTag = String(release.tag_name ?? "").trim();
      if (!latestTag) {
        throw new Error("Latest release has no tag_name");
      }
      const latestVersion = latestTag.replace(/^v/, "");
      if (compareSemver(latestVersion, currentVersion) > 0) {
        setPendingUpdate({ version: latestVersion });
        setUpdateStatus("available");
      } else {
        setPendingUpdate(null);
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
