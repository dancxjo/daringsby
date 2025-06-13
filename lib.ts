// lib.ts - pete deno package
//
// This module exposes basic sensory primitives using RxJS for reactive streams.
//
// Example usage:
//
//     import { Sensor } from "./lib.ts";
//     const sensor = new Sensor<string>();
//     sensor.subscribe((s) => console.log(`felt ${s.what} at ${s.when}`));
//     sensor.feel("warmth");
//
import { Subject, Subscription } from "npm:rxjs";

/** A single sensation felt by a sensor. */
export interface Sensation<X> {
  /** Timestamp of the sensation */
  when: Date;
  /** Sensory payload */
  what: X;
}

/**
 * An experience is a collection of sensations with a description of how they
 * feel together.
 */
export interface Experience<X> {
  /** The sensations that make up the experience */
  what: Sensation<X>[];
  /** A sentence describing how the sensations feel together */
  how: string;
}

/**
 * Sensor is an observable source of sensations using RxJS. A filter predicate
 * can be provided to ignore certain sensations.
 */
export class Sensor<X> {
  private subject = new Subject<Sensation<X>>();

  constructor(private filter: (s: Sensation<X>) => boolean = () => true) {}

  /** Emit a new sensation if it passes the filter. */
  feel(what: X): void {
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

/**
 * Psyche holds a collection of sensors representing external stimuli.
 */
export class Psyche<X = unknown> {
  private beats = 0;
  private live = true;

  constructor(public externalSensors: Sensor<X>[] = []) {}

  /** How many beats have occurred. */
  get beatCount(): number {
    return this.beats;
  }

  /** Whether the psyche should keep running. */
  isLive(): boolean {
    return this.live;
  }

  /** Stop the psyche's run loop. */
  stop(): void {
    this.live = false;
  }

  /** Increment the internal beat counter. */
  beat(): void {
    this.beats++;
  }

  /**
   * Continuously run while the psyche is live.
   *
   * ```ts
   * const psyche = new Psyche();
   * psyche.run();
   * psyche.stop();
   * ```
   */
  async run(): Promise<void> {
    while (this.isLive()) {
      this.beat();
      await new Promise((res) => setTimeout(res, 0));
    }
  }
}
