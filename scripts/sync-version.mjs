/**
 * Syncs the new release version across all project files.
 *
 * Usage:
 *   node scripts/sync-version.mjs <new-version>
 *
 * Updates:
 *   - package.json          → "version"
 *   - src-tauri/Cargo.toml  → [package].version
 *   - package-lock.json     → "version"
 *   - src-tauri/tauri.conf.json → "version"
 */

import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const newVersion = process.argv[2];
if (!newVersion) {
  console.error("Usage: node scripts/sync-version.mjs <new-version>");
  process.exit(1);
}

const rootDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

// ------- package.json -------
const pkgPath = path.join(rootDir, "package.json");
const pkg = JSON.parse(fs.readFileSync(pkgPath, "utf8"));
pkg.version = newVersion;
fs.writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + "\n");
console.log(`✓ Updated package.json → ${newVersion}`);

// ------- src-tauri/Cargo.toml -------
const cargoPath = path.join(rootDir, "src-tauri", "Cargo.toml");
let cargo = fs.readFileSync(cargoPath, "utf8");
cargo = cargo.replace(/^version\s*=\s*"[^"]+"/m, `version = "${newVersion}"`);
fs.writeFileSync(cargoPath, cargo);
console.log(`✓ Updated Cargo.toml → ${newVersion}`);

// ------- package-lock.json -------
const lockPath = path.join(rootDir, "package-lock.json");
const lock = JSON.parse(fs.readFileSync(lockPath, "utf8"));
if (lock.version) {
  lock.version = newVersion;
}
if (lock.packages?.[""]?.version) {
  lock.packages[""].version = newVersion;
}
fs.writeFileSync(lockPath, JSON.stringify(lock, null, 2) + "\n");
console.log(`✓ Updated package-lock.json → ${newVersion}`);

// ------- src-tauri/tauri.conf.json -------
const tauriConfPath = path.join(rootDir, "src-tauri", "tauri.conf.json");
const tauriConf = JSON.parse(fs.readFileSync(tauriConfPath, "utf8"));
tauriConf.version = newVersion;
fs.writeFileSync(tauriConfPath, JSON.stringify(tauriConf, null, 2) + "\n");
console.log(`✓ Updated tauri.conf.json → ${newVersion}`);

console.log("\nDone! All version files synced.");
