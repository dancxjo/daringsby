import { assertEquals } from "https://raw.githubusercontent.com/denoland/deno_std/0.224.0/assert/mod.ts";
import { Sensor } from "../lib.ts";

Deno.test("sensor emits filtered sensations", async () => {
  const sensor = new Sensor<number>((s) => s.what > 0);
  const results: number[] = [];

  sensor.subscribe((s) => results.push(s.what));

  sensor.feel(-1);
  sensor.feel(1);
  sensor.feel(2);

  // Allow microtasks to flush
  await Promise.resolve();

  assertEquals(results, [1, 2]);
});
