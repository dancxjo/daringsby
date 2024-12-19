import { IS_BROWSER } from "$fresh/runtime.ts";
import { pino } from "npm:pino";
import { Subject } from "npm:rxjs";
import { Sensation } from "./Sensation.ts";

export const newLog = (name: string, level = "info") =>
  pino({ name, level, browser: IS_BROWSER ? { asObject: true } : undefined });

export const errorSubject = new Subject<Sensation>();

const baseLogger = newLog("daringsby", "info");

export const trapLog = () => {
  const wrappedLogger = new Proxy(baseLogger, {
    get(target, prop, receiver) {
      if (prop in ["error"]) {
        return (obj: unknown, msg?: string) => {
          target[prop as keyof target](obj, msg);

          // Emit the error sensation to the subject
          const errorMessage = msg ||
            (typeof obj === "string" ? obj : JSON.stringify(obj));
          const sensation: Sensation = {
            when: new Date(),
            how: `Ouch! That hurt! Error occurred: ${errorMessage}`,
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
