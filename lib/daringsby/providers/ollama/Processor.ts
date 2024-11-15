import {
    ChatRequest,
    ChatResponse,
    EmbeddingsRequest,
    EmbeddingsResponse,
    EmbedRequest,
    EmbedResponse,
    GenerateRequest,
    GenerateResponse,
} from "npm:ollama";
import { Observable } from "npm:rxjs";

import { OllamaClient } from "./Client.ts";
import { Processor } from "../../processors.ts";
import { Task } from "../../tasks.ts";

export class OllamaProcessor extends Processor {
    constructor(private client: OllamaClient) {
        super();
        this.generator = {
            generate: (...args) => {
                return this.generate(...args);
            },
        };
        this.chatter = {
            chat: this.chat.bind(this),
        };
        this.embedder = {
            embed: this.embed.bind(this),
            embeddings: this.embeddings.bind(this),
        };
    }

    generate(
        task: Task<GenerateRequest, GenerateResponse>,
        model: string,
    ): Observable<GenerateResponse> {
        return this.client.generate(task, model);
    }

    chat(
        task: Task<ChatRequest, ChatResponse>,
        model: string,
    ): Observable<ChatResponse> {
        return this.client.chat(task, model);
    }

    embed(
        task: Task<EmbedRequest, EmbedResponse>,
        model: string,
    ): Observable<EmbedResponse> {
        return this.client.embed(task, model);
    }

    embeddings(
        task: Task<EmbeddingsRequest, EmbeddingsResponse>,
        model: string,
    ): Observable<EmbeddingsResponse> {
        return this.client.embeddings(task, model);
    }
}
