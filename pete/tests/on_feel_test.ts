import { Psyche } from "../../lib/Psyche.ts";
import { InstructionFollower } from "../../lib/InstructionFollower.ts";
import { Chatter } from "../../lib/Chatter.ts";
import { Sensor } from "../../lib/Sensor.ts";
import { Experience } from "../../lib/Experience.ts";

class StubFollower extends InstructionFollower {
  async instruct(): Promise<string> {
    return "🙂";
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

Deno.test("onFeel receives updated emoji", async () => {
  const follower = new StubFollower();
  const chatter = new StubChatter();
  const sensor = new DummySensor();
  const emotions: string[] = [];
  const psyche = new Psyche([sensor], follower, chatter, {
    onFeel: async (e) => emotions.push(e),
  });
  sensor.feel("tick");
  await psyche.beat();
  sensor.feel("tock");
  await psyche.beat();
  sensor.feel("boom");
  await psyche.beat();
  if (!emotions.includes("🙂")) {
    throw new Error("feeling not forwarded");
  }
});
