import { Observable } from "npm:rxjs";

// TODO: Export our own, simpler (i.e. superclasses of) these types
import type {
    ChatRequest,
    ChatResponse,
    EmbeddingsRequest,
    EmbeddingsResponse,
    EmbedRequest,
    EmbedResponse,
    GenerateRequest,
    GenerateResponse,
} from "npm:ollama";

export type {
    ChatRequest,
    ChatResponse,
    EmbeddingsRequest,
    EmbeddingsResponse,
    EmbedRequest,
    EmbedResponse,
    GenerateRequest,
    GenerateResponse,
};

export function isAsyncIterable<T>(
    value: unknown,
): value is AsyncIterable<T> {
    return !!value && typeof value === "object" &&
        Symbol.asyncIterator in value;
}

export enum Method {
    Generate = "generate",
    Chat = "chat",
    Embed = "embed",
    Embeddings = "embeddings",
}

export interface Task<I, O> {
    method: Method;
    input: Partial<I>; // might be missing model field, for instance
    stream?: Observable<O>;
    abortController: AbortController;
}

export type GenerateTask = Task<GenerateRequest, GenerateResponse>;

export function isGenerateTask(
    task: Task<unknown, unknown>,
): task is GenerateTask {
    return task.method === "generate";
}

export type ChatTask = Task<ChatRequest, ChatResponse>;

export function isChatTask(task: Task<unknown, unknown>): task is ChatTask {
    return task.method === "chat";
}

export type EmbedTask = Task<EmbedRequest, EmbedResponse>;

export function isEmbedTask(task: Task<unknown, unknown>): task is EmbedTask {
    return task.method === "embed";
}

export type EmbeddingsTask = Task<EmbeddingsRequest, EmbeddingsResponse>;

export function isEmbeddingsTask(
    task: Task<unknown, unknown>,
): task is EmbeddingsTask {
    return task.method === "embeddings";
}
