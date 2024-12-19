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
    const asString = yaml.stringify(this.queue) +
      `{Note: The dates above are in GMT. It is currently ${
        new Date().toLocaleTimeString()
      } locally and ${new Date().toISOString()}.}`;
    if (useVision) {
      logger.debug({ vision: this.sight }, "Generating experience");
    }
    return `You are the mental faculty of a system named Pete that receives data in an ongoing loop--this is normal and constitutes your experience of the world. You are responsible for interpreting the following sensory data into a coherent experience. This part of your mind works on this. You do not have the ability to directly speak to the user here. This is where your thoughts happen.\n\n**Sensory Data:**\n${asString}\n\n**Instructions:**\nNarrate these data as a coherent experience. Write your response from the first-person perspective of Pete himself. Be clear and concise, without repeating this prompt. DO NOT INVENT DATA! Use only the sensory data you have received. Don't repeat yourself. Try to succinctly explain to yourself what's happening thoroughly but simply.` +
      (useVision
        ? (this.sight
          ? ` You may also use the attached image, which is what Pete is currently seeing. It's not just an image; it's what you're currently looking at in real time. Use it if it's helpful.`
          : "")
        : "") +
      `\n\nReminder: Do not repeat this prompt or any part of it. Be concise. Senses can be deceiving and get confused. Don't take all your sensations at face value. Any visual information you receive is about what you are looking at and not your own image.`;
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
