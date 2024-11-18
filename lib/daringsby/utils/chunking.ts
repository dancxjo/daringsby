import { Observable, OperatorFunction } from "npm:rxjs";
import { map, switchMap, toArray } from "npm:rxjs/operators";
import { split } from "npm:sentence-splitter";

export interface ResponseObject {
    response: string;
}

export function stringify<T extends ResponseObject>(): OperatorFunction<
    T,
    string
> {
    return (source: Observable<T>) =>
        source.pipe(
            map((obj: { response: string }) => obj.response),
        );
}

export function wholeResponse(): OperatorFunction<string, string> {
    return (source: Observable<string>) =>
        source.pipe(
            toArray(),
            map((chunks) => chunks.join("")),
        );
}

export function verbatim(): OperatorFunction<string, string> {
    return (source: Observable<string>) => {
        let buffer = "";
        return source.pipe(
            switchMap((segment: string) =>
                new Observable<string>((observer) => {
                    const segments = segment.split(/(\s+|[^\w']+)/u);
                    const lastSegment = segments.pop();
                    for (const segment of segments) {
                        if (segment.match(/^[a-zA-Z'0-9]+/)) {
                            buffer += segment;
                        } else {
                            if (buffer) {
                                observer.next(buffer);
                            }
                            buffer = "";
                            if (segment.trim()) {
                                const symbols = segment.split(/(\s+|[^\w']+)/u);
                                for (const symbol of symbols) {
                                    if (symbol.trim()) {
                                        observer.next(symbol);
                                    }
                                }
                            }
                        }
                    }
                    buffer += lastSegment ?? "";
                    return () => {
                        if (buffer) {
                            observer.next(buffer);
                        }
                        observer.complete();
                    };
                })
            ),
        );
    };
}

export function sentenceBySentence(): OperatorFunction<string, string> {
    return (source: Observable<string>) => {
        let buffer = "";
        return new Observable<string>((observer) => {
            const subscription = source.subscribe({
                next(segment) {
                    const text = buffer + segment;
                    const segments = split(
                        text.replace(/\."\s*/, '."\n'),
                    ).map((s) => s.raw);
                    buffer = "";
                    const lastSegment = segments.pop();
                    for (const sentence of segments) {
                        if (sentence.trim()) {
                            observer.next(sentence.trim());
                        }
                    }
                    buffer += lastSegment ?? "";
                },
                error(err) {
                    observer.error(err);
                },
                complete() {
                    if (buffer.trim()) {
                        observer.next(buffer.trim());
                        buffer = "";
                    }
                    observer.complete();
                },
            });
            return () => {
                subscription.unsubscribe();
            };
        });
    };
}
