import type { RefObject } from "react";
import { APPS } from "../lib/types";
import type { FilterKey, SortDir, SortKey } from "../lib/sort";

export function ToolBar({
  searchQuery,
  onSearchChange,
  searchInputRef,
  filter,
  onFilterChange,
  trashCount,
  sortKey,
  sortDir,
  onToggleSort,
}: {
  searchQuery: string;
  onSearchChange: (value: string) => void;
  searchInputRef: RefObject<HTMLInputElement>;
  filter: FilterKey;
  onFilterChange: (filter: FilterKey) => void;
  trashCount: number;
  sortKey: SortKey;
  sortDir: SortDir;
  onToggleSort: (key: SortKey) => void;
}) {
  const sortArrow = (key: SortKey) =>
    sortKey === key ? (sortDir === "asc" ? " ▲" : " ▼") : "";

  return (
    <div className="toolbar">
      <div className="toolbar-search">
        <input
          ref={searchInputRef}
          type="text"
          className="search-input"
          placeholder="Search servers… (Ctrl+F)"
          value={searchQuery}
          onChange={(e) => onSearchChange(e.target.value)}
        />
        {searchQuery && (
          <button
            className="search-clear"
            onClick={() => {
              onSearchChange("");
              searchInputRef.current?.focus();
            }}
          >
            ×
          </button>
        )}
      </div>
      <div className="filter-group">
        {(["all", ...APPS.map((a) => a.id), "trash"] as FilterKey[]).map((f) => (
          <button
            key={f}
            className={`filter-chip ${filter === f ? "active" : ""} ${f === "trash" ? "filter-chip-trash" : ""}`}
            onClick={() => onFilterChange(f)}
          >
            {f === "all"
              ? "All"
              : f === "trash"
              ? `Trash${trashCount > 0 ? ` (${trashCount})` : ""}`
              : APPS.find((a) => a.id === f)?.label.split(" ")[0] ?? f}
          </button>
        ))}
      </div>
      <div className="sort-group">
        <button
          className={`btn btn-sort ${sortKey === "name" ? "active" : ""}`}
          onClick={() => onToggleSort("name")}
        >
          Name{sortArrow("name")}
        </button>
        <button
          className={`btn btn-sort ${sortKey === "status" ? "active" : ""}`}
          onClick={() => onToggleSort("status")}
        >
          Status{sortArrow("status")}
        </button>
      </div>
    </div>
  );
}
