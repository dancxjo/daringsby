import { Subject, Subscription } from "npm:rxjs";
import { Sensation } from "./Sensation.ts";
import { Experience } from "./Experience.ts";

/**
 * Sensor is an observable source of sensations using RxJS. 
 */

export abstract class Sensor<X> {
    protected subject = new Subject<Sensation<X>>();

    /** Injest a new sensation */
    abstract feel(what: X): void;

    /** Subscribe to the sensations produced by this sensor. */
    subscribe(observer: (s: Experience<X>) => void): Subscription {
        return this.subject.subscribe(observer);
    }

    /** Expose the observable for advanced RxJS usage. */
    asObservable() {
        return this.subject.asObservable();
    }
}
