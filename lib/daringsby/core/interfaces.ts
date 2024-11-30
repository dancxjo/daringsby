export interface Sensation<D = unknown> {
  when: Date; // We must always record and maintain the time of the event
  what: D;
}

export interface Impression<D = unknown> {
  how: string; // This is what the text-processing model receives
  what: Sensation<D>; // This is the raw data preserved for further manipulation
}

export interface Sensitive<D = unknown> {
  feel(sensation: Sensation<D>): Impression<D>;
}
export type Experience = Impression<Impression[]>;
export type Experiencer = Sensitive<Impression[]>;
