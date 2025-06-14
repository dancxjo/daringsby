import { Psyche } from "../../lib/Psyche.ts";
import { InstructionFollower } from "../../lib/InstructionFollower.ts";
import { Chatter } from "../../lib/Chatter.ts";
import { Sensor } from "../../lib/Sensor.ts";
import { Experience } from "../../lib/Experience.ts";

class StubFollower extends InstructionFollower {
  calls = 0;
  async instruct(): Promise<string> {
    this.calls++;
    return "moment";
  }
}

class SilentChatter extends Chatter {
  async chat(): Promise<string> { return ""; }
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

Deno.test("combobulator summarizes instants", async () => {
  const sensor = new StubSensor();
  const follower = new StubFollower();
  const chatter = new SilentChatter();
  const psyche = new Psyche([sensor], follower, chatter);
  sensor.feel("a");
  await psyche.beat();
  if (psyche.moment !== "moment") {
    throw new Error("moment not set");
  }
  if (follower.calls !== 2) {
    throw new Error("wit call count");
  }
});
