import { useEffect, useState } from "react";
import { libraryApi } from "../api/library.ts";
import type { TagPage, TagQuery, Thumbnail } from "../api/library-contracts.ts";
import { toApiError } from "../api/contracts.ts";
import type { ApiError } from "../api/contracts.ts";

export function useTagPage(query: TagQuery, revision: number) {
  const key = JSON.stringify(query);
  const [state, setState] = useState<{ page: TagPage; loading: boolean; error: ApiError | null }>({ page: { items: [], nextAfter: null }, loading: true, error: null });
  useEffect(() => {
    let active = true;
    setState({ page: { items: [], nextAfter: null }, loading: true, error: null });
    libraryApi.tags(JSON.parse(key) as TagQuery).then(page => { if (active) setState({ page, loading: false, error: null }); }, e => { if (active) setState({ page: { items: [], nextAfter: null }, loading: false, error: toApiError(e) }); });
    return () => { active = false; };
  }, [key, revision]);
  return state;
}
export function useThumbnails(ids: string[], revision = 0) {
  const key = JSON.stringify(ids);
  const [thumbnails, setThumbnails] = useState<Record<string, Thumbnail>>({});
  const [error, setError] = useState<ApiError | null>(null);
  useEffect(() => {
    let active = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    setThumbnails({}); setError(null);
    const all = JSON.parse(key) as string[];
    async function poll(requested: string[]) {
      if (!requested.length) return;
      try {
        const result = await libraryApi.thumbnails(requested);
        if (!active) return;
        setThumbnails(previous => ({ ...previous, ...Object.fromEntries(result.map(t => [t.mediaId, t])) }));
        const pending = result.filter(t => t.status === "pending" || t.status === "running").map(t => t.mediaId);
        if (pending.length) timer = setTimeout(() => { void poll(pending); }, 2000);
      } catch (e) { if (active) setError(toApiError(e)); }
    }
    void poll(all);
    return () => { active = false; if (timer) clearTimeout(timer); };
  }, [key, revision]);
  return { thumbnails, error };
}
