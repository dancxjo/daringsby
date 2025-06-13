import { Sensor } from "./Sensor.ts";
import { InstructionFollower } from "./InstructionFollower.ts";
import { Experience } from "./Experience.ts";
import { ChatMessage, Chatter } from "./Chatter.ts";
import { WebSocketSensor } from "../sensors/websocket.ts";

/**
 * Psyche holds a collection of sensors representing external stimuli.
 */

export class Psyche {
    private beats = 0;
    private live = true;
    private buffer: Experience<any>[] = [];
    private speaking = false;
    private pendingSpeech = "";
    public instant = "Pete has just been born.";
    public conversation: ChatMessage[] = [];

    constructor(
        public externalSensors: Sensor<any>[] = [],
        private instructionFollower: InstructionFollower,
        private chatter: Chatter,
        private opts: {
            onStream?: (chunk: string) => Promise<void>;
            /** Called with the prompt used during integrate_sensory_input */
            onPrompt?: (prompt: string) => Promise<void>;
            onSay?: (text: string) => Promise<void>;
            wsSensor?: WebSocketSensor;
        } = {},
    ) {
        for (const sensor of this.externalSensors) {
            sensor.subscribe((e) => {
                Deno.stdout.writeSync(
                    new TextEncoder().encode(`x`),
                );
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
        await this.integrate_sensory_input();
        if (!this.speaking) {
            await this.take_turn();
        }
        if (this.opts.wsSensor) {
            this.opts.wsSensor.self(this.pendingSpeech);
        }
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
            "## Pete's Senses\n* " +
            this.externalSensors.map((s) => s.describeSensor()).join("\n* ") +
            "\nThese are currently your *only* senses. If you sense something else, it is internal to your own mind.\n## Pete's Current Situation (as he understands it)\n" +
            `${this.instant}\n` +
            "## What just happened in the last instant\n\n" +
            `${happenings}\n` +
            "Condense the happenings here into one sentence, emphasizing the most salient information and omitting irrelevant information. Speak only as Pete (who is not an LLM).";
        try {
            await this.opts.onPrompt?.(prompt);
            this.instant = await this.instructionFollower.instruct(
                prompt,
                this.opts.onStream,
            );
        } catch (err) {
            console.error("instruction follower failed", err);
        }
        this.buffer = [];
        Deno.stdout.writeSync(
            new TextEncoder().encode(`.`),
        );
    }

    /**
     * Engage in conversation based on the current instant and stored messages.
     */
    async take_turn(): Promise<void> {
        if (this.speaking) {
            Deno.stdout.writeSync(
                new TextEncoder().encode(`O`),
            );
            return;
        }

        const messages: ChatMessage[] = [
            {
                role: "system",
                content:
                    `You are the linguistic processing unit for an artificial entity named Pete. Here's the situation as Pete understands it: ${this.instant}\n\nSpeak in Pete's voice on his behalf to the user. As your conversation progresses, you will receive more information about Pete's situation. Use this information to inform your responses, and respond only with spoken text (no non-linguistic notes). Everything you return will be spoken out loud by Pete. Be concise, clear, and conversational. Do not use any markdown or code blocks. You will have a chance to continue further so do not try to say everything at once.`,
            },
            ...this.conversation,
        ];
        try {
            this.pendingSpeech = "";
            const reply = await this.chatter.chat(
                messages,
                async (chunk) => {
                    this.pendingSpeech += chunk;
                    await this.opts.onStream?.(chunk);
                },
            );
            this.pendingSpeech = reply;
            this.opts.wsSensor?.self(reply);
            await this.opts.onSay?.(reply);
        } catch (err) {
            console.error("chatter failed", err);
            this.pendingSpeech = "";
        } finally {
            this.speaking = true;
        }
    }

    confirm_echo(message: string): void {
        if (this.pendingSpeech && message === this.pendingSpeech) {
            this.conversation.push({ role: "assistant", content: message });
            this.pendingSpeech = "";
            this.speaking = false;
        }
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
