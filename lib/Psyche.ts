import { Sensor } from "./Sensor.ts";
import { InstructionFollower } from "./InstructionFollower.ts";
import { Experience } from "./Experience.ts";
import { ChatMessage, Chatter } from "./Chatter.ts";

/**
 * Psyche holds a collection of sensors representing external stimuli.
 */

export class Psyche {
  private beats = 0;
  private live = true;
  private buffer: Experience<unknown>[] = [];
  public instant = "Pete has just been born.";
  public conversation: ChatMessage[] = [];

  constructor(
    public externalSensors: Sensor<unknown>[] = [],
    private instructionFollower: InstructionFollower,
    private chatter: Chatter,
    private onStream?: (chunk: string) => Promise<void>,
  ) {
    for (const sensor of this.externalSensors) {
      sensor.subscribe((e) => {
        this.buffer.push(e);
      });
    }
  }

  /** How many beats have occurred. */
  get beatCount(): number {
    return this.beats;
  }

  /** Whether the psyche should keep running. */
  isLive(): boolean {
    return this.live;
  }

  /** Stop the psyche's run loop. */
  stop(): void {
    this.live = false;
  }

  /** Increment the internal beat counter. */
  async beat(): Promise<void> {
    this.beats++;
    // console.log(`Beat ${this.beats} at ${new Date().toLocaleTimeString()}`);
    await this.integrate_sensory_input();
  }

  /**
   * Integrate buffered sensory input using the instruction follower.
   * Clears the buffer and updates `instant` with the follower's response.
   */
  async integrate_sensory_input(): Promise<void> {
    if (this.buffer.length === 0) return;
    const happenings = this.buffer.map((s) => {
      return `[${s.what[0]?.when}] ${s.how}`;
    }).join("\n");
    const prompt =
      "You are the linguistic processor for an artificial entity named Pete. Speak in Pete's voice on his behalf.\n" +
      "## Pete's Current Situation (as he understands it)\n" +
      `${this.instant}\n` +
      "## What just happened in the last instant\n\n" +
      `${happenings}\n` +
      "Condense the happenings here into one sentence, emphasizing the most salient information and omitting irrelevant information. Speak only as Pete (who is not an LLM).";
    this.instant = await this.instructionFollower.instruct(
      prompt,
      this.onStream,
    );
    this.buffer = [];
    console.log(
      `Beat ${this.beats} at ${
        new Date().toLocaleTimeString()
      }: ${this.instant}`,
    );
    await this.take_turn();
  }

  /**
   * Engage in conversation based on the current instant and stored messages.
   */
  async take_turn(): Promise<void> {
    const messages: ChatMessage[] = [
      {
        role: "system",
        content: `You are Pete. Here's your situation: ${this.instant}`,
      },
      ...this.conversation,
    ];
    const reply = await this.chatter.chat(messages, this.onStream);
    this.conversation.push({ role: "assistant", content: reply });
  }

  /**
   * Continuously run while the psyche is live.
   *
   * ```ts
   * const psyche = new Psyche();
   * psyche.run();
   * psyche.stop();
   * ```
   */
  async run(): Promise<void> {
    while (this.isLive()) {
      await this.beat();
      await new Promise((res) => setTimeout(res, 0));
    }
  }
}
