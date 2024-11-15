import { pino } from "npm:pino";

export const logger = pino({
    name: "daringsby",
    level: "debug",
});
