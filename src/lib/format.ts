export function parseLines(text: string): string[] | undefined {
  const lines = text
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean);
  return lines.length > 0 ? lines : undefined;
}

export function parseKeyValueLines(text: string): Record<string, string> | undefined {
  const out: Record<string, string> = {};
  for (const line of text.split("\n").map((l) => l.trim()).filter(Boolean)) {
    const idx = line.indexOf("=");
    if (idx <= 0) continue;
    out[line.slice(0, idx).trim()] = line.slice(idx + 1).trim();
  }
  return Object.keys(out).length > 0 ? out : undefined;
}

export function keyValueMapToLines(map: Record<string, string> | undefined): string {
  return Object.entries(map ?? {})
    .map(([k, v]) => `${k}=${v}`)
    .join("\n");
}
