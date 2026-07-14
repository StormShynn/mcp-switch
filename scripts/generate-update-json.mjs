/**
 * Generates update.json for Tauri 2 updater.
 *
 * Usage:
 *   node scripts/generate-update-json.mjs <version> [artifacts-dir] [release-tag]
 *
 * - <version>: the version string (e.g. "0.4.4" or "0.1.0-nightly-123")
 * - [artifacts-dir]: directory with platform subfolders (default: "artifacts")
 * - [release-tag]: custom tag for download URL (default: "v<version>")
 *   e.g. "nightly" → downloads from .../releases/download/nightly/<file>
 *
 * The artifacts directory structure from actions/download-artifact@v4:
 *   artifacts/
 *     mcp-switch-aarch64-apple-darwin/    (installer files + .sig)
 *     mcp-switch-x86_64-apple-darwin/     (installer files + .sig)
 *     mcp-switch-x86_64-unknown-linux-gnu/ (installer files + .sig)
 *     mcp-switch-x86_64-pc-windows-msvc/  (installer files + .sig)
 *
 * Outputs update.json to stdout.
 */

import fs from "fs";
import path from "path";

const version = process.argv[2];
const artifactsDir = process.argv[3] || "artifacts";
const releaseTag = process.argv[4] || `v${version}`;

if (!version) {
  console.error("Usage: node generate-update-json.mjs <version> [artifacts-dir] [release-tag]");
  process.exit(1);
}

const REPO = "StormShynn/mcp-switch";
const BASE_URL = `https://github.com/${REPO}/releases/download/${releaseTag}`;

// Target triple -> Tauri 2 platform key
const PLATFORM_MAP = {
  "aarch64-apple-darwin": "darwin-aarch64",
  "x86_64-apple-darwin": "darwin-x86_64",
  "x86_64-unknown-linux-gnu": "linux-x86_64",
  "x86_64-pc-windows-msvc": "windows-x86_64",
};

// Preferred bundle extensions per platform, in priority order
const BUNDLE_EXT_RANK = [
  ".dmg",
  ".AppImage",
  ".deb",
  ".msi",
  ".exe",
  ".app.tar.gz",
];

// Artifact subdirectory name pattern
function artifactDir(target) {
  return `mcp-switch-${target}`;
}

// Find the primary bundle and its .sig file in a flat artifact directory
function findBundle(targetDir) {
  if (!fs.existsSync(targetDir)) {
    console.warn(`Warning: Directory not found: ${targetDir}`);
    return null;
  }

  const entries = fs.readdirSync(targetDir, { withFileTypes: true });

  // Collect files (non-recursive — artifacts are now flat)
  const sigs = new Map(); // basename without .sig -> sig content
  const bundles = [];

  for (const entry of entries) {
    if (!entry.isFile()) continue;
    const full = path.join(targetDir, entry.name);
    if (entry.name.endsWith(".sig")) {
      const stem = entry.name.slice(0, -4);
      sigs.set(stem, fs.readFileSync(full, "utf8").trim());
    } else {
      bundles.push({ name: entry.name, path: full });
    }
  }

  // Try to match a bundle with its .sig (same stem)
  for (const bundle of bundles) {
    const sig = sigs.get(bundle.name);
    if (sig) {
      return { signature: sig, url: `${BASE_URL}/${encodeURIComponent(bundle.name)}` };
    }
  }

  // Fallback: highest-ranked bundle even without .sig
  for (const ext of BUNDLE_EXT_RANK) {
    const match = bundles.find((b) => b.name.endsWith(ext));
    if (match) {
      return {
        signature: sigs.get(match.name) || "",
        url: `${BASE_URL}/${encodeURIComponent(match.name)}`,
      };
    }
  }

  console.warn(`Warning: No supported bundle found in ${targetDir}`);
  return null;
}

// Collect platforms
const platforms = {};
const pubDate = new Date().toISOString();

for (const [target, platformKey] of Object.entries(PLATFORM_MAP)) {
  const targetDir = path.join(artifactsDir, artifactDir(target));
  const info = findBundle(targetDir);
  if (info) {
    platforms[platformKey] = info;
  }
}

// Build the update manifest
const manifest = {
  version,
  notes: `Release v${version}`,
  pub_date: pubDate,
  platforms,
};

console.log(JSON.stringify(manifest, null, 2));
