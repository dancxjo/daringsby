import { Ollama } from "npm:ollama";
import { InstructionFollower } from "../lib/InstructionFollower.ts";

export class OllamaInstructionFollower extends InstructionFollower {
  constructor(protected client: Ollama, protected model: string) {
    super();
  }

  async instruct(
    prompt: string,
    onChunk?: (chunk: string) => Promise<void>,
  ): Promise<string> {
    const stream = await this.client.generate({
      stream: true,
      model: this.model,
      prompt: prompt,
    });
    let response = "";
    for await (const chunk of stream) {
      const content = chunk.response || "";
      response += content;
      if (onChunk) {
        await onChunk(content);
      }
    }
    return response;
  }
}

import { ChatMessage, Chatter } from "../lib/Chatter.ts";

export class OllamaChatter extends Chatter {
  constructor(protected client: Ollama, protected model: string) {
    super();
  }

  async chat(
    messages: ChatMessage[],
    onChunk?: (chunk: string) => Promise<void>,
  ): Promise<string> {
    console.log(`Messages: ${JSON.stringify(messages)}`);
    const stream = await this.client.chat({
      stream: true,
      model: this.model,
      messages,
    });
    let response = "";
    for await (const chunk of stream) {
      const content = chunk.message?.content || "";
      response += content;
      if (onChunk) {
        await onChunk(content);
      }
    }
    return response;
  }
}
