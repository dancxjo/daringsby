import { assertEquals } from "https://deno.land/std@0.224.0/assert/mod.ts";
import { Sensor } from "../mod.ts";

deno.test("sensor emits filtered sensations", async () => {
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
