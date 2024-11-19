import { IS_BROWSER } from "$fresh/runtime.ts";
import { pino } from "npm:pino";

export const newLog = (name: string, level = "info") =>
    pino({ name, level, browser: IS_BROWSER ? { asObject: true } : undefined });

export const logger = newLog("daringsby", "debug");

export default logger;
