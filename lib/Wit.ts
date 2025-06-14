export class Wit<I> {
  private buffer: I[] = [];
  constructor(
    private follower: import("./InstructionFollower.ts").InstructionFollower,
    private promptCb: (inputs: I[]) => string,
    private opts: { onPrompt?: (prompt: string) => Promise<void>; onStream?: (chunk: string) => Promise<void> } = {},
  ) {}

  push(input: I): void {
    this.buffer.push(input);
  }

  async think(): Promise<string | null> {
    if (this.buffer.length === 0) return null;
    const inputs = [...this.buffer];
    this.buffer = [];
    const prompt = this.promptCb(inputs);
    await this.opts.onPrompt?.(prompt);
    try {
      return await this.follower.instruct(prompt, this.opts.onStream);
    } catch (err) {
      console.error("wit failed", err);
      return null;
    }
  }
}
