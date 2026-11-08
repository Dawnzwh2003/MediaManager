import type { PlaybackState, ProgressWrite } from "../api/viewer.ts";
// Kept type-only: model tests need neither Tauri nor a DOM.
export interface Position { positionSeconds: number; durationSeconds: number | null }
type Save = (request: ProgressWrite) => Promise<PlaybackState>;
export class ProgressSaver {
  private pending: Position | null = null;
  private failed: ProgressWrite | null = null;
  private running: Promise<void> | null = null;
  private sequence = 0;
  private version: number;
  private last: Position;
  constructor(privateToken: string, state: PlaybackState, save: Save) { this.token = privateToken; this.version = state.stateVersion; this.last = { positionSeconds: state.positionSeconds, durationSeconds: state.durationSeconds }; this.save = save; }
  private token: string;
  private save: Save;
  checkpoint(position: Position): Promise<void> {
    if (!Number.isFinite(position.positionSeconds) || position.positionSeconds < 0 || (position.durationSeconds !== null && (!Number.isFinite(position.durationSeconds) || position.durationSeconds <= 0))) return Promise.reject(new Error("Invalid playback position"));
    this.pending = { ...position }; return this.flush();
  }
  flush(): Promise<void> {
    if (this.running) return this.running.then(() => this.flush());
    if (!this.pending && !this.failed) return Promise.resolve();
    const task = this.drain(); this.running = task;
    void task.then(() => { this.running = null; }, () => { this.running = null; });
    return task;
  }
  private async drain() {
    while (this.failed || this.pending) {
      let request = this.failed;
      if (!request) {
        const position = this.pending!; this.pending = null;
        if (position.positionSeconds === this.last.positionSeconds && position.durationSeconds === this.last.durationSeconds) continue;
        request = { ...position, token: this.token, sequence: ++this.sequence, expectedVersion: this.version };
      }
      this.failed = request;
      const result = await this.save(request);
      this.version = result.stateVersion; this.last = { positionSeconds: result.positionSeconds, durationSeconds: result.durationSeconds }; this.failed = null;
    }
  }
}
export interface ImageTransform { scale: number; x: number; y: number }
export function fitScale(width: number, height: number, boxWidth: number, boxHeight: number): number { return Math.max(.02, Math.min(1, Math.max(1, boxWidth - 40) / Math.max(1, width), Math.max(1, boxHeight - 40) / Math.max(1, height))); }
export function clampPan(transform: ImageTransform, width: number, height: number, boxWidth: number, boxHeight: number): ImageTransform {
  const x = Math.max(0, (width * transform.scale - boxWidth) / 2), y = Math.max(0, (height * transform.scale - boxHeight) / 2);
  return { scale: transform.scale, x: Math.max(-x, Math.min(x, transform.x)), y: Math.max(-y, Math.min(y, transform.y)) };
}
export function zoomAt(transform: ImageTransform, scale: number, x = 0, y = 0): ImageTransform {
  scale = Math.max(.02, Math.min(16, scale)); const ratio = scale / transform.scale;
  return { scale, x: x - (x - transform.x) * ratio, y: y - (y - transform.y) * ratio };
}
export function formatTime(seconds: number): string { if (!Number.isFinite(seconds) || seconds < 0) return "0:00"; const n = Math.floor(seconds); return `${Math.floor(n / 60)}:${String(n % 60).padStart(2, "0")}`; }
