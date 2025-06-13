import { Sensation } from "../lib/Sensation.ts";

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
