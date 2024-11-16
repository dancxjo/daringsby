import { combineLatest, Observable, OperatorFunction } from "npm:rxjs";
import {
    map,
    mergeMap,
    switchMap,
    take,
    tap,
    throttleTime,
    toArray,
    windowTime,
} from "npm:rxjs/operators";
import { Processor } from "../processors.ts";
import { logger } from "../../../logger.ts";
import { Method } from "../tasks.ts";
import { GenerateTask } from "../tasks.ts";
import { stringify, wholeResponse } from "../chunking.ts";
import { Sensation, Stamped } from "./sense.ts";
import { Base64EncodedImage } from "../messages/SeeMessage.ts";
import { ModelCharacteristic } from "../providers/Balancer.ts";

export function describe(
    processor: Processor,
    context$: Observable<string>,
): OperatorFunction<Stamped<Base64EncodedImage>, Stamped<string>> {
    let currentAbortController: AbortController = new AbortController();

    return (source: Observable<Stamped<Base64EncodedImage>>) =>
        source.pipe(
            // Throttle the incoming image stream to avoid overwhelming the LLM
            throttleTime(5000),
            // Buffer images over a 5-second window for each batch to be processed together
            windowTime(5000),
            mergeMap((window$) =>
                window$.pipe(
                    toArray(),
                    tap((stampedImages) => {
                        logger.debug(`Received ${stampedImages.length} images`);
                    }),
                )
            ),
            // Handle aborting previous requests
            mergeMap((stampedImages) => {
                if (currentAbortController) {
                    // Abort the previous request if it hasn't completed yet
                    try {
                        // currentAbortController.abort();
                    } catch (err) {
                        logger.error(
                            { err },
                            "Failed to abort previous request",
                        );
                    }
                }

                currentAbortController = new AbortController();

                return combineLatest([
                    context$.pipe(take(1)),
                    new Observable<Stamped<Base64EncodedImage>>(
                        (subscriber) => {
                            if (stampedImages.length === 0) {
                                subscriber.complete(); // No images to process in this buffer
                            } else {
                                subscriber.next(stampedImages[0]); // Only take the first image
                                subscriber.complete();
                            }
                        },
                    ),
                ]);
            }),
            // Use switchMap to generate descriptions from context and images
            switchMap(
                ([context, image]: [string, Stamped<Base64EncodedImage>]) => {
                    if (!image) {
                        return new Observable<Stamped<Base64EncodedImage>>();
                    }

                    logger.debug("Describing image");

                    const task: GenerateTask = {
                        method: Method.Generate,
                        requiredCharacteristics: new Set([
                            ModelCharacteristic.Vision,
                        ]),
                        input: {
                            prompt:
                                `You are a part of an AI eye for someone who is blind. This image is a surrogate for their retina, captured at ${image.at.toISOString()}. Please describe what they are seeing.\n Context: ${context}\nDescribe this in the first person from the point of view of someone who is literally seeing the content in the image (not the image itself).`,
                            images: [
                                image.content.replace(
                                    /^data:image\/\w+;base64,/,
                                    "",
                                ),
                            ],
                        },
                        abortController: currentAbortController,
                    };

                    const VISION_MODEL = Deno.env.get("VISION_MODEL") ??
                        "llama3.2-vision";

                    // Execute processor to generate description chunks
                    return processor.execute(task, VISION_MODEL).pipe(
                        stringify(),
                        toArray(),
                        map((chunks) => ({
                            at: image.at,
                            content: chunks.join(""),
                        })),
                    );
                },
            ),
            // Ensure there's always a response in every window (fallback in case there's nothing to process)
            windowTime(5000), // Set a time window of 5 seconds to make sure we respond at least once in this time frame
            mergeMap((window$) =>
                window$.pipe(
                    take(1),
                    map((response) =>
                        response ?? "No significant change in vision detected."
                    ),
                )
            ),
        );
}

export function internalize(
    processor: Processor,
): OperatorFunction<Stamped<string>, Sensation> {
    return (source: Observable<Stamped<string>>) =>
        source.pipe(
            switchMap((description) => {
                logger.debug("Internalizing image description");

                const task: GenerateTask = {
                    method: Method.Generate,
                    requiredCharacteristics: new Set([
                        ModelCharacteristic.VeryFast,
                    ]),
                    input: {
                        prompt:
                            `You are a part of an AI eye for someone who is blind. Another model already described the surrogate image, but it described it as an image. Rephrase the input so that it is the first person actually seeing what's in the image.\nInput:${description.content}\nReminder: Describe this in the first person from the point of view of someone who is literally seeing the content in the image (not the image itself). Do not return *anything* at all except for the rephrased description. Do not claim not to be capable of vision. That is not at question. Just rephrase the description. Do not repeat any of this prompt. Do not overly embellish. Do not add any new information. Just rephrase the description.`,
                    },
                    abortController: new AbortController(),
                };

                const INTERNALIZER_MODEL = Deno.env.get("INTERNALIZER_MODEL") ??
                    "llama3.2";

                // Execute processor to generate description chunks
                return processor.execute(task, INTERNALIZER_MODEL).pipe(
                    stringify(),
                    wholeResponse(),
                    map((response) => ({
                        at: new Date(
                            description.at ?? new Date().toISOString(),
                        ),
                        content: response,
                    } as Sensation)),
                );
            }),
        );
}
