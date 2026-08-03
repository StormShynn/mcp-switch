import type { Transport } from "./types";
import { keyValueMapToLines } from "./format";

export interface ParsedServerJson {
  name?: string;
  transport?: Transport;
  command?: string;
  argsText?: string;
  envText?: string;
  url?: string;
  headersText?: string;
}

function asStringArray(value: unknown): string[] | undefined {
  if (!Array.isArray(value)) return undefined;
  const items = value.filter((v): v is string => typeof v === "string");
  return items.length > 0 ? items : undefined;
}

function asStringRecord(value: unknown): Record<string, string> | undefined {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return undefined;
  const out: Record<string, string> = {};
  for (const [k, v] of Object.entries(value as Record<string, unknown>)) {
    if (typeof v === "string") out[k] = v;
    else if (typeof v === "number" || typeof v === "boolean") out[k] = String(v);
  }
  return Object.keys(out).length > 0 ? out : undefined;
}

function looksLikeServerEntry(value: unknown): value is Record<string, unknown> {
  return (
    typeof value === "object" &&
    value !== null &&
    !Array.isArray(value) &&
    ("command" in value || "url" in value || "httpUrl" in value || "serverUrl" in value)
  );
}

/** Parses `raw` as JSON. If that fails and `raw` doesn't already look like a
 * complete object/array (e.g. the user copied just `"name": { ... }` from
 * inside their real config's `mcpServers` block, braces and all left
 * behind), retries after wrapping it in `{ }` — stripping a trailing comma
 * first, since a copied middle-of-object entry often has one. Throws the
 * *original* error when even that doesn't parse, since it's more likely to
 * point at the real problem than an error from the synthetic wrapper. */
function tryParseJson(raw: string): unknown {
  try {
    return JSON.parse(raw);
  } catch (firstErr) {
    const trimmed = raw.trim();
    if (trimmed.startsWith("{") || trimmed.startsWith("[")) throw firstErr;
    try {
      return JSON.parse(`{${trimmed.replace(/,\s*$/, "")}}`);
    } catch {
      throw firstErr instanceof Error ? firstErr : new Error("Invalid JSON");
    }
  }
}

/** Recognizes the JSON shapes MCP servers are commonly documented in — a
 * bare entry (`{"command": "npx", ...}`), a full `{"mcpServers": {name:
 * {...}}}` block copied from another tool's config, or a single `{name:
 * {...}}` pair — and pulls out the fields the form needs. Returns null when
 * `raw` doesn't look like a server config at all. */
export function extractServerConfig(raw: string): ParsedServerJson | null {
  const parsed: unknown = tryParseJson(raw);
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
    throw new Error("Expected a JSON object");
  }

  let name: string | undefined;
  let body: Record<string, unknown> = parsed as Record<string, unknown>;

  const mcpServers = body.mcpServers;
  if (typeof mcpServers === "object" && mcpServers !== null && !Array.isArray(mcpServers)) {
    const [firstName, firstValue] = Object.entries(mcpServers)[0] ?? [];
    if (firstValue !== undefined && looksLikeServerEntry(firstValue)) {
      name = firstName;
      body = firstValue;
    }
  } else if (!looksLikeServerEntry(body)) {
    const [firstName, firstValue] = Object.entries(body)[0] ?? [];
    if (firstValue !== undefined && looksLikeServerEntry(firstValue)) {
      name = firstName;
      body = firstValue;
    }
  }

  if (!looksLikeServerEntry(body)) return null;

  let command: string | undefined;
  let args: string[] | undefined;
  if (Array.isArray(body.command)) {
    const [cmd, ...rest] = body.command;
    if (typeof cmd === "string") command = cmd;
    args = asStringArray(rest);
  } else if (typeof body.command === "string") {
    command = body.command;
    args = asStringArray(body.args);
  }

  const url = [body.url, body.httpUrl, body.serverUrl].find(
    (v): v is string => typeof v === "string"
  );
  const rawType =
    typeof body.type === "string"
      ? body.type
      : typeof body.transport === "string"
      ? body.transport
      : undefined;
  const transport: Transport | undefined = command
    ? "stdio"
    : url
    ? rawType === "http" || rawType === "streamable-http"
      ? "http"
      : "sse"
    : undefined;

  const env = asStringRecord(body.env ?? body.environment);
  const headers = asStringRecord(body.headers ?? body.http_headers);

  return {
    name,
    transport,
    command,
    argsText: args ? args.join("\n") : undefined,
    envText: env ? keyValueMapToLines(env) : undefined,
    url,
    headersText: headers ? keyValueMapToLines(headers) : undefined,
  };
}
