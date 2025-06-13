import { Psyche } from "./lib/Psyche.ts";
import { HeartbeatSensor } from "./sensors/heartbeat.ts";
import { MockInstructionFollower } from "./lib/InstructionFollower.ts";

/**
 * Pete is our main character.
 */
export const Pete = new Psyche(
  [new HeartbeatSensor()],
  new MockInstructionFollower(),
  async (chunk: string) => {
    await Deno.stdout.write(new TextEncoder().encode(chunk));
  },
);

// Start Pete's life cycle.
Pete.run();
