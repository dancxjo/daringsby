import { Psyche } from "../../lib/Psyche.ts";
import { InstructionFollower } from "../../lib/InstructionFollower.ts";
import { Chatter } from "../../lib/Chatter.ts";
import { Sensor } from "../../lib/Sensor.ts";
import { Experience } from "../../lib/Experience.ts";

class StubFollower extends InstructionFollower {
  async instruct(prompt: string): Promise<string> {
    return prompt;
  }
}

class StubChatter extends Chatter {
  async chat(): Promise<string> {
    return "";
  }
}

class DummySensor extends Sensor<string> {
  override describeSensor(): string {
    return "Dummy";
  }
  feel(what: string): void {
    const exp: Experience<string> = {
      what: [{ when: new Date(), what }],
      how: what,
    };
    this.subject.next(exp);
  }
}

Deno.test("onPrompt receives prompt text", async () => {
  const follower = new StubFollower();
  const chatter = new StubChatter();
  const sensor = new DummySensor();
  const prompts: { name: string; prompt: string }[] = [];
  const psyche = new Psyche([sensor], follower, chatter, {
    onPrompt: async (name, p) => {
      prompts.push({ name, prompt: p });
    },
  });
  sensor.feel("hi");
  await psyche.beat();
  if (!prompts.some((p) => p.prompt.includes("hi") && p.name === "quick")) {
    throw new Error("prompt not forwarded");
  }
});
