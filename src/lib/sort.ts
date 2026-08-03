import type { AppId, McpServerEntry } from "./types";

export type SortKey = "name" | "status";
export type SortDir = "asc" | "desc";
export type FilterKey = "all" | AppId | "trash";

export function sortServers(
  servers: McpServerEntry[],
  key: SortKey,
  dir: SortDir,
  filter: FilterKey
): McpServerEntry[] {
  let filtered: McpServerEntry[];
  if (filter === "trash") {
    filtered = servers.filter((s) => s.deleted);
  } else if (filter === "all") {
    filtered = servers.filter((s) => !s.deleted);
  } else {
    filtered = servers.filter((s) => !s.deleted && s.app === filter);
  }

  return [...filtered].sort((a, b) => {
    let cmp: number;
    if (key === "name") {
      cmp = a.name.localeCompare(b.name);
    } else {
      cmp = Number(b.enabled) - Number(a.enabled);
    }
    return dir === "asc" ? cmp : -cmp;
  });
}
