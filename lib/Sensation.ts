/** A single sensation felt by a sensor. */

export interface Sensation<X> {
    /** Timestamp of the sensation */
    when: Date;
    /** Sensory payload */
    what: X;
}
