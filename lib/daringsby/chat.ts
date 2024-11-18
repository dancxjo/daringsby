import { Message } from "npm:ollama";
import { map, Observable, OperatorFunction, switchMap } from "npm:rxjs";
import { logger } from "../../logger.ts";
import { sentenceBySentence, stringify } from "./chunking.ts";
import { Processor } from "./processors.ts";
import { ModelCharacteristic } from "./providers/Balancer.ts";
import { Stamped } from "./senses/sense.ts";
import { ChatTask, Method } from "./tasks.ts";

export type IntentionToSay = string;

export function chitChat(
    processor: Processor,
    context$: Observable<string>,
): OperatorFunction<Message[], Stamped<IntentionToSay>> {
    return (source: Observable<Message[]>) => {
        return context$.pipe(
            switchMap((context) =>
                source.pipe(
                    switchMap((messages) => {
                        logger.debug("Consulting the LLM for a response");
                        const task: ChatTask = {
                            method: Method.Chat,
                            requiredCharacteristics: new Set([
                                ModelCharacteristic.Fast,
                                ModelCharacteristic.Chat,
                            ]),
                            input: {
                                messages: [{
                                    role: "system",
                                    content:
                                        `This is the situation you find yourself in: ${context}`,
                                }, ...messages],
                            },
                            abortController: new AbortController(),
                        };

                        const CHATTY_CATHEY = Deno.env.get("CHATTY_CATHEY") ??
                            "gemma2";
                        let iWasGoingToSay = "";

                        // Execute processor to generate description chunks
                        return processor.execute(task, CHATTY_CATHEY).pipe(
                            map((response) => {
                                iWasGoingToSay += response.message.content;
                                return { response: response.message.content };
                            }),
                            stringify(),
                            sentenceBySentence(),
                            map((sentence) => ({
                                at: new Date(),
                                content: sentence,
                            } as Stamped<IntentionToSay>)),
                        );
                    }),
                )
            ),
        );
    };
}
