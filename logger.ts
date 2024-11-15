import { pino } from "npm:pino";
import { IS_BROWSER } from "$fresh/runtime.ts";

export const logger = pino({
    name: "daringsby",
    level: "debug",
    browser: IS_BROWSER ? { asObject: true } : undefined,
});
