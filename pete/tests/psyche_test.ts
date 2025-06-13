import { assertEquals, assert } from "https://raw.githubusercontent.com/denoland/deno_std/0.224.0/assert/mod.ts";
import { Psyche } from "../mod.ts";
import { HeartbeatSensor } from "../heartbeat_sensor.ts";

Deno.test("psyche stores external sensors", () => {
  const sensor = new HeartbeatSensor(1, 0);
  const psyche = new Psyche([sensor]);
  assertEquals(psyche.externalSensors.length, 1);
  sensor.stop();
});

Deno.test("heartbeat sensor emits a message", async () => {
  const sensor = new HeartbeatSensor(5, 0);
  const messages: string[] = [];
  sensor.subscribe((s) => messages.push(s.what));
  await new Promise((res) => setTimeout(res, 10));
  sensor.stop();
  assert(messages.length > 0);
});
