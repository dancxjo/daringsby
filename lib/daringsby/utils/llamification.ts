import { Config, Ollama } from "npm:ollama";
import { mergeMap, Observable, OperatorFunction } from "npm:rxjs";

export function llamify(
    model: string = "llama3.2",
    config: Partial<Config> = {},
): OperatorFunction<string, string> {
    const ollama = new Ollama(config);
    return (source: Observable<string>) =>
        source.pipe(
            mergeMap((prompt) => {
                return new Observable<string>((observer) => {
                    ollama.generate({
                        prompt,
                        model,
                        stream: true,
                    }).then((stream) => {
                        (async () => {
                            try {
                                for await (const response of stream) {
                                    observer.next(response.response);
                                }
                                observer.complete();
                            } catch (error) {
                                observer.error(error);
                            }
                        })();
                    }).catch((error) => {
                        observer.error(error);
                    });
                });
            }),
        );
}
