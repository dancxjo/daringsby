import { mergeMap, Observable, OperatorFunction } from "npm:rxjs";
import { speak } from "../utils/audio_processing.ts";
import { MessageType } from "../network/messages/MessageType.ts";
import { SayMessage } from "../network/messages/SayMessage.ts";

export function toSayMessage(): OperatorFunction<string, SayMessage> {
  return (source: Observable<string>) => {
    return source.pipe(
      mergeMap(async (sentence: string) => {
        const wav = await speak(sentence);
        return {
          type: MessageType.Say,
          data: {
            words: sentence,
            wav,
          },
        } as SayMessage;
      }),
    );
  };
}
