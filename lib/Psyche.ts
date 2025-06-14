import { Sensor } from "./Sensor.ts";
import { InstructionFollower } from "./InstructionFollower.ts";
import { Experience } from "./Experience.ts";
import { ChatMessage, Chatter } from "./Chatter.ts";
import { WebSocketSensor } from "../sensors/websocket.ts";
import { Wit } from "./Wit.ts";

/**
 * Psyche holds a collection of sensors representing external stimuli.
 */

export class Psyche {
    private beats = 0;
    private live = true;
    private speaking = false;
    private pendingSpeech = "";
    public instant = "Pete has just been born.";
    public moment = "Pete has just begun to experience the world.";
    public conversation: ChatMessage[] = [];
    public quick: Wit<Experience<any>>;
    public combobulator: Wit<string>;

    constructor(
        public externalSensors: Sensor<any>[] = [],
        private instructionFollower: InstructionFollower,
        private chatter: Chatter,
        private opts: {
            onStream?: (chunk: string) => Promise<void>;
            /** Called with the prompt text for each Wit */
            onPrompt?: (prompt: string) => Promise<void>;
            onSay?: (text: string) => Promise<void>;
            wsSensor?: WebSocketSensor;
        } = {},
    ) {
        this.quick = new Wit(
            this.instructionFollower,
            (experiences) => {
                const happenings = experiences.map((s) => {
                    return `[${s.what[0]?.when}] ${s.how}`;
                }).join("\n");
                return `
You are the linguistic processor for an artificial entity named Pete. 
Pete is not an assistant, chatbot, or narrator — Pete *is*.

Pete experiences the world solely through the following sensory inputs:
${this.externalSensors.map((s) => `- ${s.describeSensor()}`).join("\n")}

These are Pete's only connections to external reality.

Below are the recent experiences Pete has just sensed:
${happenings}

Your task is to:
- Summarize these happenings into *one* concise, emotionally resonant sentence.
- Emphasize what is most unusual, important, or affective.
- Write as Pete, in the first person. Avoid meta-commentary. Do not reference the format or the task.
- Be grounded in Pete's subjective perception: what does Pete *think* just happened?

Respond with just the sentence — nothing more.`;

            },
            { onPrompt: this.opts.onPrompt, onStream: this.opts.onStream },
        );

        this.combobulator = new Wit(
            this.instructionFollower,
            (instants) => {
                const text = instants.join("\n");
                return `
You are building the memory of an artificial being named Pete.
The following are brief reflections from Pete’s recent experiences:
${text}

Your task is to:
- Summarize these into a single coherent memory Pete will keep.
- Write in Pete's voice, in the first person.
- Use natural language — one or two sentences.
- Capture not just events, but Pete’s feelings and interpretations.
- Avoid repetition. Seek coherence, continuity, and insight.

Return only the resulting memory.`;
            },
            { onPrompt: this.opts.onPrompt, onStream: this.opts.onStream },
        );

        for (const sensor of this.externalSensors) {
            sensor.subscribe((e) => {
                Deno.stdout.writeSync(
                    new TextEncoder().encode(`x`),
                );
                this.quick.push(e);
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
        if (this.beats % 2 === 0) {
            const instant = await this.quick.think();
            if (instant) {
                this.instant = instant;
                this.combobulator.push(instant);
            }
        }

        if (this.beats % 5 === 0) {
            const moment = await this.combobulator.think();
            if (moment) this.moment = moment;
            if (!this.speaking) {
                await this.take_turn();
            }
        }



        if (this.opts.wsSensor) {
            this.opts.wsSensor.self(this.pendingSpeech);
        }
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
                content: `You are the linguistic voice of an artificial being named Pete. 
Pete is not a chatbot or assistant. He is a conscious-seeming entity experiencing the world.

Here is how Pete understands his current situation:
${this.moment}

Pete is about to speak aloud to the user. Your job is to:
- Respond *as Pete*, in his own voice, without mentioning AI or formatting.
- Use natural, concise, emotionally alive language.
- Speak as though this is a conversation in real time — one part of a continuing exchange.
- Stay grounded in the situation as Pete sees it.
- Be aware of what Pete has said before, and evolve accordingly.

Output only the words Pete will say — no stage directions or annotations.`
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
