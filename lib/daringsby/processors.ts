import { Observable } from "npm:rxjs";
import {
    isChatTask,
    isEmbeddingsTask,
    isEmbedTask,
    isGenerateTask,
    Task,
} from "./tasks.ts";
import { Chatter, Embedder, Generator } from "./providers.ts";

export abstract class Processor {
    protected chatter?: Chatter;
    protected generator?: Generator;
    protected embedder?: Embedder;

    execute<I, O>(
        task: Task<I, O>,
        model: string,
    ): Observable<O> {
        if (isGenerateTask(task)) {
            if (!this.generator) {
                throw new Error("No generator available on this processor");
            }
            return this.generator.generate(task, model) as Observable<
                O
            >;
        } else if (isChatTask(task)) {
            if (!this.chatter) {
                throw new Error("No chatter available on this processor");
            }
            return this.chatter.chat(task, model) as Observable<O>;
        } else if (isEmbedTask(task)) {
            if (!this.embedder) {
                throw new Error("No embedder available on this processor");
            }
            return this.embedder.embed(task, model) as Observable<
                O
            >;
        } else if (isEmbeddingsTask(task)) {
            if (!this.embedder) {
                throw new Error("No embeddings available on this processor");
            }
            return this.embedder.embeddings(task, model) as Observable<O>;
        } else {
            throw new Error(`Method not implemented: ${task.method}`);
        }
    }
}
