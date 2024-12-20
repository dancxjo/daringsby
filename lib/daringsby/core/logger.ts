import { IS_BROWSER } from "$fresh/runtime.ts";
import { pino } from "npm:pino";
import { Subject } from "npm:rxjs";
import { Sensation } from "../core/interfaces.ts";

export const newLog = (name: string, level = "info") =>
  pino({ name, level, browser: IS_BROWSER ? { asObject: true } : undefined });

const errorSubject = new Subject<Sensation<string>>();

const baseLogger = newLog("daringsby");

export const trapLog = () => {
  const wrappedLogger = new Proxy(baseLogger, {
    get(target, prop, receiver) {
      if (prop === "error") {
        return (obj: unknown, msg?: string) => {
          target.error(obj, msg);

          // Emit the error sensation to the subject
          const errorMessage = msg ||
            (typeof obj === "string" ? obj : JSON.stringify(obj));
          const sensation: Sensation<string> = {
            when: new Date(),
            what: `Ouch! That hurt! Error occurred: ${errorMessage}`,
          };
          errorSubject.next(sensation);
        };
      }
      return Reflect.get(target, prop, receiver);
    },
  });

  return wrappedLogger;
};

// Use the wrapped logger
export const logger = trapLog();
export default logger;
