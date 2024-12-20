import { Ollama } from "npm:ollama";
import { Voice } from "./voice.ts";
import logger from "./logger.ts";

const voice = new Voice(
  new Ollama({
    host: "http://10.0.0.95:11434",
  }),
);

voice.messagesOut$.subscribe((message) => {
  logger.debug({ message }, "Sending message to main thread");
  self.postMessage({ message: message.content });
});

voice.thought$.subscribe((thought) => {
  logger.debug({ thought }, "Sending thought to main thread");
  self.postMessage({ thought });
});

async function tick() {
  await voice.think();
  setTimeout(tick, 1000);
}

tick();

// A voice worker to manage conversations in a separate thread.
self.onmessage = async (e) => {
  logger.debug({ e }, "Received message from main thread");
  voice.orient(e.data.context);
  let last = "";
  voice.mien$.subscribe((mien) => {
    if (mien === last) {
      return;
    }
    last = mien;
    logger.debug({ mien }, "Sending");
    self.postMessage({ mien });
  });
  if (e.data.message) {
    logger.debug(
      { message: e.data.message, role: e.data.role },
      "Received message",
    );
    voice.hear({ role: e.data.role, content: e.data.message });
  }
};
