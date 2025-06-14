import { Psyche } from "../../lib/Psyche.ts";
import { InstructionFollower } from "../../lib/InstructionFollower.ts";
import { ChatMessage, Chatter } from "../../lib/Chatter.ts";
import { WebSocketSensor } from "../../sensors/websocket.ts";
import { Sensor } from "../../lib/Sensor.ts";
import { Experience } from "../../lib/Experience.ts";

class StubFollower extends InstructionFollower {
  async instruct(): Promise<string> {
    return "instant";
  }
}

class CountingChatter extends Chatter {
  calls = 0;
  async chat(_m: ChatMessage[]): Promise<string> {
    this.calls++;
    return "reply";
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

Deno.test("take_turn waits until a websocket client connects", async () => {
  const wsSensor = new WebSocketSensor();
  const dummy = new DummySensor();
  const follower = new StubFollower();
  const chatter = new CountingChatter();
  const psyche = new Psyche([dummy, wsSensor], follower, chatter, { wsSensor });

  dummy.feel("hi");
  await psyche.beat();
  if (chatter.calls !== 0) {
    throw new Error("took a turn without clients");
  }

  wsSensor.connected("ip");
  await psyche.beat();
  if (chatter.calls !== 1) {
    throw new Error("did not take a turn after connection");
  }

  wsSensor.disconnected("ip");
  await psyche.beat();
  if (chatter.calls !== 1) {
    throw new Error("took turn after all clients disconnected");
  }
});
