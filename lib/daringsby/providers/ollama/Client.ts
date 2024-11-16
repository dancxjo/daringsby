import { Observable, Subject } from "npm:rxjs";
import {
    ChatRequest,
    ChatResponse,
    EmbeddingsRequest,
    EmbeddingsResponse,
    EmbedRequest,
    EmbedResponse,
    GenerateRequest,
    GenerateResponse,
    Ollama,
} from "npm:ollama";
import { isAbortable, PossiblyAbortable } from "../../abortable.ts";
import { Task } from "../../tasks.ts";
import { logger } from "../../../../logger.ts";

function isAsyncIterable<T>(
    value: unknown,
): value is AsyncIterable<T> {
    return !!value && typeof value === "object" &&
        Symbol.asyncIterator in value;
}

export class OllamaClient {
    private models = new Subject<string[]>();
    constructor(readonly name: string, private ollama: Ollama) {}

    get models$(): Observable<string[]> {
        this.ollama.list().then((response) => {
            this.models.next(response.models.map((model) => model.name));
            this.models.complete();
        }).catch((error) => {
            this.models.error(error);
        });
        return this.models.asObservable();
    }

    private handleStream<T, U>(
        promise: Promise<PossiblyAbortable>,
        subject: Subject<T>,
        task: Task<unknown, U>,
    ) {
        promise.then(
            async (result) => {
                task.abortController.signal.addEventListener("abort", () => {
                    if (isAbortable(result)) result.abort();
                });

                if (isAsyncIterable<T>(result)) {
                    try {
                        for await (const chunk of result) {
                            subject.next(chunk);
                        }
                    } catch (error) {
                        logger.error({ error }, "Probably aborted");
                        // subject.error(error);
                    }
                }

                subject.complete();
            },
            (error) => {
                subject.error(error);
            },
        ).catch((error) => {
            subject.error(error);
        });
    }

    generate(
        task: Task<GenerateRequest, GenerateResponse>,
        model: string,
    ): Observable<GenerateResponse> {
        const subject = new Subject<GenerateResponse>();
        // @ts-ignore TODO: Typecheck this
        const promise = this.ollama.generate({
            ...task.input,
            model,
            stream: true,
        });
        this.handleStream(promise, subject, task);
        task.stream = subject.asObservable();
        return subject.asObservable();
    }

    chat(
        task: Task<ChatRequest, ChatResponse>,
        model: string,
    ): Observable<ChatResponse> {
        const subject = new Subject<ChatResponse>();
        const promise = this.ollama.chat({
            ...task.input,
            model,
            stream: true,
        });
        this.handleStream(promise, subject, task);
        return subject.asObservable();
    }

    embed(
        task: Task<EmbedRequest, EmbedResponse>,
        model: string,
    ): Observable<EmbedResponse> {
        const subject = new Subject<EmbedResponse>();
        const promise = this.ollama.embed(
            { ...task.input, model } as EmbedRequest,
        ).then(
            (result) => {
                subject.next(result);
                subject.complete();
                // Embedding requests are not abortable
                return { ...result, abort: () => {} } as PossiblyAbortable;
            },
        );
        this.handleStream(promise, subject, task);
        return subject.asObservable();
    }

    embeddings(
        task: Task<EmbeddingsRequest, EmbeddingsResponse>,
        model: string,
    ): Observable<EmbeddingsResponse> {
        const subject = new Subject<EmbeddingsResponse>();
        this.ollama.embeddings({ ...task.input, model } as EmbeddingsRequest)
            .then(
                (result) => {
                    subject.next(result);
                    subject.complete();
                },
            ).catch((error) => {
                subject.error(error);
            });
        return subject.asObservable();
    }
}
