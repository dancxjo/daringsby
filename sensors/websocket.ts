export type WebSocketWhat =
  | { type: "connect"; remote: string }
  | { type: "disconnect"; remote: string }
  | { type: "message"; remote: string; name: string; message: string }
  | { type: "self"; message: string }
  | { type: "echo"; remote: string; message: string };

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
  describeSensor(): string {
    return "WebSocket: Allows your developers to communicate with you directly. This sensor tells you when clients connect, disconnect and speak to you (ASR support forthcoming). It also allows you to speak back to them.";
  }
  feel(what: WebSocketWhat): void {
    let how: string;
    switch (what.type) {
      case "connect":
        how = `I feel a Client ${what.remote} connected.`;
        break;
      case "disconnect":
        how = `I feel a Client ${what.remote} disconnected.`;
        break;
      case "message":
        how = `I hear ${what.name} say: ${what.message}`;
        break;
      case "self":
        how = `I feel myself on the verge of saying: ${what.message}`;
        break;
      case "echo":
        how = `I heard myself speak the following: ${what.message}`;
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

  self(message: string): void {
    this.feel({ type: "self", message });
  }

  echoed(remote: string, message: string): void {
    this.feel({ type: "echo", remote, message });
  }
}
