import {
    buffer,
    catchError,
    filter,
    finalize,
    map,
    mergeMap,
    Observable,
    of,
    OperatorFunction,
    switchMap,
    tap,
} from "npm:rxjs";
import {
    GenerateRequest,
    GenerateResponse,
    GenerateTask,
    Method,
    Task,
} from "./tasks.ts";
import { stringify, toSentences } from "./chunking.ts";
import { Processor } from "./processors.ts";
import { logger } from "../../logger.ts";

export function sanitize(
    processor: Processor,
): OperatorFunction<string, string> {
    return (source: Observable<string>) => {
        return source.pipe(
            toSentences(),
            mergeMap((sentence: string) => {
                const task: GenerateTask = {
                    method: Method.Generate,
                    input: {
                        prompt:
                            `This text will be passed to a TTS engine that has trouble reading. Return exactly and only a sanitized version of the input text. For example, if given "Sen. John K. Smith, III (D-OH) said, 'I am a U.S. Senator since 1976. I spent $100bn every year'", return "Senator John K. Smith the third (Democrat-Ohio) said, 'I am a United States Senator since nineteen seventy-six. I spent one hundred billion dollars every year'". Here is the sentence: ${sentence}\nReminder: Return *only* the sentence and nothing else. Do not say "sure" or "okay" or anything else. Do not repeat any of this prompt. Only include actual words to be spoken and minimal punctuation.`,
                    },
                    abortController: new AbortController(),
                };
                const execution = processor.execute(task, "gemma2:27b");
                return execution.pipe(
                    stringify(),
                    toSentences(),
                    catchError((error) => {
                        logger.error("Error in execution:", error);
                        return of("");
                    }),
                );
            }),
            filter((sentence) => sentence.trim().length > 0),
        );
    };
}
