import { openUrl } from "@tauri-apps/plugin-opener";

export const REPO_URL = "https://github.com/StormShynn/mcp-switch";

/** Hard-coded allowlist of URLs the app is allowed to hand to the OS shell.
 *
 * `opener:allow-open-url` is a powerful capability: any URL the frontend
 * passes to `openUrl` becomes a process the OS launches. We never take a URL
 * from user input today, but defense in depth: if a future feature starts
 * passing a less-trusted string here, this allowlist blocks it from becoming
 * an arbitrary command/shell launch vector before the call ever leaves the
 * renderer.
 */
const OPEN_URL_ALLOWLIST: readonly URL[] = [new URL(REPO_URL)];

export function isAllowedExternalUrl(target: string): boolean {
  let parsed: URL;
  try {
    parsed = new URL(target);
  } catch {
    return false;
  }
  if (parsed.protocol !== "https:" && parsed.protocol !== "http:") {
    return false;
  }
  return OPEN_URL_ALLOWLIST.some(
    (allowed) =>
      allowed.protocol === parsed.protocol &&
      allowed.host === parsed.host &&
      (allowed.pathname === parsed.pathname ||
        parsed.pathname.startsWith(allowed.pathname.replace(/\/$/, "") + "/"))
  );
}

export async function safeOpenUrl(target: string): Promise<void> {
  if (!isAllowedExternalUrl(target)) {
    console.warn(`Refusing to open URL outside allowlist: ${target}`);
    return;
  }
  await openUrl(target);
}
