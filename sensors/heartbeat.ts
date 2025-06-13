import { Experience } from "../lib/Experience.ts";
import { Sensor } from "../lib/Sensor.ts";

/**
 * HeartbeatSensor emits a message every baseInterval milliseconds
 * with a small random jitter. The message includes the current time.
 */
export class HeartbeatSensor extends Sensor<null> {
  private running = true;
  private timerId?: number;
  constructor(
    private readonly baseInterval = 10_000,
    private readonly jitter = 1_000,
  ) {
    super();
    this.schedule();
  }

  private schedule() {
    if (!this.running) return;
    const delta = Math.floor(Math.random() * (this.jitter * 2)) - this.jitter;
    const delay = this.baseInterval + delta;
    this.timerId = setTimeout(() => {
      const timeoclock = new Date().toLocaleTimeString();
      const experience: Experience<null> = {
        what: [],
        how: `It's ${timeoclock}, and I feel my heart beat.`,
      };
      this.subject.next(experience);
      this.schedule();
    }, delay);
  }

  feel(what: null): void {
    // No-op: HeartbeatSensor does not use external input.
  }

  stop() {
    this.running = false;
    if (this.timerId !== undefined) {
      clearTimeout(this.timerId);
    }
  }
}
