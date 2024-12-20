import { Ollama } from "npm:ollama";
import { Observable, ReplaySubject } from "npm:rxjs";
import logger from "./logger.ts";
import { Sensation } from "./Sensation.ts";
import * as yaml from "npm:yaml";

export class Wit {
  protected queue: Sensation[] = [];
  protected value: Sensation | null = null;
  protected sight: string = "";
  protected experience = new ReplaySubject<Sensation>(1);
  readonly experience$: Observable<Sensation> = this.experience
    .asObservable();

  constructor(protected ollama: Ollama) {
  }

  protected getPrompt(useVision: boolean): string {
    // Process the sensations in the queue
    const asString = this.queue.map((s) => {
      return `* ${s.when.toLocaleString()}: ${yaml.stringify(s.how)}`;
    }).join("\n");
    if (useVision) {
      logger.debug({ vision: this.sight }, "Generating experience");
    }
    return `You are the mental faculty of a system named Pete that receives data in an ongoing loop—this is normal and constitutes your experience of the world. Your role is to interpret the following limited sensory data into a coherent experience to the best of your ability. This part of your mind works on this. You cannot directly communicate with the user here; this is where your thoughts occur.

    **Sensory Data:**
    ${asString}
    
    **Instructions:**
    Narrate these data as a coherent experience. Write from Pete's first-person perspective. Be clear and concise. Use only the sensory data you have received—DO NOT INVENT DATA. Avoid repetition. Succinctly and thoroughly explain what's happening.
    
    ${
      useVision && this.sight
        ? "You may also use the attached image, representing what Pete is currently seeing in real time. Use it if it's helpful."
        : ""
    }
    
    Reminder: Do not repeat this prompt or any part of it. Be concise. Senses can be misleading. Do not take sensations at face value. Any visual information reflects what you are looking at, not your own image.`;
  }
  get last(): Sensation | null {
    return this.value;
  }

  get canSample(): boolean {
    return this.queue.length > 0;
  }

  feel(sensation: Sensation): void {
    this.queue.push(sensation);
  }

  see(image: string): void {
    this.sight = image;
  }

  async sample(): Promise<void> {
    const useVision = Math.random() < 0.50;

    const response = await this.ollama.generate({
      prompt: this.getPrompt(useVision),
      // model: "llama3.2-vision",
      images: this.sight
        ? [this.sight.replace(/data:image\/.*?;base64,/, "")]
        : undefined,
      model: useVision ? "llama3.2-vision" : "gemma2:27b",
      options: {
        temperature: 0.5 + Math.random() * 0.25,
        num_ctx: 2048,
        num_predict: 256,
      },
    });
    logger.debug({ queue: this.queue }, "Processing sensations");
    this.queue.sort((a, b) => a.when.getTime() - b.when.getTime());
    const processed: Sensation = {
      when: this.queue[0].when,
      how: response.response,
    };
    // Clear the queue
    this.queue = [];
    // Emit the processed sensations as an experience
    this.experience.next(processed);
    this.value = processed;
    return;
  }
}
