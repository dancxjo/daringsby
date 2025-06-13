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
  override describeSensor(): string {
    return "StubSensor: A sensor that allows you to feel strings.";
  }
  feel(what: string): void {
    const exp: Experience<string> = {
      what: [{ when: new Date(), what }],
      how: what,
    };
    this.subject.next(exp);
  }
}

