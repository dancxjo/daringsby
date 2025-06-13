import { Psyche } from "./lib/Psyche.ts";
import { HeartbeatSensor } from "./sensors/heartbeat.ts";

/**
 * Pete is our main character.
 */
export const Pete = new Psyche([
  new HeartbeatSensor(),
]);

// Start Pete's life cycle.
Pete.run();

