import { useState, useMemo } from "react";

export type SortDir = "asc" | "desc";

/** 通用列表：文本搜索(空格分词 AND) + 多筛选 + 点列排序。全部客户端，瞬时。 */
export function useList<T>(rows: T[], opts: {
  searchText?: (r: T) => string;
  sorts?: Record<string, (r: T) => number | string>;
  initialSort?: { key: string; dir: SortDir };
}) {
  const [search, setSearch] = useState("");
  const [filters, setFilters] = useState<Record<string, string>>({});
  const [matchers, setMatchers] = useState<Record<string, (r: T, v: string) => boolean>>({});
  const [sortKey, setSortKey] = useState(opts.initialSort?.key || "");
  const [sortDir, setSortDir] = useState<SortDir>(opts.initialSort?.dir || "desc");

  const setFilter = (key: string, value: string, match: (r: T, v: string) => boolean) => {
    setFilters((f) => ({ ...f, [key]: value }));
    setMatchers((m) => ({ ...m, [key]: match }));
  };

  const toggleSort = (key: string) => {
    if (sortKey === key) setSortDir((d) => (d === "asc" ? "desc" : "asc"));
    else { setSortKey(key); setSortDir("desc"); }
  };

  const view = useMemo(() => {
    const terms = search.trim().toLowerCase().split(/\s+/).filter(Boolean);
    let out = rows.filter((r) => {
      for (const [k, v] of Object.entries(filters)) {
        if (v && matchers[k] && !matchers[k](r, v)) return false;
      }
      if (terms.length && opts.searchText) {
        const hay = opts.searchText(r).toLowerCase();
        if (!terms.every((t) => hay.includes(t))) return false;
      }
      return true;
    });
    if (sortKey && opts.sorts?.[sortKey]) {
      const f = opts.sorts[sortKey];
      out = [...out].sort((a, b) => {
        const va = f(a), vb = f(b);
        const c = typeof va === "number" && typeof vb === "number" ? va - vb : String(va).localeCompare(String(vb), "zh");
        return sortDir === "asc" ? c : -c;
      });
    }
    return out;
  }, [rows, search, filters, matchers, sortKey, sortDir]); // eslint-disable-line

  return { view, search, setSearch, filters, setFilter, sortKey, sortDir, toggleSort };
}
