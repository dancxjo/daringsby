import { Psyche } from "../../lib/Psyche.ts";
import { InstructionFollower } from "../../lib/InstructionFollower.ts";
import { ChatMessage, Chatter } from "../../lib/Chatter.ts";
import { Sensor } from "../../lib/Sensor.ts";
import { Experience } from "../../lib/Experience.ts";

class StubFollower extends InstructionFollower {
  async instruct(): Promise<string> {
    return "instant";
  }
}

class StubChatter extends Chatter {
  messages: ChatMessage[] = [];
  async chat(messages: ChatMessage[]): Promise<string> {
    this.messages = messages;
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

Deno.test("take_turn prepends system message with instant", async () => {
  const sensor = new EmptySensor();
  const follower = new StubFollower();
  const chatter = new StubChatter();
  const psyche = new Psyche([sensor], follower, chatter);

  psyche.conversation.push({ role: "user", content: "hi" });
  await psyche.take_turn();

  const first = chatter.messages[0];
  if (!first.content.includes("instant") || first.role !== "system") {
    throw new Error("system message missing");
  }
});
