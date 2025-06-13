import { MockInstructionFollower } from "../../lib/InstructionFollower.ts";

function assertEquals(actual: unknown, expected: unknown) {
  if (actual !== expected) {
    throw new Error(`Expected ${expected}, got ${actual}`);
  }
}

Deno.test("replaces every word with Malkovitch", async () => {
  const follower = new MockInstructionFollower();
  const result = await follower.instruct("Hello there, world!");
  assertEquals(result, "Malkovitch Malkovitch, Malkovitch!");
});

Deno.test("calls onChunk with the replaced text", async () => {
  const follower = new MockInstructionFollower();
  let chunk = "";
  const result = await follower.instruct("Hi", (c) => chunk = c);
  assertEquals(result, "Malkovitch");
  assertEquals(chunk, "Malkovitch");
});
