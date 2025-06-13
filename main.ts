import { Psyche } from "./lib/Psyche.ts";
import { HeartbeatSensor } from "./sensors/heartbeat.ts";
import {
  OllamaChatter,
  OllamaInstructionFollower,
} from "./providers/ollama.ts";
import { Ollama } from "npm:ollama";
/**
 * Pete is our main character.
 */
export const Pete = new Psyche(
  [new HeartbeatSensor()],
  new OllamaInstructionFollower(new Ollama(), "gemma3"),
  new OllamaChatter(new Ollama(), "gemma3"),
  async (chunk: string) => {
    await Deno.stdout.write(new TextEncoder().encode(chunk));
  },
);

// Start Pete's life cycle.
Pete.run();
