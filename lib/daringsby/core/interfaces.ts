export interface Sensation<D = unknown> {
  when: Date; // We must always record and maintain the time of the event
  what: D;
}

export function isSensation<D = unknown>(
  sensation: unknown,
): sensation is Sensation<D> {
  return (
    sensation !== null &&
    typeof sensation === "object" &&
    "when" in sensation &&
    "what" in sensation
  );
}

export interface Impression<D = unknown> {
  how: string; // This is what the text-processing model receives
  what: Sensation<D>; // This is the raw data preserved for further manipulation
}

export function isImpression<D = unknown>(
  impression: unknown,
): impression is Impression<D> {
  return (
    impression !== null &&
    typeof impression === "object" &&
    "how" in impression &&
    "what" in impression &&
    isSensation(impression.what)
  );
}

export interface Sensitive<D = unknown> {
  feel(sensation: Sensation<D>): Promise<Impression<D>>;
}
export type Experience = Impression<Impression[]>;
export type Experiencer = Sensitive<Impression[]>;
