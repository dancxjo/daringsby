import { Subject, Subscription } from "npm:rxjs";
import { Sensation } from "./Sensation.ts";

/**
 * Sensor is an observable source of sensations using RxJS. A filter predicate
 * can be provided to ignore certain sensations.
 */

export class Sensor<X> {
    protected subject = new Subject<Sensation<X>>();

    constructor(private filter: (s: Sensation<X>) => boolean = () => true) { }

    /** Emit a new sensation if it passes the filter. */
    feel(what: X): void {
        console.log(`Sensor felt: ${what}`);
        const sensation: Sensation<X> = {
            when: new Date(),
            what,
        };
        if (this.filter(sensation)) {
            this.subject.next(sensation);
        }
    }

    /** Subscribe to the sensations produced by this sensor. */
    subscribe(observer: (s: Sensation<X>) => void): Subscription {
        return this.subject.subscribe(observer);
    }

    /** Expose the observable for advanced RxJS usage. */
    asObservable() {
        return this.subject.asObservable();
    }
}
