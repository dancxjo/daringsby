import { Sensor } from "../lib/Sensor.ts";

/**
 * HeartbeatSensor emits a message every baseInterval milliseconds
 * with a small random jitter. The message includes the current time.
 */
export class HeartbeatSensor extends Sensor<string> {
  private running = true;
  private timerId?: number;
  constructor(
    private readonly baseInterval = 60_000,
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
      this.subject.next(`It's ${timeoclock}, and I feel my heart beat.`);
      this.schedule();
    }, delay);
  }

  stop() {
    this.running = false;
    if (this.timerId !== undefined) {
      clearTimeout(this.timerId);
    }
  }
}
