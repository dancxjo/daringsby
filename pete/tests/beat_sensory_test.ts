import { Psyche } from "../../lib/Psyche.ts";
import { InstructionFollower } from "../../lib/InstructionFollower.ts";
import { Chatter, ChatMessage } from "../../lib/Chatter.ts";
import { Sensor } from "../../lib/Sensor.ts";
import { Experience } from "../../lib/Experience.ts";

class CountingFollower extends InstructionFollower {
  calls = 0;
  async instruct(): Promise<string> {
    this.calls++;
    return "instant";
  }
}

class SilentChatter extends Chatter {
  async chat(_m: ChatMessage[]): Promise<string> {
    return "reply";
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

Deno.test("integrate_sensory_input runs while speaking", async () => {
  const sensor = new StubSensor();
  const follower = new CountingFollower();
  const chatter = new SilentChatter();
  const psyche = new Psyche([sensor], follower, chatter);

  sensor.feel("one");
  await psyche.beat(); // start speaking

  sensor.feel("two");
  await psyche.beat(); // should integrate again even though speaking

  if (follower.calls !== 2) {
    throw new Error("expected integrate_sensory_input on each beat");
  }
});
