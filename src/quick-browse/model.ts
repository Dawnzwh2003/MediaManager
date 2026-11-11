import type { Activation } from "../api/activity.ts";
import type { BrowseItem, BrowseSession, BrowseSettings, ToggleTagRequest, ToggleTagResult } from "../api/quick-browse.ts";

export type QuickAction = "previous" | "next" | "like" | "tags" | "fullscreen" | "info" | "shuffle" | "exit" | number;
export function quickShortcut(event: Pick<KeyboardEvent, "key" | "code" | "repeat" | "ctrlKey" | "altKey" | "metaKey">): QuickAction | null {
  if (event.repeat || event.ctrlKey || event.altKey || event.metaKey) return null;
  if (/^[1-9]$/.test(event.key)) return Number(event.key);
  if (event.key === "ArrowLeft") return "previous";
  if (event.key === "ArrowRight" || event.key === " " || event.code === "Space") return "next";
  switch (event.key.toLowerCase()) {
    case "l": return "like";
    case "t": return "tags";
    case "f": return "fullscreen";
    case "i": return "info";
    case "r": return "shuffle";
    case "escape": return "exit";
    default: return null;
  }
}

export class TagShortcutQueue {
  private pending: ToggleTagRequest[] = [];
  private running: Promise<void> | null = null;
  private sessionId: string;
  private ordinal: number;
  private send: (request: ToggleTagRequest) => Promise<ToggleTagResult>;
  private feedback: (result: ToggleTagResult) => void;
  constructor(sessionId: string, ordinal: number, send: (request: ToggleTagRequest) => Promise<ToggleTagResult>, feedback: (result: ToggleTagResult) => void) {
    this.sessionId = sessionId;
    this.ordinal = ordinal;
    this.send = send;
    this.feedback = feedback;
  }
  toggle(digit: number): Promise<void> {
    this.pending.push({ requestId: crypto.randomUUID(), sessionId: this.sessionId, ordinal: this.ordinal, digit });
    return this.flush();
  }
  flush(): Promise<void> {
    if (this.running) return this.running.then(() => this.flush());
    if (!this.pending.length) return Promise.resolve();
    const operation = this.drain();
    this.running = operation;
    void operation.then(() => { this.running = null; }, () => { this.running = null; });
    return operation;
  }
  private async drain() {
    while (this.pending.length) {
      const result = await this.send(this.pending[0]!);
      this.pending.shift();
      this.feedback(result);
    }
  }
}

export interface QuickViewerContext {
  session: BrowseSession;
  item: BrowseItem;
  activation: Activation;
  settings: BrowseSettings;
  settingsOpen: boolean;
  navigate: (step: -1 | 1, flush: () => Promise<void>) => Promise<void>;
  shuffle: (flush: () => Promise<void>) => Promise<void>;
  fullscreen: () => void;
  openSettings: () => void;
  registerFlush: (flush: (() => Promise<void>) | null) => void;
}
