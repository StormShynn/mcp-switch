/**
 * Generates update.json for Tauri 2 updater.
 *
 * Usage:
 *   node scripts/generate-update-json.mjs <version> [artifacts-dir] [release-tag]
 *
 * - <version>: the version string (e.g. "0.1.0" or "0.1.0-nightly-123")
 * - [artifacts-dir]: directory with platform subfolders (default: "artifacts")
 * - [release-tag]: custom tag for download URL (default: "v<version>")
 *   e.g. "nightly" → downloads from .../releases/download/nightly/<file>
 *
 * The artifacts directory structure from actions/download-artifact@v4:
 *   artifacts/
 *     mcp-switch-aarch64-apple-darwin/    (bundle files + .sig)
 *     mcp-switch-x86_64-apple-darwin/     (bundle files + .sig)
 *     mcp-switch-x86_64-unknown-linux-gnu/ (bundle files + .sig)
 *     mcp-switch-x86_64-pc-windows-msvc/  (bundle files + .sig)
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

// Preferred bundle extensions per platform (first match wins)
const BUNDLE_RANK = [
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

// Find the primary bundle and its .sig file in the artifact directory
function findBundle(targetDir) {
  if (!fs.existsSync(targetDir)) return null;

  const entries = fs.readdirSync(targetDir, { withFileTypes: true });

  // Collect all files, separating bundles from .sig
  const sigs = new Map(); // basename without .sig -> sig path
  const bundles = [];

  function scan(dir) {
    const entries = fs.readdirSync(dir, { withFileTypes: true });
    for (const entry of entries) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        scan(full);
      } else if (entry.isFile()) {
        if (entry.name.endsWith(".sig")) {
          const stem = entry.name.slice(0, -4);
          sigs.set(stem, full);
        } else {
          bundles.push({ name: entry.name, path: full });
        }
      }
    }
  }

  scan(targetDir, "");

  // Try to match a bundle with its .sig
  for (const bundle of bundles) {
    if (sigs.has(bundle.name)) {
      const sigContent = fs.readFileSync(sigs.get(bundle.name), "utf8").trim();
      const urlName = bundle.name;
      return { signature: sigContent, url: `${BASE_URL}/${encodeURIComponent(urlName)}` };
    }
  }

  // Fallback: find the highest-ranked bundle even without .sig
  for (const ext of BUNDLE_RANK) {
    const match = bundles.find((b) => b.name.endsWith(ext));
    if (match) {
      const sigContent = sigs.get(match.name) 
        ? fs.readFileSync(sigs.get(match.name), "utf8").trim()
        : "";
      const urlName = match.name;
      return { signature: sigContent, url: `${BASE_URL}/${encodeURIComponent(urlName)}` };
    }
  }

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
  } else {
    console.warn(`Warning: No bundle found for ${target} in ${targetDir}`);
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
