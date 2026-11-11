import type { LibraryCursor, LibraryFilter, LibraryMedia, LibraryPage } from "../api/library-contracts.ts";
import { initialFilter } from "../api/library-contracts.ts";
import { toApiError } from "../api/contracts.ts";
import type { ApiError } from "../api/contracts.ts";

export const PAGE_SIZE = 100;
export const ROW_HEIGHT = 268;
export const MAX_SELECTION = 200;
export const MAX_FILTER_TAGS = 20;
const RANDOM_SEED_KEY = "media-manager.library-random-v1";
type SeedStorage = Pick<Storage, "getItem" | "setItem">;

function browserStorage(): SeedStorage | undefined {
  if (typeof window === "undefined") return undefined;
  try { return globalThis.localStorage; } catch { return undefined; }
}

function savedSeed(storage: SeedStorage | undefined): string {
  try {
    const seed = storage?.getItem(RANDOM_SEED_KEY);
    if (seed && /^[a-f0-9]{32}$/.test(seed)) return seed;
  } catch { /* Storage is optional in a restricted WebView. */ }
  return initialFilter.random.seed;
}

export function customFilterIds(filter: LibraryFilter): string[] {
  return [...new Set([...filter.customTagIds, ...(filter.customTagId ? [filter.customTagId] : [])])];
}

export function hasLibraryFilters(filter: LibraryFilter): boolean {
  return Boolean(filter.search || filter.mediaType || filter.systemTagId || customFilterIds(filter).length
    || filter.likes || filter.views || filter.plays || filter.imported || filter.viewed !== "all");
}
export function virtualRows(count: number, columns: number, scrollTop: number, height: number) {
  columns = Math.max(1, Math.floor(columns));
  const rows = Math.ceil(count / columns);
  const first = Math.max(0, Math.min(rows, Math.floor(Math.max(0, scrollTop) / ROW_HEIGHT) - 2));
  const last = Math.min(rows, Math.ceil((Math.max(0, scrollTop) + height) / ROW_HEIGHT) + 2);
  return { start: Math.min(count, first * columns), end: Math.min(count, last * columns), offset: first * ROW_HEIGHT, totalHeight: rows * ROW_HEIGHT };
}
export function toggleSelection(selected: ReadonlySet<string>, id: string): Set<string> {
  const next = new Set(selected);
  if (next.has(id)) next.delete(id); else if (next.size < MAX_SELECTION) next.add(id);
  return next;
}
export interface LibrarySnapshot { items: LibraryMedia[]; filter: LibraryFilter; loading: boolean; error: ApiError | null; next: LibraryCursor | null; previous: boolean; page: number }
type Query = (filter: LibraryFilter, cursor: LibraryCursor | null, limit: number) => Promise<LibraryPage>;
export class LibraryController {
  private snapshot: LibrarySnapshot = { items: [], filter: { ...initialFilter }, loading: true, error: null, next: null, previous: false, page: 1 };
  private listeners = new Set<() => void>();
  private trail: (LibraryCursor | null)[] = [];
  private current: LibraryCursor | null = null;
  private serial = 0;
  private query: Query;
  private storage: SeedStorage | undefined;
  constructor(query: Query, storage = browserStorage()) {
    this.query = query;
    this.storage = storage;
    this.snapshot.filter.random = { seed: savedSeed(storage), version: 1 };
  }
  getSnapshot = () => this.snapshot;
  subscribe = (listener: () => void) => { this.listeners.add(listener); return () => { this.listeners.delete(listener); }; };
  private publish(patch: Partial<LibrarySnapshot>) { this.snapshot = { ...this.snapshot, ...patch }; this.listeners.forEach(f => f()); }
  async load(filter: LibraryFilter, cursor: LibraryCursor | null, trail: (LibraryCursor | null)[], page: number) {
    const serial = ++this.serial;
    this.publish({ filter, items: [], loading: true, error: null });
    try {
      const result = await this.query(filter, cursor, PAGE_SIZE);
      if (serial !== this.serial) return;
      this.current = cursor; this.trail = trail;
      this.publish({ items: result.items, next: result.nextCursor, previous: trail.length > 0, page, loading: false });
    } catch (error) {
      if (serial !== this.serial) return;
      this.publish({ items: [], next: null, loading: false, error: toApiError(error) });
    }
  }
  refresh = () => this.load(this.snapshot.filter, null, [], 1);
  change = (filter: LibraryFilter) => this.load({ ...filter, random: this.snapshot.filter.random }, null, [], 1);
  shuffle = () => {
    const seed = globalThis.crypto.randomUUID().replaceAll("-", "");
    try { this.storage?.setItem(RANDOM_SEED_KEY, seed); } catch { /* Keep the current session's seed. */ }
    return this.load({ ...this.snapshot.filter, sort: "random", random: { seed, version: 1 } }, null, [], 1);
  };
  next = () => { if (!this.snapshot.loading && this.snapshot.next) return this.load(this.snapshot.filter, this.snapshot.next, [...this.trail, this.current].slice(-256), this.snapshot.page + 1); };
  previous = () => { if (!this.snapshot.loading && this.trail.length) return this.load(this.snapshot.filter, this.trail[this.trail.length - 1] ?? null, this.trail.slice(0, -1), this.snapshot.page - 1); };
  cancel = () => { ++this.serial; };
}
