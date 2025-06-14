import { OllamaInstructionFollower, OllamaChatter } from "../../providers/ollama.ts";
import { ChatMessage } from "../../lib/Chatter.ts";

class StubOllama {
  temps: number[] = [];
  async *generate(options: { options?: { temperature?: number } }) {
    this.temps.push(options.options?.temperature ?? -1);
    yield { response: "" };
  }
  async *chat(options: { options?: { temperature?: number } }) {
    this.temps.push(options.options?.temperature ?? -1);
    yield { message: { content: "" } };
  }
}

deno.test("instruction follower sets temperature between 0.7 and 1", async () => {
  const client = new StubOllama();
  const follower = new OllamaInstructionFollower(client as any, "model");
  await follower.instruct("hi");
  const t = client.temps[0];
  if (t < 0.7 || t > 1) {
    throw new Error(`temperature ${t}`);
  }
});

deno.test("chatter sets temperature between 0.7 and 1", async () => {
  const client = new StubOllama();
  const chatter = new OllamaChatter(client as any, "model");
  await chatter.chat([ { role: "user", content: "hi" } ] as ChatMessage[]);
  const t = client.temps[0];
  if (t < 0.7 || t > 1) {
    throw new Error(`temperature ${t}`);
  }
});
