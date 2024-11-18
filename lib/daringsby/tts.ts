import {
    catchError,
    filter,
    mergeMap,
    Observable,
    of,
    OperatorFunction,
} from "npm:rxjs";
import { GenerateTask, Method } from "./tasks.ts";
import { sentenceBySentence, stringify } from "./chunking.ts";
import { Processor } from "./processors.ts";
import { logger } from "../../logger.ts";
import { speak } from "./audio_processing.ts";
import { ModelCharacteristic } from "./providers/Balancer.ts";

export function sanitize(
    processor: Processor,
): OperatorFunction<string, string> {
    return (source: Observable<string>) => {
        return source.pipe(
            sentenceBySentence(),
            mergeMap((sentence: string) => {
                const task: GenerateTask = {
                    method: Method.Generate,
                    requiredCharacteristics: new Set([
                        ModelCharacteristic.VeryFast,
                    ]),
                    input: {
                        prompt:
                            `This text will be passed to a TTS engine that has trouble reading. Return exactly and only a sanitized version of the input text. For example, if given "Sen. John K. Smith, III (D-OH) said, 'I am a U.S. Senator since 1976. I spent $100bn every year'", return "Senator John K. Smith the third (Democrat-Ohio) said, 'I am a United States Senator since nineteen seventy-six. I spent one hundred billion dollars every year'". Here is the sentence: ${sentence}\nReminder: Return *only* the sentence and nothing else. Do not say "sure" or "okay" or anything else. Do not repeat any of this prompt. Only include actual words to be spoken and minimal punctuation. If there is no text to summarize return an empty response: do not say anything at all. (Under no circumstances should you use the example sentences above!) Check your response to be sure it follows all these rules!`,
                    },
                    abortController: new AbortController(),
                };
                const SANITIZER_MODEL = Deno.env.get("SANITIZER_MODEL") ||
                    Deno.env.get("OLLAMA_MODEL") || "llama3.2";
                const execution = processor.execute(task, SANITIZER_MODEL);
                return execution.pipe(
                    stringify(),
                    sentenceBySentence(),
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

export function toEncodedWav(): OperatorFunction<string, string> {
    return (source: Observable<string>) => {
        return source.pipe(
            mergeMap(async (sentence: string) => {
                const spoken = await speak(sentence);
                return spoken;
            }),
        );
    };
}
