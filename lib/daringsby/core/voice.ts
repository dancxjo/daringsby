import { Ollama } from "npm:ollama";
import { Voice } from "./newt.ts";
import { take } from "npm:rxjs";
import logger from "./logger.ts";

const voice = new Voice(
  new Ollama({
    host: "http://10.0.0.95:11434",
  }),
);

voice.sentences$.subscribe((message) => {
  logger.debug({ message }, "Sending message to main thread");
  self.postMessage({ message });
});

setInterval(async () => {
  logger.info("Thinking of a response...");
  await voice.thinkOfResponse();
  logger.info("Done thinking.");
}, 5000);

// A voice worker to manage conversations in a separate thread.
self.onmessage = async (e) => {
  logger.debug({ e }, "Received message from main thread");
  voice.orient(e.data.context);

  if (e.data.message) {
    logger.debug(
      { message: e.data.message, role: e.data.role },
      "Received message",
    );
    voice.hear({ role: e.data.role, content: e.data.message });
  }
  voice.thinkOfResponse();
};
