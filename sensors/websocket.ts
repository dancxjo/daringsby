export type WebSocketWhat =
  | { type: "connect"; remote: string }
  | { type: "disconnect"; remote: string }
  | { type: "message"; remote: string; name: string; message: string };

import { Sensor } from "../lib/Sensor.ts";
import { Experience } from "../lib/Experience.ts";

/**
 * WebSocketSensor emits experiences for websocket events.
 *
 * ```ts
 * const sensor = new WebSocketSensor();
 * sensor.connected("1.1.1.1");
 * sensor.received("1.1.1.1", "hi");
 * ```
 */
export class WebSocketSensor extends Sensor<WebSocketWhat> {
  feel(what: WebSocketWhat): void {
    let how: string;
    switch (what.type) {
      case "connect":
        how = `Client ${what.remote} connected.`;
        break;
      case "disconnect":
        how = `Client ${what.remote} disconnected.`;
        break;
      case "message":
        how = `${what.name} says: ${what.message}`;
        break;
    }
    const exp: Experience<WebSocketWhat> = {
      what: [{ when: new Date(), what }],
      how,
    };
    this.subject.next(exp);
  }

  connected(remote: string): void {
    this.feel({ type: "connect", remote });
  }

  disconnected(remote: string): void {
    this.feel({ type: "disconnect", remote });
  }

  received(remote: string, name: string, message: string): void {
    this.feel({ type: "message", remote, name, message });
  }
}
