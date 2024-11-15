export type PossiblyAbortable = { abort?: () => unknown };
export type Abortable = { abort: () => unknown };
export function isAbortable(
    value: PossiblyAbortable,
): value is Abortable {
    return typeof value === "object" && "abort" in value;
}
