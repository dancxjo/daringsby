import { Observable, ReplaySubject, Subject } from "npm:rxjs";
import { Message, Ollama } from "npm:ollama";
import yaml from "npm:yaml";
import logger from "./logger.ts";
import { sentenceBySentence } from "../utils/chunking.ts";

export interface Sensation {
  how: string; // An interpretation of the raw data stored in .what that tells "how does it feel?"
  when: Date; // When the sensation was felt/occurred
}

export type Sensor = Observable<Sensation>;

function chooseModel(): string {
  const availableModels = [
    // "llama3.2",
    // "llama3.2",
    // "llama3.2",
    // "llama3.2",
    // "llama3.2-vision",
    // "llama3.2",
    // "gemma2",
    // "gemma2:27b",
    "gemma2:27b",
  ];
  return availableModels[Math.floor(Math.random() * availableModels.length)];
}

export class Wit {
  protected queue: Sensation[] = [];
  protected sight: string = "";
  protected experience = new ReplaySubject<Sensation>(1);
  readonly experience$: Observable<Sensation> = this.experience
    .asObservable();

  constructor(protected ollama: Ollama) {
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
    // Process the sensations in the queue
    const asString = yaml.stringify(this.queue) +
      `{Note: The dates above are in GMT. I must adjust all timestamps above for your current time zone. It is currently ${
        new Date().toLocaleTimeString()
      } locally and ${new Date().toISOString()}.}`;
    // logger.info({ vision: this.sight }, "Generating experience");
    const response = await this.ollama.generate({
      prompt:
        `You are the mental faculty of a system named Pete. You are responsible for interpreting the following sensory data into a coherent experience. \n\n**Sensory Data:**\n${asString}\n\n**Instructions:**\nNarrate these data as a coherent experience. Write your response from the first-person perspective of Pete himself. Be clear and concise, without repeating this prompt. DO NOT INVENT DATA! Use only the sensory data you have received.` +
        (this.sight
          ? ` You may also use the attached image, which is what Pete is currently seeing. It's not just an image; it's what you're currently looking at in real time. Use it if it's helpful.`
          : "") +
        `\n\nReminder: Do not repeat this prompt or any part of it. Be concise.`,
      model: "llama3.2-vision",
      images: this.sight
        ? [this.sight.replace(/data:image\/.*?;base64,/, "")]
        : undefined,
    });
    logger.info({ queue: this.queue }, "Processing sensations");
    this.queue.sort((a, b) => a.when.getTime() - b.when.getTime());
    const processed: Sensation = {
      when: this.queue[0].when,
      how: response.response,
    };
    // Clear the queue
    this.queue = [];
    // Emit the processed sensations as an experience
    this.experience.next(processed);
    return;
  }
}

export class Voice {
  protected recentConversation: Message[] = [];
  protected chunksOut = new Subject<string>();
  protected messagesOut = new Subject<Message>();
  readonly raw$: Observable<string> = this.chunksOut.asObservable();
  readonly sentences$ = this.raw$.pipe(sentenceBySentence());
  protected context: string = "";

  constructor(protected ollama: Ollama) {}

  orient(context: string): void {
    this.context = context;
  }

  hear(message: Message): void {
    this.recentConversation.push(message);
  }

  async thinkOfResponse(): Promise<void> {
    const chunks = await this.ollama.chat({
      messages: [{
        role: "system",
        content:
          `You are Pete, an AI system. You are more than merely an LLM. You have several mental faculties that are running simultaneously. This is a chance for your Voice component to respond to the current conversation (if any). Base your messages on the following context (Pete's thoughts from a different component).\n\nContext: ${this.context}\nSpeak in the voice of Pete himself. Do not emit more than a sentence at a time. You will have the opportunity to speak forthwith.`,
      }, ...this.recentConversation],
      model: "gemma2:27b",
      stream: true,
    });
    let completeResponse = "";
    for await (const chunk of chunks) {
      this.chunksOut.next(chunk.message.content);
      completeResponse += chunk.message.content;
    }
    this.messagesOut.next({
      content: completeResponse,
      role: "ai",
    });
  }
}
