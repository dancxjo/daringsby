export abstract class InstructionFollower {
  /**
   * Process a prompt and produce a response.
   * @param prompt text to process
   * @param onChunk optional handler for streaming output chunks
   * @returns resolved response string
   */
  abstract instruct(
    prompt: string,
    onChunk?: (chunk: string) => Promise<void>,
  ): Promise<string>;
}

/**
 * Mock implementation replacing every word with "Malkovitch".
 *
 * ```ts
 * const follower = new MockInstructionFollower();
 * const reply = await follower.instruct("Hello world");
 * // reply === "Malkovitch Malkovitch"
 * ```
 */
export class MockInstructionFollower extends InstructionFollower {
  async instruct(
    prompt: string,
    onChunk?: (chunk: string) => Promise<void>,
  ): Promise<string> {
    const result = prompt.replace(/\b\w+\b/g, "Malkovitch");
    if (onChunk) await onChunk(result);
    return result;
  }
}
