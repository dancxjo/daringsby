import { Message, Ollama } from "npm:ollama";
import { ReplaySubject } from "npm:rxjs";
import logger from "./logger.ts";

export enum Characteristics {
  Fast = "Fast",
  Smart = "Smart",
  Vision = "Vision",
  Embed = "Embed",
  Huge = "Huge",
}

const { Fast, Smart, Vision, Embed, Huge } = Characteristics;

export interface Profile {
  model: string;
  ollama: Ollama;
  tokenRate: number;
  capabilities: Characteristics[];
}

export const characteristics: Record<string, Characteristics[]> = {
  "tinyllama": [Fast], // 637 MB
  "nomic-embed-text": [Embed, Fast], // 274 MB
  "mxbai-embed-large": [Embed, Fast], // 669 MB
  "moondream": [Vision, Fast], // 1.7 GB
  "smollm2": [Fast], // 1.8 GB
  "llama3.2": [Fast], // 2.0 GB
  "orca-mini": [Fast], // 2.0 GB
  "phi3.5": [Fast], // 2.2 GB
  "nemotron-mini": [Fast], // 2.7 GB
  "llama3.2:3b-text-fp16": [], // 6.4 GB
  "llama3.2-vision": [Vision], // 7.9 GB
  "llava:13b": [Vision], // 8.0 GB
  "gemma2": [], // 5.4 GB
  "gemma2:27b": [Smart], // 15 GB
  "mistral-small": [Fast], // 12 GB
  "mistral": [Fast], // 4.1 GB
  "openchat": [Fast], // 4.1 GB
  "llama3": [Fast], // 4.7 GB
  "llama3:instruct": [Fast], // 4.7 GB
  "bakllava": [Fast, Vision], // 4.7 GB
  "mistral-nemo": [Smart], // 7.1 GB
  "llama3.1:70b-instruct-q2_K": [Smart, Huge], // 26 GB
};

export interface GenerationParams {
  prompt: string;
  image?: string;
}

export interface ChatParams {
  messages: Message[];
  image?: string;
}

export interface EmbeddingsParams {
  text: string;
}

export interface Task<T = unknown, P = unknown> {
  method: "generate" | "chat" | "embed";
  params: P;
  priority?: number;
  model?: string;
  required: Characteristics[];
  onError: (error: Error) => Promise<void>;
  onComplete: (response: T) => Promise<void>;
}

export interface GenerationTask extends Task<string, GenerationParams> {
  method: "generate";
}

export function isGenerationTask(
  task: Task<string, GenerationParams>,
): task is GenerationTask {
  return task.method === "generate";
}

export interface ChatTask extends Task<Message, ChatParams> {
  method: "chat";
}

export function isChatTask(task: Task<Message, ChatParams>): task is ChatTask {
  return task.method === "chat";
}

export interface EmbeddingsTask extends Task<number[], EmbeddingsParams> {
  method: "embed";
}

export function isEmbeddingsTask(
  task: Task<number[], EmbeddingsParams>,
): task is EmbeddingsTask {
  return task.method === "embed";
}

export class LinguisticProcessor {
  protected taskQueues: Task<unknown, unknown>[][] = [];
  protected business: Map<Ollama, number> = new Map();

  constructor(protected instances: Ollama[]) {}

  // Adds a new task to the processor queue
  public execute(task: Task<unknown>): void {
    if (task.priority === undefined) {
      task.priority = 5;
    }
    if (this.taskQueues[task.priority] === undefined) {
      this.taskQueues[task.priority] = [];
    }
    const queue = this.taskQueues[task.priority];
    queue.push(task);
  }

  // Finds a suitable model for the task based on required characteristics
  private findModelForTask(required: Characteristics[]): string | undefined {
    for (const [modelName, charList] of Object.entries(characteristics)) {
      if (required.every((reqChar) => charList.includes(reqChar))) {
        return modelName;
      }
    }
    return undefined;
  }

  private async findInstanceForModel(
    model: string,
  ): Promise<Ollama | undefined> {
    this.instances.sort((a, b) => {
      const aBusiness = this.business.get(a) ?? 0;
      const bBusiness = this.business.get(b) ?? 0;
      return aBusiness - bBusiness;
    });
    logger.info(
      { instances: this.instances },
      `Finding instance for model ${model}`,
    );
    for (const instance of this.instances) {
      const modelResponse = await instance.list();
      const models = modelResponse.models.map((model) => model.name);
      // TODO: prefer servers with the speediest token rates on the models
      if (models.includes(model)) {
        return instance;
      }
    }
    return undefined;
  }

  // Process all tasks in the queue
  public async processTasks(): Promise<void> {
    for (const queue of this.taskQueues) {
      for (const task of queue) {
        try {
          const response = await this.processTask(task);
          await task.onComplete(response);
        } catch (e) {
          let error = e;
          if (!(e instanceof Error)) {
            error = new Error(JSON.stringify(error));
          }
          await task.onError(error as Error);
        }
      }
    }
  }

  private async processTask<T>(task: Task<T>): Promise<T> {
    const model = task.model || this.findModelForTask(task.required);
    if (!model) {
      throw new Error("No suitable model found for task");
    }
    const instance = await this.findInstanceForModel(model);
    if (!instance) {
      throw new Error("No instance found for model");
    }
    const chunks = new ReplaySubject<string>();

    if (isGenerationTask(task as Task<string, GenerationParams>)) {
      const generationTask = task as GenerationTask;
      let outstandingJobs = this.business.get(instance) ?? 0;
      this.business.set(instance, outstandingJobs + 1);
      const stream = await instance.generate({
        prompt: generationTask.params.prompt,
        stream: true,
        model,
      });
      let buffer = "";
      for await (const chunk of stream) {
        chunks.next(chunk.response);
        buffer += chunk.response;
      }
      // TODO: Measure timing around here
      outstandingJobs = this.business.get(instance) ?? 0;
      this.business.set(instance, outstandingJobs - 1);
      return buffer as T;
    } else if (isChatTask(task as Task<Message, ChatParams>)) {
      const chatTask = task as ChatTask;
      let outstandingJobs = this.business.get(instance) ?? 0;
      this.business.set(instance, outstandingJobs + 1);
      const stream = await instance.chat({
        messages: chatTask.params.messages,
        model,
        stream: true,
      });
      let buffer = "";
      for await (const chunk of stream) {
        chunks.next(chunk.message.content);
        buffer += chunk.message.content;
      }
      outstandingJobs = this.business.get(instance) ?? 0;
      this.business.set(instance, outstandingJobs - 1);
      return { role: "assistant", content: buffer } as T;
    } else if (isEmbeddingsTask(task as Task<number[], EmbeddingsParams>)) {
      const embeddingsTask = task as EmbeddingsTask;
      let outstandingJobs = this.business.get(instance) ?? 0;
      this.business.set(instance, outstandingJobs + 1);
      const embeddings = await instance.embeddings({
        prompt: embeddingsTask.params.text,
        model,
      });
      outstandingJobs = this.business.get(instance) ?? 0;
      this.business.set(instance, outstandingJobs - 1);
      return embeddings.embedding as T;
    } else {
      throw new Error("Invalid task method");
    }
  }
}
