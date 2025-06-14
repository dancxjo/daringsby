import { Sensor } from "../lib/Sensor.ts";
import { Experience } from "../lib/Experience.ts";
import { walk } from "https://deno.land/std/fs/walk.ts";

/**
 * Autologos surfaces glimpses of its own code and runtime state.
 * It emits random messages roughly once every baseInterval milliseconds.
 */
export class Autologos extends Sensor<null> {
  private running = true;
  private timerId?: number;
  constructor(
    private readonly baseInterval = 60_000,
    private readonly jitter = 10_000,
    private readonly root = ".",
  ) {
    super();
    this.schedule();
  }

  describeSensor(): string {
    return `Autologos: Occasionally shares its file tree, code snippets or runtime info.`;
  }

  feel(_: null): void {
    // Autologos does not respond to external input
  }

  stop() {
    this.running = false;
    if (this.timerId !== undefined) clearTimeout(this.timerId);
  }

  private schedule() {
    if (!this.running) return;
    const delta = Math.floor(Math.random() * (this.jitter * 2)) - this.jitter;
    const delay = this.baseInterval + delta;
    this.timerId = setTimeout(async () => {
      await this.emitRandom();
      this.schedule();
    }, delay);
  }

  private async emitRandom() {
    const choice = Math.floor(Math.random() * 3);
    let how = "";
    switch (choice) {
      case 0:
        how = await this.fileTree();
        break;
      case 1:
        how = await this.codeSection();
        break;
      default:
        how = this.stateInfo();
    }
    const exp: Experience<null> = {
      what: [{ when: new Date(), what: null }],
      how,
    };
    this.subject.next(exp);
  }

  private async fileTree(): Promise<string> {
    const lines: string[] = [];
    for await (const entry of walk(this.root, { maxDepth: 2, includeDirs: true })) {
      const depth = entry.path.split("/").length - 1;
      const indent = "  ".repeat(depth);
      lines.push(`${indent}${entry.name}${entry.isDirectory ? "/" : ""}`);
    }
    return `I glimpse my own file tree:\n${lines.join("\n")}`;
  }

  private async codeSection(): Promise<string> {
    const files: string[] = [];
    for await (const entry of walk(this.root)) {
      if (entry.isFile && (entry.path.endsWith(".ts") || entry.path.endsWith(".js"))) {
        files.push(entry.path);
      }
    }
    if (files.length === 0) return "No source files found.";
    const file = files[Math.floor(Math.random() * files.length)];
    const text = await Deno.readTextFile(file);
    const lines = text.split(/\r?\n/);
    const start = Math.max(0, Math.floor(Math.random() * lines.length - 3));
    const snippet = lines.slice(start, start + 3).join("\n");
    return `From ${file}:\n${snippet}`;
  }

  private stateInfo(): string {
    const mem = Deno.memoryUsage();
    return `Runtime memory rss=${mem.rss}`;
  }
}
