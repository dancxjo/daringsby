import { Observable, ReplaySubject } from "npm:rxjs";

export interface Stamped<I> {
    when: Date; // We must always record and maintain the time of the event
    content: I;
}

export interface Described<I> {
    explanation: string; // This is what the text-processing model receives
    content: I; // This is the raw data preserved for further manipulation
}

export type Sensation<I> = Stamped<Described<string>>;
export type Sensitive<I> = ReplaySubject<Sensation<I>>;

export interface Faculty<I = unknown, O = unknown> {
    quick?: Sensitive<I>;
    consult(): Observable<O>;
}
