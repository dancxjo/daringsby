import { mergeMap, Observable, OperatorFunction } from "npm:rxjs";
import { speak } from "../utils/audio_processing.ts";

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
