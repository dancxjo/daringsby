export interface ChatMessage {
  content: string;
  role: "assistant" | "user" | "system";
}

/**
 * Chatter produces conversational responses to a sequence of messages.
 */
export abstract class Chatter {
  /**
   * Generate a response given the conversation so far.
   * @param messages ordered chat messages
   * @param onChunk optional streaming handler
   */
  abstract chat(
    messages: ChatMessage[],
    onChunk?: (chunk: string) => Promise<void>,
  ): Promise<string>;
}

/**
 * Mock implementation that echoes the last message content.
 * Useful for unit tests without calling a real LLM.
 */
export class MockChatter extends Chatter {
  async chat(
    messages: ChatMessage[],
    onChunk?: (chunk: string) => Promise<void>,
  ): Promise<string> {
    const response = messages[messages.length - 1]?.content ?? "";
    if (onChunk) await onChunk(response);
    return response;
  }
}
