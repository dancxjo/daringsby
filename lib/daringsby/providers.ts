import { Observable } from "npm:rxjs";
import {
    ChatRequest,
    ChatResponse,
    EmbeddingsRequest,
    EmbeddingsResponse,
    EmbedRequest,
    EmbedResponse,
    GenerateRequest,
    GenerateResponse,
    Task,
} from "./tasks.ts";

export interface Provider {
    generate?(
        task: Task<GenerateRequest, GenerateResponse>,
        model: string,
    ): Observable<GenerateResponse>;
    chat?(
        task: Task<ChatRequest, ChatResponse>,
        model: string,
    ): Observable<ChatResponse>;
    embed?(
        task: Task<EmbedRequest, EmbedResponse>,
        model: string,
    ): Observable<EmbedResponse>;
    embeddings?(
        task: Task<EmbeddingsRequest, EmbeddingsResponse>,
        model: string,
    ): Observable<EmbeddingsResponse>;
}

export interface Chatter extends Provider {
    chat(
        task: Task<ChatRequest, ChatResponse>,
        model: string,
    ): Observable<ChatResponse>;
}

export interface Generator extends Provider {
    generate(
        task: Task<GenerateRequest, GenerateResponse>,
        model: string,
    ): Observable<GenerateResponse>;
}

export interface Embedder extends Provider {
    embed(
        task: Task<EmbedRequest, EmbedResponse>,
        model: string,
    ): Observable<EmbedResponse>;

    embeddings(
        task: Task<EmbeddingsRequest, EmbeddingsResponse>,
        model: string,
    ): Observable<EmbeddingsResponse>;
}
