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
  calls = 0;
  async chat(messages: ChatMessage[]): Promise<string> {
    this.calls++;
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

Deno.test("reply added after echo", async () => {
  const sensor = new EmptySensor();
  const follower = new StubFollower();
  const chatter = new StubChatter();
  const psyche = new Psyche([sensor], follower, chatter);

  await psyche.take_turn();
  if (psyche.conversation.some((m) => m.role === "assistant")) {
    throw new Error("assistant added early");
  }
  await psyche.take_turn();
  if (chatter.calls !== 1) {
    throw new Error("should not chat while speaking");
  }
  psyche.confirm_echo("reply");
  const last = psyche.conversation.pop();
  if (last?.content !== "reply" || last.role !== "assistant") {
    throw new Error("reply missing after echo");
  }
});
