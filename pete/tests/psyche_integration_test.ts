import { Psyche } from "../../lib/Psyche.ts";
import { Sensor } from "../../lib/Sensor.ts";
import { InstructionFollower } from "../../lib/InstructionFollower.ts";
import { ChatMessage, Chatter } from "../../lib/Chatter.ts";
import { Experience } from "../../lib/Experience.ts";

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

class StubChatter extends Chatter {
  messages: ChatMessage[] = [];
  async chat(messages: ChatMessage[]): Promise<string> {
    this.messages = messages;
    return "ok";
  }
}

class StubSensor extends Sensor<string> {
  feel(what: string): void {
    const exp: Experience<string> = {
      what: [{ when: new Date(), what }],
      how: what,
    };
    this.subject.next(exp);
  }
}

Deno.test("integrate_sensory_input summarizes buffered sensations", async () => {
  const sensor = new StubSensor();
  const follower = new StubFollower();
  const chatter = new StubChatter();
  const psyche = new Psyche([sensor], follower, chatter);

  sensor.feel("hello world");
  await psyche.integrate_sensory_input();

  assert(follower.prompt.includes("hello world"), "prompt missing sensation");
  const first = chatter.messages[0];
  assert(
    first.role === "system" && first.content.includes("stub"),
    "chatter not called",
  );
});
