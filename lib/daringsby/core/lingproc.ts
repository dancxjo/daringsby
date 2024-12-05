import { Message, Ollama } from "npm:ollama";
import { ReplaySubject } from "npm:rxjs";
import { logger } from "./logger.ts";

export enum Characteristic {
  Fast = "Fast",
  Smart = "Smart",
  Vision = "Vision",
  Embed = "Embed",
  Huge = "Huge",
  Generate = "Generate",
  Chat = "Chat",
  Code = "Code",
}

const { Fast, Smart, Vision, Embed, Code, Chat, Generate } = Characteristic;

export interface Profile {
  model: string;
  ollama: Ollama;
  tokenRate: number;
  capabilities: Characteristic[];
}

export const characteristics: Record<string, Characteristic[]> = {
  // "tinyllama:latest": [Fast, Chat, Generate], // 637 MB
  "nomic-embed-text:latest": [Embed, Fast], // 274 MB
  "llama3.2:latest": [Fast, Chat, Generate, Code], // 2.0 GB
  // "mistral:latest": [Fast, Chat, Generate], // 2.7 GB
  "llama3.2-vision:latest": [Vision, Generate], // 7.9 GB
  "codellama:latest": [Fast, Chat, Generate, Code], // 3.3 GB
  // "llama3.1:70b-instruct-q2_K": [Smart, Chat, Generate],
  // "llava:13b:latest": [Vision, Generate], // 8.0 GB
  // "phi3.5:latest": [Chat, Fast, Generate],
  "gemma2:latest": [Chat, Fast, Generate, Code], // 5.4 GB
  "gemma2:27b": [Smart, Chat, Generate, Code], // 15 GB
  // "mistral-nemo:latest": [Smart, Chat, Generate], // 7.1 GB
  // "qwq:latest": [Huge, Chat, Generate], // 20 GB
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
  required: Characteristic[];
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
    required: Characteristic[],
  ): Promise<{ instance: Ollama; model: string } | undefined> {
    // return {
    //   instance: this.instances[0],
    //   model: Vision in required
    //     ? "llama3.2-vision:latest"
    //     : (Embed in required ? "nomic-embed-text:latest" : "gemma2:27b"),
    // };
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
        // First, try to get a valid model from instance.ps
        const psResponse = await Promise.race([
          instance.ps(),
          new Promise((_, reject) =>
            setTimeout(() => reject(new Error("Instance ps timeout")), 10000)
          ),
        ]);
        logger.debug({ instance, psResponse }, "Instance ps response");

        if (psResponse) {
          const response = psResponse as { models: { name: string }[] };
          const availableModels = response.models.map((model) => model.name);

          // Check each model in the list to see if it meets the requirements
          for (const [modelName, charList] of Object.entries(characteristics)) {
            logger.debug(
              { modelName, required, charList },
              "Checking if model meets required characteristics",
            );
            if (
              (availableModels.includes(modelName)) &&
              required.every((reqChar) => charList.includes(reqChar))
            ) {
              logger.debug(
                { modelName, charList },
                "Model available on current instance via ps",
              );
              return { instance, model: modelName };
            }
          }
        }

        // If no valid model found with instance.ps, then use instance.list
        const modelResponse = await Promise.race([
          instance.list(),
          new Promise((_, reject) =>
            setTimeout(() => reject(new Error("Instance list timeout")), 10000)
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
          logger.debug(
            { modelName, required, charList },
            "Checking if model meets required characteristics",
          );
          if (
            (availableModels.includes(modelName)) &&
            required.every((reqChar) => charList.includes(reqChar))
          ) {
            logger.debug(
              { modelName, charList },
              "Model available on current instance via list",
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
        // logger.warn(
        //   { instance, error },
        //   `Could not find suitable model for instance`,
        // );
      }
    }

    // If no suitable instance and model were found, log an error and return undefined
    logger.error({ task: required }, "No suitable instance found for task");
    return undefined;
  }

  private getInstanceAffinityScore(
    instance: Ollama,
    required: Characteristic[],
  ): number {
    return 0;
    // Implement a scoring system that gives preference to instances that have handled similar tasks
    // For now, return a value based on previous successful task completions (placeholder logic)
    // let score = 0;
    // if (instance.hasHandledTaskWith(required)) {
    //   score += 10;
    // }
    // return score;
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
              const response = await this.executeTask(task).catch((e) => {
                throw e;
              });
              await task.onComplete(response);
              logger.debug({ task }, "Task completed successfully");
            } catch (e) {
              let error = e;
              if (!(e instanceof Error)) {
                error = new Error(JSON.stringify(error));
              }
              await task.onError(error as Error);
              logger.error({ task, error }, "Task execution failed");
              // Instead of crashing, log the failure and continue processing other tasks
              continue;
            }
          }
        }
      }
    }
  }

  // Executes a specific task based on its type
  private async executeTask<T>(task: Task<T>): Promise<T | void> {
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

    // If we exhausted all instances and failed, log a warning instead of throwing
    if (error) {
      logger.warn(
        { error, task },
        "No instances available to execute the task, moving on",
      );
    }

    // throw new Error("Unexpected error in executing task");
    task.onError(error as Error);
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
      options: {
        num_ctx: 2048,
        // temperature: 0.75 + (Math.random() * 0.5 - 0.25),
      },
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
      options: {
        num_ctx: 4096,
        temperature: 0.7,
      },
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

  generate(
    params: GenerationParams,
    extraRequirements: Characteristic[] = [],
  ): Promise<string> {
    const required = [Generate, ...extraRequirements];
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

  chat(
    params: ChatParams,
    extraRequirements: Characteristic[] = [],
  ): Promise<Message> {
    return new Promise((resolve, reject) => {
      this.enqueueTask({
        method: "chat",
        params,
        required: [Chat, ...extraRequirements],
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

export function isMessage(message: unknown): message is Message {
  return (
    typeof message === "object" &&
    message !== null &&
    "role" in message &&
    "content" in message
  );
}
