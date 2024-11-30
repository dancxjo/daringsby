import { Message, Ollama } from "npm:ollama";
import { ReplaySubject } from "npm:rxjs";
import logger from "./logger.ts";

export enum Characteristics {
  Fast = "Fast",
  Smart = "Smart",
  Vision = "Vision",
  Embed = "Embed",
  Huge = "Huge",
  Generate = "Generate",
  Chat = "Chat",
}

const { Fast, Smart, Vision, Embed, Huge, Chat, Generate } = Characteristics;

export interface Profile {
  model: string;
  ollama: Ollama;
  tokenRate: number;
  capabilities: Characteristics[];
}

export const characteristics: Record<string, Characteristics[]> = {
  "tinyllama": [Fast, Chat, Generate], // 637 MB
  "nomic-embed-text": [Embed, Fast], // 274 MB
  // "mxbai-embed-large": [Embed, Fast], // 669 MB
  "moondream": [Vision, Fast], // 1.7 GB
  "smollm2": [Fast, Chat, Generate], // 1.8 GB
  "llama3.2": [Fast, Chat, Generate], // 2.0 GB
  "orca-mini": [Fast, Chat, Generate], // 2.0 GB
  "phi3.5": [Fast, Chat, Generate], // 2.2 GB
  "nemotron-mini": [Fast, Chat, Generate], // 2.7 GB
  "llama3.2:3b-text-fp16": [Chat, Generate], // 6.4 GB
  "llama3.2-vision": [Vision, Chat, Generate], // 7.9 GB
  "llava:13b": [Vision], // 8.0 GB
  "gemma2": [Chat, Generate], // 5.4 GB
  "gemma2:27b": [Smart, Chat, Generate], // 15 GB
  "mistral-small": [Fast, Chat, Generate], // 12 GB
  "mistral": [Fast, Chat, Generate], // 4.1 GB
  "openchat": [Fast, Chat, Generate], // 4.1 GB
  "llama3": [Fast, Chat, Generate], // 4.7 GB
  "llama3:instruct": [Fast, Chat, Generate], // 4.7 GB
  "bakllava": [Fast, Vision, Chat, Generate], // 4.7 GB
  "mistral-nemo": [Smart, Chat, Generate], // 7.1 GB
  "llama3.1:70b-instruct-q2_K": [Smart, Huge, Chat, Generate], // 26 GB
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
  protected instanceLoadMap: Map<Ollama, number> = new Map();

  constructor(protected instances: Ollama[]) {}

  // Adds a new task to the processor queue
  public enqueueTask(task: Task<unknown>): void {
    logger.debug({ task }, "Enqueuing new task");
    if (task.priority === undefined) {
      task.priority = 5;
    }
    if (this.taskQueues[task.priority] === undefined) {
      this.taskQueues[task.priority] = [];
    }
    this.taskQueues[task.priority].push(task);
    this.processAllTasks();
  }

  // Finds a suitable server instance for the task based on required characteristics
  private async findOptimalInstance(
    required: Characteristics[],
  ): Promise<{ instance: Ollama; model: string } | undefined> {
    for (const instance of this.instances) {
      try {
        const modelResponse = await Promise.race([
          instance.list(),
          new Promise((_, reject) =>
            setTimeout(() => reject(new Error("Instance list timeout")), 5000)
          ),
        ]);
        logger.debug({ instance, modelResponse }, "Instance list response");
        if (!modelResponse) {
          continue;
        }
        const response = modelResponse as { models: { name: string }[] };
        const availableModels = response.models.map((model) => model.name);
        availableModels.sort((a, b) => {
          if (Math.random() > 0.5) return -1;
          return 1;
        });
        for (const [modelName, charList] of Object.entries(characteristics)) {
          if (
            (availableModels.includes(modelName) ||
              availableModels.includes(modelName + ":latest")) &&
            required.every((reqChar) => charList.includes(reqChar))
          ) {
            logger.info({ modelName, charList }, "Model available");
            return { instance, model: modelName };
          } else {
            logger.info({ modelName, charList }, "Model not available");
          }
        }
      } catch (error) {
        logger.warn(
          { instance, error },
          `Could not find suitable model for instance`,
        );
      }
    }
    logger.error("No suitable instance found for task");
    return undefined;
  }

  // Processes all tasks in the queue
  public async processAllTasks(): Promise<void> {
    logger.debug("Starting to process all tasks");
    for (const queue of this.taskQueues) {
      if (!queue) {
        continue;
      }
      for (const task of queue) {
        try {
          const response = await this.executeTask(task);
          await task.onComplete(response);
          logger.debug({ task }, "Task completed successfully");
        } catch (e) {
          let error = e;
          if (!(e instanceof Error)) {
            error = new Error(JSON.stringify(error));
          }
          await task.onError(error as Error);
          logger.error({ task, error }, "Task execution failed");
        }
      }
    }
  }

  // Executes a specific task based on its type
  private async executeTask<T>(task: Task<T>): Promise<T> {
    let error: Error | null = null;

    for (const instance of this.instances) {
      try {
        const optimalInstance = await this.findOptimalInstance(task.required);
        if (!optimalInstance) {
          throw new Error("No suitable server or model found for task");
        }

        const { instance, model } = optimalInstance;
        logger.debug({ task, model }, "Selected model for task");
        logger.debug(
          { model, instance },
          "Executing task on selected instance",
        );

        const chunks = new ReplaySubject<string>();
        return await this.wrapWithLoadManagement(instance, async () => {
          if (isGenerationTask(task as Task<string, GenerationParams>)) {
            return await this.executeGenerationTask(
              instance,
              task as GenerationTask,
              model,
              chunks,
            ) as T;
          } else if (isChatTask(task as Task<Message, ChatParams>)) {
            return await this.executeChatTask(
              instance,
              task as ChatTask,
              model,
              chunks,
            ) as T;
          } else if (
            isEmbeddingsTask(task as Task<number[], EmbeddingsParams>)
          ) {
            return await this.executeEmbeddingsTask(
              instance,
              task as EmbeddingsTask,
              model,
            ) as T;
          } else {
            throw new Error("Invalid task method");
          }
        });
      } catch (e) {
        error = e instanceof Error ? e : new Error(JSON.stringify(e));
        logger.error({ error, task }, "Failed to execute task on instance");
      }
    }

    // If we exhausted all instances and failed, throw the last error
    if (error) {
      throw error;
    }

    throw new Error("Unexpected error in executing task");
  }

  // Manages load for a given instance during task execution
  private async wrapWithLoadManagement<T>(
    instance: Ollama,
    taskExecution: () => Promise<T>,
  ): Promise<T> {
    let outstandingJobs = this.instanceLoadMap.get(instance) ?? 0;
    this.instanceLoadMap.set(instance, outstandingJobs + 1);
    try {
      return await taskExecution();
    } finally {
      outstandingJobs = this.instanceLoadMap.get(instance) ?? 0;
      this.instanceLoadMap.set(instance, outstandingJobs - 1);
    }
  }

  // Executes a generation task
  private async executeGenerationTask(
    instance: Ollama,
    task: GenerationTask,
    model: string,
    chunks: ReplaySubject<string>,
  ): Promise<string> {
    const rawImage =
      task.params.image?.replace(/^data:image\/\w+;base64,/, "") ?? "";
    const stream = await instance.generate({
      prompt: task.params.prompt,
      stream: true,
      model,
      images: task.params.image ? [rawImage] : undefined,
    });
    logger.info({ model }, "Generating text");
    let buffer = "";
    for await (const chunk of stream) {
      chunks.next(chunk.response);
      buffer += chunk.response;
    }
    return buffer;
  }

  // Executes a chat task
  private async executeChatTask(
    instance: Ollama,
    task: ChatTask,
    model: string,
    chunks: ReplaySubject<string>,
  ): Promise<Message> {
    const stream = await instance.chat({
      messages: task.params.messages,
      model,
      stream: true,
    });
    let buffer = "";
    for await (const chunk of stream) {
      chunks.next(chunk.message.content);
      buffer += chunk.message.content;
    }
    return { role: "assistant", content: buffer };
  }

  // Executes an embeddings task
  private async executeEmbeddingsTask(
    instance: Ollama,
    task: EmbeddingsTask,
    model: string,
  ): Promise<number[]> {
    const embeddings = await instance.embeddings({
      prompt: task.params.text,
      model,
    });
    return embeddings.embedding;
  }

  generate(params: GenerationParams): Promise<string> {
    const required = [Generate];
    if (params.image) required.push(Vision);
    logger.info({ required }, "Generating text");
    return new Promise((resolve, reject) => {
      this.enqueueTask({
        method: "generate",
        params,
        required,
        onError: async (reason) => reject(reason),
        onComplete: async (response) => {
          if (typeof response !== "string") reject();
          else resolve(response);
        },
      });
    });
  }

  chat(params: ChatParams): Promise<Message> {
    return new Promise((resolve, reject) => {
      this.enqueueTask({
        method: "chat",
        params,
        required: [Chat],
        onError: async (reason) => reject(reason),
        onComplete: async (response) => {
          if (!isMessage(response)) reject();
          else resolve(response);
        },
      });
    });
  }

  async vectorize(params: EmbeddingsParams): Promise<number[]> {
    return new Promise((resolve, reject) => {
      const task: EmbeddingsTask = {
        method: "embed",
        params,
        required: [Embed],
        onError: async (reason: Error) => reject(reason),
        onComplete: async (response: number[]) => resolve(response),
      };
      this.enqueueTask(task as Task<unknown, unknown>);
    });
  }
}

function isMessage(message: unknown): message is Message {
  return (
    typeof message === "object" &&
    message !== null &&
    "role" in message &&
    "content" in message
  );
}
