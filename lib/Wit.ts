export class Wit<I> {
  private buffer: I[] = [];
  constructor(
    private readonly name: string,
    private follower: import("./InstructionFollower.ts").InstructionFollower,
    private promptCb: (inputs: I[]) => string,
    private opts: {
      onPrompt?: (name: string, prompt: string) => Promise<void>;
      onStream?: (name: string, chunk: string) => Promise<void>;
    } = {},
  ) {}

  push(input: I): void {
    this.buffer.push(input);
  }

  async think(): Promise<string | null> {
    if (this.buffer.length === 0) return null;
    const inputs = [...this.buffer];
    this.buffer = [];
    const prompt = this.promptCb(inputs);
    await this.opts.onPrompt?.(this.name, prompt);
    try {
      return await this.follower.instruct(
        prompt,
        this.opts.onStream
          ? (c) => this.opts.onStream!(this.name, c)
          : undefined,
      );
    } catch (err) {
      console.error("wit failed", err);
      return null;
    }
  }
}
