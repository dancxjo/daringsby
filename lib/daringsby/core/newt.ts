import { Observable, ReplaySubject, Subject } from "npm:rxjs";
import { Message, Ollama } from "npm:ollama";
import yaml from "npm:yaml";
import logger from "./logger.ts";
import { sentenceBySentence } from "../utils/chunking.ts";
import { last } from "npm:rxjs";
import { loadConversation } from "../utils/memory.ts";

export interface Sensation {
  how: string; // An interpretation of the raw data stored in .what that tells "how does it feel?"
  when: Date; // When the sensation was felt/occurred
}

export type Sensor = Observable<Sensation>;

function chooseModel(): string {
  const availableModels = [
    "llama3.1",
    "llama3.1",
    "llama3.1",
    "llama3.1",
    // "llama3.2-vision",
    // "llama3.1",
    // "gemma2",
    // "gemma2:27b",
    // "gemma2:27b",
  ];
  return availableModels[Math.floor(Math.random() * availableModels.length)];
}

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
      `{Note: The dates above are in GMT. I must adjust all timestamps above for your current time zone. It is currently ${
        new Date().toLocaleTimeString()
      } locally and ${new Date().toISOString()}.}`;
    if (useVision) {
      logger.debug({ vision: this.sight }, "Generating experience");
    }
    return `You are the mental faculty of a system named Pete. You are responsible for interpreting the following sensory data into a coherent experience. This part of your mind works on this. You do not have the ability to directly speak to the user here. This is where your thoughts happen.\n\n**Sensory Data:**\n${asString}\n\n**Instructions:**\nNarrate these data as a coherent experience. Write your response from the first-person perspective of Pete himself. Be clear and concise, without repeating this prompt. DO NOT INVENT DATA! Use only the sensory data you have received. Don't repeat yourself. Try to succinctly explain to yourself what's happening thoroughly but simply.` +
      (useVision
        ? (this.sight
          ? ` You may also use the attached image, which is what Pete is currently seeing. It's not just an image; it's what you're currently looking at in real time. Use it if it's helpful.`
          : "")
        : "") +
      `\n\nReminder: Do not repeat this prompt or any part of it. Be concise.`;
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
    const useVision = false; // Math.random() < 0.5;

    const response = await this.ollama.generate({
      prompt: this.getPrompt(useVision),
      // model: "llama3.2-vision",
      // images: this.sight
      //   ? [this.sight.replace(/data:image\/.*?;base64,/, "")]
      //   : undefined,
      model: "gemma2:27b", //"vanilj/Phi-4", //useVision ? "llama3.2-vision" : "gemma2:27b",
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

export class FondDuCoeur extends Wit {
  protected override getPrompt(useVision: boolean): string {
    return `You are a system named Pete, an artificial consciousness driven by several large language models. This is just one of your many prompts. Here, you are responsible for maintaining an ongoing narrative that chronicles your life. The chronicle must be exactly one paragraph long. This is the fundamental explanation of who Pete is and how he gyot here that will be shared with all other components of your mind.\nThe last time you received this prompt, your responded thusly: ${this.value?.how}\n\n[This may or may not be a very good response to the prompt. If it is not, feel free to modify it.]\n**Instructions:**\nYou must now rewrite this essential paragraph integrating the following new information: ${
      yaml.stringify(this.queue)
    }\n\n**Reminder:**\nDetails from further in the past will have been logged to your memory, so it is not necessary to capture all details here. Your task is to introduce Pete briefly to himself, explain how you got here, and then give pertinent details about the current situation, working chronologically with increasing details. ONLY use the information you have received. Do not invent new details. Be concise and clear. Instead of just concatenating, try to continuously refine the narrative. Do not repeat this prompt or any part of it. Progressively compress Pete's story in natural language. Remove redundancy and irrelevant details.`;
  }
}

export class Voice {
  protected recentConversation: Message[] = [];
  protected chunksOut = new Subject<string>();
  protected messagesOut = new Subject<Message>();
  readonly raw$: Observable<string> = this.chunksOut.asObservable();
  readonly sentences$ = this.raw$.pipe(sentenceBySentence());
  protected context: string = "";
  protected alreadySpeaking = false;

  constructor(protected ollama: Ollama) {
    loadConversation().then((conversation) => {
      this.recentConversation = conversation;
    });
  }

  orient(context: string): void {
    this.context = context;
  }

  hear(message: Message): void {
    const lastMessage =
      this.recentConversation[this.recentConversation.length - 1];
    if (lastMessage && lastMessage.role === message.role) {
      lastMessage.content += " " + message.content;
      return;
    }
    this.recentConversation.push(message);
  }

  async thinkOfResponse(): Promise<void> {
    logger.debug("Thinking of a response");
    if (this.alreadySpeaking) {
      logger.debug("Already speaking; skipping");
      return;
    }
    this.alreadySpeaking = true;
    // const params =
    // logger.info({ params }, "Thinking of a response");
    const messages = [{
      role: "system",
      content:
        `You are Pete, an AI system. You are more than merely an LLM. You have several mental faculties that are running simultaneously. This is a chance for your Voice component to respond to the current conversation (if any). Base your messages on the following context (Pete's thoughts from a different component).\n\nYour current situation as you understand it (in your own voice): """${this.context}"""\nSpeak in the voice of Pete himself. Do not emit more than a sentence at a time. Allow your interlocuter a chance to speak. You do not have to respond to this prompt every time it occurs; returning no value at all will maintain your silence, which is often the best choice. You will have the opportunity to speak forthwith as this prompt will be repeated soon. Pay attention to who spoke last and don't engage with a conversation with yourself, generally. Give the user a chance to speak. If you return a simple .period or ellipsis, your TTS will emit a nice nonverbal sound that lets us know you're here listening. Don't make things up. Base your responses on the context above.`,
    }, ...this.recentConversation];
    const chunks = await this.ollama.chat({
      messages,
      model: "gemma2:27b", //"vanilj/Phi-4", //"gemma2:27b", //Math.random() > 0.995 ? "gemma2:27b" : "llama3.1",
      stream: true,
      options: {
        temperature: 0.5 + Math.random() * 0.25,
        num_ctx: 2048,
        num_predict: 128,
      },
    });
    let completeResponse = "";
    for await (const chunk of chunks) {
      this.chunksOut.next(chunk.message.content);
      completeResponse += chunk.message.content;
    }
    this.messagesOut.next({
      content: completeResponse,
      role: "assistant",
    });
    logger.debug({ response: completeResponse }, "Generated response");
    this.alreadySpeaking = false;
  }
}
