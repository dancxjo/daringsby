import { Psyche } from "../../lib/Psyche.ts";
import { Sensor } from "../../lib/Sensor.ts";
import { InstructionFollower } from "../../lib/InstructionFollower.ts";

function assert(condition: unknown, msg = "Assertion failed") {
  if (!condition) throw new Error(msg);
}

class StubFollower extends InstructionFollower {
  prompt = "";
  async instruct(prompt: string): Promise<string> {
    this.prompt = prompt;
    return "stub";
  }
}

Deno.test("integrate_sensory_input summarizes buffered sensations", async () => {
  const sensor = new Sensor<string>();
  const follower = new StubFollower();
  const psyche = new Psyche([sensor], follower);

  sensor.feel("hello world");
  await psyche.integrate_sensory_input();

  assert(follower.prompt.includes("hello world"), "prompt missing sensation");
});
