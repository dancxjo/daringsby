import { Sensor } from "./Sensor.ts";
import { InstructionFollower } from "./InstructionFollower.ts";
import { Sensation } from "./Sensation.ts";

/**
 * Psyche holds a collection of sensors representing external stimuli.
 */

export class Psyche<X = unknown> {
  private beats = 0;
  private live = true;
  private buffer: Sensation<X>[] = [];
  public instant = "Pete has just been born.";

  constructor(
    public externalSensors: Sensor<X>[] = [],
    private instructionFollower: InstructionFollower,
  ) {
    for (const sensor of this.externalSensors) {
      sensor.subscribe((s) => this.buffer.push(s));
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
  beat(): void {
    this.beats++;
  }

  /**
   * Integrate buffered sensory input using the instruction follower.
   * Clears the buffer and updates `instant` with the follower's response.
   */
  async integrate_sensory_input(): Promise<void> {
    if (this.buffer.length === 0) return;
    const happenings = this.buffer.map((s) => {
      const when = s.when.toLocaleString();
      return `[${when}] ${s.what}`;
    }).join("\n");
    const prompt =
      "You are the linguistic processor for an artificial entity named Pete. Speak in Pete's voice on his behalf.\n" +
      "## Pete's Current Situation (as he understands it)\n" +
      `${this.instant}\n` +
      "## What just happened in the last instant\n\n" +
      `${happenings}\n` +
      "Condense the happenings here into one sentence, emphasizing the most salient information and omitting irrelevant information. Speak only as Pete (who is not an LLM).";
    this.instant = await this.instructionFollower.instruct(prompt);
    this.buffer = [];
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
      this.beat();
      await this.integrate_sensory_input();
      await new Promise((res) => setTimeout(res, 0));
    }
  }
}
