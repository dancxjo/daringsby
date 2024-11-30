import { Message, Ollama } from "npm:ollama";
import { ReplaySubject } from "npm:rxjs";
import { newLog } from "./logger.ts";

const logger = newLog(import.meta.url, "debug");

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
  // "tinyllama": [Fast, Chat, Generate], // 637 MB
  "nomic-embed-text": [Embed, Fast], // 274 MB
  "llama3.2": [Fast, Chat, Generate], // 2.0 GB
  // "nemotron-mini": [Fast, Chat, Generate], // 2.7 GB
  "llama3.2-vision": [Vision, Chat, Generate], // 7.9 GB
  // "llava:13b": [Vision], // 8.0 GB
  "gemma2": [Chat, Generate], // 5.4 GB
  "gemma2:27b": [Smart, Chat, Generate], // 15 GB
  // "mistral-nemo": [Smart, Chat, Generate], // 7.1 GB
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
  enqueuedAt?: number;
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
    task.enqueuedAt = Date.now();
    this.taskQueues[task.priority].push(task);
    this.processAllTasks();
  }

  // Finds a suitable server instance for the task based on required characteristics
  private async findOptimalInstance(
    required: Characteristics[],
  ): Promise<{ instance: Ollama; model: string } | undefined> {
    // Sort instances by busyness (load in ascending order) and affinity score
    const sortedInstances = this.instances.sort((a, b) => {
      const loadA = this.instanceLoadMap.get(a) ?? 0;
      const loadB = this.instanceLoadMap.get(b) ?? 0;

      // Prioritize instances with lower load, but also consider previous successful completions
      const affinityA = this.getInstanceAffinityScore(a, required);
      const affinityB = this.getInstanceAffinityScore(b, required);

      return loadA + affinityA - (loadB + affinityB);
    });

    // Iterate through sorted instances and try to find a model that meets requirements
    for (const instance of sortedInstances) {
      try {
        // Fetch models from the current instance with a timeout of 5 seconds
        const modelResponse = await Promise.race([
          instance.list(),
          new Promise((_, reject) =>
            setTimeout(() => reject(new Error("Instance list timeout")), 5000)
          ),
        ]);
        logger.debug({ instance, modelResponse }, "Instance list response");

        // If no models are available, continue to the next instance
        if (!modelResponse) {
          continue;
        }

        const response = modelResponse as { models: { name: string }[] };
        const availableModels = response.models.map((model) => model.name);

        // Check each model in the list to see if it meets the requirements
        for (const [modelName, charList] of Object.entries(characteristics)) {
          if (
            (availableModels.includes(modelName) ||
              availableModels.includes(modelName + ":latest")) &&
            required.every((reqChar) => charList.includes(reqChar))
          ) {
            logger.debug(
              { modelName, charList },
              "Model available on current instance",
            );
            // Return the instance and model as soon as a suitable match is found
            return { instance, model: modelName };
          } else {
            logger.debug(
              { modelName, charList },
              "Model not available on current instance",
            );
          }
        }
      } catch (error) {
        // Log any issues finding suitable models for an instance, but continue to the next one
        logger.warn(
          { instance, error },
          `Could not find suitable model for instance`,
        );
      }
    }

    // If no suitable instance and model were found, log an error and return undefined
    logger.error("No suitable instance found for task");
    return undefined;
  }

  private getInstanceAffinityScore(
    instance: Ollama,
    required: Characteristics[],
  ): number {
    // Implement a scoring system that gives preference to instances that have handled similar tasks
    // For now, just return 0, but this can be updated based on successful task completions
    return 0;
  }

  // Processes all tasks in the queue with a round-robin mechanism
  public async processAllTasks(): Promise<void> {
    logger.debug("Starting to process all tasks");
    let hasTasks = true;

    while (hasTasks) {
      hasTasks = false;

      for (let priority = 0; priority < this.taskQueues.length; priority++) {
        const queue = this.taskQueues[priority];
        if (queue && queue.length > 0) {
          hasTasks = true;
          const task = queue.shift(); // Get the next task from the queue
          if (task) {
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
    }
  }

  // Adjusts task priorities to avoid starvation
  private adjustTaskPriorities(): void {
    for (let priority = 0; priority < this.taskQueues.length; priority++) {
      const queue = this.taskQueues[priority];
      if (queue) {
        for (const task of queue) {
          if (this.isTaskStarving(task)) {
            // Move the task to a higher priority if it's been waiting too long
            task.priority = Math.max(0, priority - 1);
          }
        }
      }
    }
  }

  private isTaskStarving(task: Task<unknown>): boolean {
    // Define starvation logic, e.g., based on how long the task has been waiting
    return Date.now() - (task.enqueuedAt ?? 0) > 10000; // Example: waiting for more than 10 seconds
  }

  // Executes a specific task based on its type
  private async executeTask<T>(task: Task<T>): Promise<T> {
    let error: Error | null = null;

    try {
      const optimalInstance = await this.findOptimalInstance(task.required);
      if (!optimalInstance) {
        throw new Error("No suitable server or model found for task");
      }

      const { instance, model } = optimalInstance;
      logger.debug({ model }, "Selected model for task");
      logger.debug({ model }, "Executing task on selected instance");

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
    logger.debug({ model }, "Generating text");
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
    logger.debug({ required }, "Generating text");
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
