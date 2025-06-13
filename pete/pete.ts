import { Psyche } from "./mod.ts";
import { HeartbeatSensor } from "./heartbeat_sensor.ts";

/**
 * Pete is a psyche with a single heartbeat sensor.
 */
export const Pete = new Psyche([
  new HeartbeatSensor(),
]);
