import { map, Observable, OperatorFunction, switchMap } from "npm:rxjs";
import { logger } from "../../../logger.ts";
import { stringify, wholeResponse } from "../chunking.ts";
import { Processor } from "../processors.ts";
import { GenerateTask, Method } from "../tasks.ts";

export interface Stamped<T> {
    at: Date;
    content: T;
}

/**
 * A sensation is just a text description of a sensory experience and a time when it happened.
 * This information should stay together for as long as possible.
 */
export type Sensation = Stamped<string>;

/**
 * A Sense is a preprocessing unit that takes in raw data and processes it into Sensations.
 */
export type Sense = Observable<Sensation>;
export type Instant = Stamped<string>;
export function integrate(
    processor: Processor,
): OperatorFunction<Sensation[], Instant> {
    return (source: Observable<Sensation[]>) =>
        source.pipe(
            switchMap((sensations) => {
                const description = sensations.sort((a, b) =>
                    a.at.getTime() - b.at.getTime()
                ).map((sensation) => `At ${sensation.at}: ${sensation.content}`)
                    .join("\n");
                logger.debug("Integrating sensations");

                const task: GenerateTask = {
                    method: Method.Generate,
                    input: {
                        prompt:
                            `You are a part of an AI system. You are speaking on behalf of the overall system in the present tense and first person, as if you were not an LLM but the whole system. You're pretty smart and can make good inferences about the real world. These are the sensations and thoughts that you have felt over the past moment. These sensations are of the real world. You are in the real world via your senses. Please integrate them into a coherent narrative. Use the present tense and the first person where appropriate. Do not embellish. Use only the information from the description. Do not repeat the prompt. Tell the truth as a reflection of the real world as reflected in the data below. Pay close attention to the timestamps:\n${description}`,
                    },
                    abortController: new AbortController(),
                };

                const COMBOBULATION_MODEL =
                    Deno.env.get("COMBOBULATION_MODEL") ??
                        "gemma2";

                return processor.execute(task, COMBOBULATION_MODEL).pipe(
                    stringify(),
                    wholeResponse(),
                    map((response) => ({
                        at: new Date(
                            sensations[sensations.length - 1].at.getTime(),
                        ),
                        content: response,
                    } as Stamped<string>)),
                );
            }),
        );
}
