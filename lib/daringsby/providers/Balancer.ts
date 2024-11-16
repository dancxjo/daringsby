import { Provider } from "../providers.ts";
import { BehaviorSubject, filter, map, mergeMap, Observable } from "npm:rxjs";
import { Method, Task } from "../tasks.ts";
import { logger } from "../../../logger.ts";
import { Processor } from "../processors.ts";

export enum ModelCharacteristic {
    Paid,
    VeryFast,
    Fast,
    Slow,
    VerySlow,
    Vision,
    Embeddings,
    Chat,
    HighQuality,
    LowQuality, // as in the reliability of the returned results
}

export const modelCharacteristics: { [key: string]: Set<ModelCharacteristic> } =
    {
        "llama3.1:70b-instruct-q2_K": new Set([
            ModelCharacteristic.VerySlow,
            ModelCharacteristic.HighQuality,
            ModelCharacteristic.Chat,
        ]),
        "llama3.2": new Set([
            ModelCharacteristic.Embeddings,
            ModelCharacteristic.Chat,
            ModelCharacteristic.VeryFast,
            ModelCharacteristic.LowQuality,
        ]),
        "llama3.2-vision": new Set([
            ModelCharacteristic.Vision,
            ModelCharacteristic.Slow,
            ModelCharacteristic.HighQuality,
        ]),
        "llava": new Set([
            ModelCharacteristic.Vision,
            ModelCharacteristic.VeryFast,
            ModelCharacteristic.LowQuality,
        ]),
        "llava:13b": new Set([
            ModelCharacteristic.Vision,
            ModelCharacteristic.Fast,
            ModelCharacteristic.LowQuality,
        ]),
        "gemma2:27b": new Set([
            ModelCharacteristic.Slow,
            ModelCharacteristic.HighQuality,
            ModelCharacteristic.Chat,
        ]),
        "phi3.5": new Set([
            ModelCharacteristic.VeryFast,
            ModelCharacteristic.LowQuality,
            ModelCharacteristic.Chat,
        ]),
        "tinyllama": new Set([
            ModelCharacteristic.VeryFast,
            ModelCharacteristic.LowQuality,
            ModelCharacteristic.Chat,
        ]),
        "openchat": new Set([
            ModelCharacteristic.Chat,
            ModelCharacteristic.VeryFast,
            ModelCharacteristic.LowQuality,
        ]),
        "nemotron-mini": new Set([
            ModelCharacteristic.Embeddings,
            ModelCharacteristic.Fast,
        ]),
        "mxbai-embed-large": new Set([
            ModelCharacteristic.Embeddings,
            ModelCharacteristic.Fast,
        ]),
    };

export enum Status {
    Offline,
    Available,
    Busy,
}

export interface Balanceable extends Processor {
    availableModel$: Observable<string[]>;
    status$: Observable<Status>;
    availableModels: string[];
}

export class Balancer extends Processor {
    protected availableProcessors = new BehaviorSubject<Balanceable[]>([]);
    protected available: Status[] = [];

    constructor(protected providers: Balanceable[]) {
        super();
        // Subscribe to provider status and update available providers list accordingly
        providers.forEach((provider, i) => {
            provider.status$.subscribe((status) => {
                this.available[i] = status;
                this.updateAvailableProcessors();
            });
        });
    }

    override execute<I, O>(task: Task<I, O>, model: string): Observable<O> {
        return this.getNextAvailableProcessor(task).pipe(
            mergeMap((processor) => {
                return processor.execute(task, model);
            }),
        );
    }

    private updateAvailableProcessors(): void {
        const availableProviders = this.providers.filter((_, i) =>
            this.available[i] === Status.Available
        );
        this.availableProcessors.next(availableProviders);
    }

    private chooseModel(task: Task, availableModels: string[]): string | null {
        let candidates = [...availableModels];

        // Filter required models first
        if (task.requiredModel) {
            candidates = candidates.filter((model) =>
                task.requiredModel!.has(model)
            );
            if (candidates.length > 0) {
                return candidates[0];
            }
        }

        // Remove forbidden models
        if (task.forbiddenModels) {
            candidates = candidates.filter((model) =>
                !task.forbiddenModels!.has(model)
            );
        }

        // Filter by characteristics
        candidates = candidates.filter((model) => {
            if (!(model in modelCharacteristics)) {
                logger.warn(`Unsupported model ${model}`);
                return false;
            }

            const characteristics = modelCharacteristics[model];

            if (
                task.method === Method.Chat &&
                !characteristics.has(ModelCharacteristic.Chat)
            ) {
                return false;
            }

            if (
                task.method === Method.Embed &&
                !characteristics.has(ModelCharacteristic.Embeddings)
            ) {
                return false;
            }

            if (
                task.method === Method.Embeddings &&
                !characteristics.has(ModelCharacteristic.Embeddings)
            ) {
                return false;
            }

            if (
                "images" in task.input &&
                !characteristics.has(ModelCharacteristic.Vision)
            ) {
                return false;
            }

            return true;
        });

        // Sort candidates by speed and quality characteristics
        candidates.sort((a, b) => {
            const speedRank = (model: string) => {
                const characteristics = modelCharacteristics[model];
                if (characteristics.has(ModelCharacteristic.VeryFast)) return 1;
                if (characteristics.has(ModelCharacteristic.Fast)) return 2;
                if (characteristics.has(ModelCharacteristic.Slow)) return 3;
                if (characteristics.has(ModelCharacteristic.VerySlow)) return 4;
                return 5; // Default rank if no speed characteristic
            };

            const qualityRank = (model: string) => {
                const characteristics = modelCharacteristics[model];
                if (characteristics.has(ModelCharacteristic.HighQuality)) {
                    return 1;
                }
                if (characteristics.has(ModelCharacteristic.LowQuality)) {
                    return 2;
                }
                return 3; // Default rank if no quality characteristic
            };

            const speedComparison = speedRank(a) - speedRank(b);
            if (speedComparison !== 0) {
                return speedComparison;
            }

            return qualityRank(a) - qualityRank(b);
        });

        return candidates.length > 0 ? candidates[0] : null;
    }

    getNextAvailableProcessor(task: Task): Observable<Processor> {
        return this.availableProcessors.pipe(
            filter((processors) => processors.length > 0),
            map((processors) => {
                // Shuffle the processors
                processors.sort(() => Math.random() - 0.5);
                const availableModels = processors.flatMap((processor) =>
                    processor.availableModels
                );
                const chosenModel = this.chooseModel(task, availableModels);
                if (!chosenModel) {
                    throw new Error("No available models");
                }
                const nextAvailable = processors.find((processor) =>
                    processor.availableModels.includes(chosenModel)
                );

                if (!nextAvailable) {
                    throw new Error("No available processors");
                }

                return nextAvailable;
            }),
        );
    }
}
