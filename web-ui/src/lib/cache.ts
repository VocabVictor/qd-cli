import { useEffect, useState, useRef, useCallback } from "react";

const store = new Map<string, unknown>();

/** stale-while-revalidate: 命中缓存立即返回旧数据(不显示 loading),后台静默刷新。 */
export function useCached<T>(key: string, fetcher: () => Promise<T>, deps: unknown[] = []) {
  const cached = store.get(key) as T | undefined;
  const [data, setData] = useState<T | undefined>(cached);
  const [loading, setLoading] = useState(cached === undefined);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string>("");
  const fnRef = useRef(fetcher);
  fnRef.current = fetcher;

  const run = useCallback(async (silent: boolean) => {
    if (silent) setRefreshing(true); else setLoading(store.get(key) === undefined);
    setError("");
    try {
      const d = await fnRef.current();
      store.set(key, d);
      setData(d);
    } catch (e: any) {
      if (store.get(key) === undefined) setError(e?.message || String(e));
    } finally { setLoading(false); setRefreshing(false); }
  }, [key]);

  useEffect(() => {
    const has = store.get(key) !== undefined;
    if (has) { setData(store.get(key) as T); setLoading(false); run(true); }
    else run(false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, ...deps]);

  const refresh = useCallback(() => run(store.get(key) !== undefined), [run, key]);
  return { data, loading, refreshing, error, refresh, setData };
}

export const dropCache = (prefix?: string) => {
  if (!prefix) return store.clear();
  for (const k of store.keys()) if (k.startsWith(prefix)) store.delete(k);
};
