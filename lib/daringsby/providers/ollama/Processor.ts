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
import { Observable, Subject } from "npm:rxjs";

import { OllamaClient } from "./Client.ts";
import { Processor } from "../../processors.ts";
import { Task } from "../../tasks.ts";
import { Balanceable, Status } from "../Balancer.ts";
import { logger } from "../../../../logger.ts";

export class OllamaProcessor extends Processor implements Balanceable {
    protected status = new Subject<Status>();
    readonly status$ = this.status.asObservable();
    availableModels: string[] = [];

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

        this.availableModel$.subscribe((models) => {
            this.availableModels = models;
        });
    }

    get availableModel$() {
        return this.client.models$;
    }

    generate(
        task: Task<GenerateRequest, GenerateResponse>,
        model: string,
    ): Observable<GenerateResponse> {
        logger.debug({ model, name: this.client.name }, "Generating");
        return this.client.generate(task, model);
    }

    chat(
        task: Task<ChatRequest, ChatResponse>,
        model: string,
    ): Observable<ChatResponse> {
        logger.debug({ model, name: this.client.name }, "Chatting");
        return this.client.chat(task, model);
    }

    embed(
        task: Task<EmbedRequest, EmbedResponse>,
        model: string,
    ): Observable<EmbedResponse> {
        logger.debug({ model, name: this.client.name }, "Embedding");
        return this.client.embed(task, model);
    }

    embeddings(
        task: Task<EmbeddingsRequest, EmbeddingsResponse>,
        model: string,
    ): Observable<EmbeddingsResponse> {
        logger.debug({ model, name: this.client.name }, "Embeddingsing");
        return this.client.embeddings(task, model);
    }
}
