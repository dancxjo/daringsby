import { Sensor } from "./Sensor.ts";

/**
 * Psyche holds a collection of sensors representing external stimuli.
 */

export class Psyche<X = unknown> {
    private beats = 0;
    private live = true;

    constructor(public externalSensors: Sensor<X>[] = []) { }

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
