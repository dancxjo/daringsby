import { Psyche } from "../../lib/Psyche.ts";
import { InstructionFollower } from "../../lib/InstructionFollower.ts";
import { Chatter, ChatMessage } from "../../lib/Chatter.ts";
import { Sensor } from "../../lib/Sensor.ts";
import { Experience } from "../../lib/Experience.ts";

class StubFollower extends InstructionFollower {
  async instruct(): Promise<string> {
    return "instant";
  }
}

class StubChatter extends Chatter {
  async chat(_m: ChatMessage[]): Promise<string> {
    return "reply";
  }
}

class EmptySensor extends Sensor<null> {
  feel(_: null): void {
    const exp: Experience<null> = {
      what: [{ when: new Date(), what: null }],
      how: "",
    };
    this.subject.next(exp);
  }
}

Deno.test("confirm_echo trims message", async () => {
  const sensor = new EmptySensor();
  const follower = new StubFollower();
  const chatter = new StubChatter();
  const psyche = new Psyche([sensor], follower, chatter);

  await psyche.take_turn();
  psyche.confirm_echo("reply\n");
  const last = psyche.conversation.pop();
  if (last?.content !== "reply" || last.role !== "assistant") {
    throw new Error("echo not recognized with newline");
  }
});
