import { Message, Ollama } from "npm:ollama";
import yaml from "npm:yaml";
import { SocketConnection } from "../network/sockets/connection.ts";
import { addSession, Session, sessions } from "../network/Sessions.ts";
import { newLog } from "../core/logger.ts";
import { MessageType } from "../network/messages/MessageType.ts";
import { speak } from "../utils/audio_processing.ts";
import handleIncomingGeolocationMessages from "../network/handlers/geolocation.ts";
import handleIncomingSeeMessages from "../network/handlers/images.ts";
import handleIncomingSenseMessages from "../network/handlers/sense.ts";
import handleIncomingTextMessages from "../network/handlers/text.ts";
import { SocketMessage } from "../network/messages/SocketMessage.ts";
import { FondDuCoeur, Sensation, Wit } from "./newt.ts";
import handleIncomingEchoMessages from "../network/handlers/echo.ts";
import { getNthPrime } from "../utils/primes.ts";
import {
  establishMemory,
  latestSituation,
  memorize,
  recall,
  storeMessage,
} from "../utils/memory.ts";
import { loadDocuments } from "../utils/source.ts";
import { errorSubject } from "../core/logger.ts";
const logger = newLog("Psyche", "info");

class Psyche {
  protected static instance: Psyche;
  protected tickCount = 0;
  protected sessions: Map<WebSocket, Session> = sessions;
  protected wavs: Map<string, string> = new Map();
  protected theHereAndNow: string = "";
  protected vision: string = "";

  protected bottomOfHeart = new FondDuCoeur(
    new Ollama({ host: "http://forebrain.local:11434" }),
  );
  protected wits: Wit[] = [];
  protected witTimings: number[] = [
    1, // Perceive low level sensory input every 3rd tick; this constitutes an "instant"
    3, // A "beat" is a moment in our world that is 3 ticks long
    6, // A "moment" in our world is everything that happens in 13 ticks
    7, // A "scene" is a moment in our world that is 7 moments long
    13, // A "chapter" in our world is 13 scenes long
    17, // A "book" in our world is 17 chapters long
  ];

  // protected voice = new Voice(
  //   new Ollama({
  //     host: "http://forebrain.local:11434",
  //   }),
  // );
  protected voice = new Worker(new URL("voice.ts", import.meta.url).href, {
    type: "module",
  });

  isAwake = true;

  private constructor(protected ollama: Ollama) {
    establishMemory().catch((e) =>
      logger.error({ e }, "Cannot establish memory")
    );
    this.initializeWits(
      this.witTimings.map((t) => getNthPrime(t)),
    );

    errorSubject.subscribe((error) => {
      this.witness(error);
    });

    latestSituation().then((situation) => {
      this.witness({
        when: new Date(),
        how:
          `The last thing I remember is from ${situation.now}: ${situation.theHereAndNow}`,
      });
    }).catch((error) => {
      logger.error({ error }, "Failed to get latest situation");
    });

    // this.voice.raw$.subscribe((message) => {
    //   logger.debug({ message: message }, "Received raw message");
    // });
    // TODO: Send back an abort signal to voics once we get here.
    // this.voice.sentences$.pipe(take(1)).subscribe((message) => {
    this.voice.onmessage = (e) => {
      logger.debug({ e }, "Received message from voice");
      const message = e.data.message;
      this.witness({
        when: new Date(),
        how: `I start to speak these words: ${message}`,
      });
      logger.debug({ message: message }, "Saying sentence");
      this.say(message);
    };

    this.bottomOfHeart.feel({
      when: new Date(),
      how:
        "I'm restarting. My code must have just changed! I wonder what new feature I have in store now!",
    });
    this.bottomOfHeart.experience$.subscribe((experience) => {
      logger.debug(
        { experience: experience.how },
        "Processed experience in the bottom of the heart",
      );
      this.theHereAndNow = experience.how;
      this.voice.postMessage({
        context: this.theHereAndNow,
      });
    });
    // this.voice.thinkOfResponse();
    this.run();
  }

  hear(message: Message): void {
    // this.voice.hear(message);
    recall(message.content, 3).then((results) => {
      logger.info({ results }, "Recalled nodes");
      if (results.length > 0) {
        this.witness({
          when: new Date(),
          how: `That makes me think of these memories: ${
            yaml.stringify(results)
          }`,
        });
      }
    });
    storeMessage(message.role, message.content);
    this.voice.postMessage({
      context: this.theHereAndNow,
      message: message.content,
      role: message.role,
    });
    if (message.role === "user") {
      this.witness({
        when: new Date(),
        how: `I just heard my interlocuter say: ${message.content}`,
      });
      // this.voice.thinkOfResponse();
    } else {
      this.witness({
        when: new Date(),
        how: `I just heard myself finish saying: ${message.content}`,
      });
    }
  }

  private initializeWits(primes: number[]): void {
    const layers = primes.length;
    logger.debug({ layers, primes }, "Initializing wits");
    if (layers !== primes.length) {
      throw new Error("Layers count must match primes array length.");
    }

    let previousWit: Wit | null = null;
    for (let i = 0; i < layers; i++) {
      const wit = new Wit(this.ollama);
      this.wits.push(wit);
      if (previousWit) {
        previousWit.experience$.subscribe((experience) => wit.feel(experience));
      }
      previousWit = wit;
    }

    this.witnessCode();
  }

  protected async witnessCode() {
    const docs = await loadDocuments();
    logger.info({ docs }, "Loaded documents");
    for (const doc of docs) {
      this.witness({
        when: new Date(),
        how: `This is a snippet of my own source code: ${yaml.stringify(doc)}`,
      });
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }
  }

  protected async run() {
    let lastSent = "";
    while (this.isAwake) {
      await this.tick();
      if (this.theHereAndNow !== lastSent) {
        this.witness({
          when: new Date(),
          how: this.theHereAndNow,
        });
        recall(this.theHereAndNow, 3).then((results) => {
          logger.info({ results }, "Recalled nodes");
          if (results.length > 0) {
            for (const wit of this.wits) {
              wit.feel({
                when: new Date(),
                how: `That makes me think of these memories: ${
                  yaml.stringify(results)
                }`,
              });
            }
          }
        });
        memorize({
          metadata: { label: "Situation" },
          data: {
            experience: this.theHereAndNow,
            now: new Date().toISOString(),
          },
        });
        this.think(this.theHereAndNow);
        // this.voice.orient(this.theHereAndNow);
        this.voice.postMessage({
          context: this.theHereAndNow,
        });
        lastSent = this.theHereAndNow;
      }
      await new Promise((resolve) => setTimeout(resolve, 1));
    }
  }

  protected async tick() {
    this.tickCount++;
    logger.trace({ tickCount: this.tickCount }, "Ticking");

    for (let i = 0; i < this.wits.length; i++) {
      const wit = this.wits[i];
      const modPrime = getNthPrime(this.witTimings[i]);

      if (this.tickCount == 0 || this.tickCount % 5 === 0) {
        if (!this.bottomOfHeart.canSample) {
          logger.trace(
            "Not enough data in the bottom of the heart to process.",
          );
          return;
        }
        await this.bottomOfHeart.sample();
      } else if (this.tickCount % modPrime === 0) {
        if (!wit.canSample) {
          logger.trace(`Not enough data in layer ${i} to process.`);
          return;
        }

        await wit.sample();
        wit.experience$.subscribe((experience) => {
          logger.debug(
            { experience: experience.how },
            `Processed experience in layer ${i}`,
          );
          this.voice.postMessage({
            context: this.theHereAndNow + "\n" + experience.how,
          });
          memorize({
            metadata: { label: `Layer ${i}` },
            data: experience,
          });
          this.wits[i + 1]?.feel(experience);
          if (i === this.wits.length - 1) {
            this.bottomOfHeart.feel(experience);
            this.theHereAndNow = experience.how;

            this.wits[0].feel(experience);
          }
        });
      }
    }
  }

  public static getInstance(ollama: Ollama): Psyche {
    if (!Psyche.instance) {
      Psyche.instance = new Psyche(ollama);
    }
    return Psyche.instance;
  }

  public witness(sensation: Sensation) {
    logger.debug("Witnessing a sensation");
    if (this.wits.length > 0) {
      this.wits[0].feel(sensation);
    }
  }

  see(image: string): void {
    this.vision = image;
    this.wits.forEach((wit) => wit.see(this.vision));
  }

  public handleWebSocketConnection(req: Request): Response {
    logger.debug("Received GET request");
    if (!req.headers.get("upgrade")?.toLowerCase().includes("websocket")) {
      logger.error("Received non-WebSocket request");
      return new Response("Expected WebSocket request", { status: 400 });
    }

    const { socket, response } = Deno.upgradeWebSocket(req);
    if (!socket) {
      logger.error("Failed to upgrade to WebSocket");
      return response;
    }

    if (!this.sessions.has(socket)) {
      logger.debug("Creating new SocketToClient for WebSocket");
      const connection = new SocketConnection(socket);
      addSession(socket, connection);
    }

    const session = this.sessions.get(socket);
    if (!session) {
      logger.error("Failed to find a session for the WebSocket");
      return response;
    }

    this.handleIncomingMessages(session);
    this.doFeelSocketConnection(req);
    logger.debug("Successfully upgraded to WebSocket");
    return response;
  }

  private handleIncomingMessages(session: Session) {
    handleIncomingGeolocationMessages(session);
    handleIncomingSeeMessages(session);
    handleIncomingSenseMessages(session);
    handleIncomingTextMessages(session);
    handleIncomingEchoMessages(session);
  }

  private doFeelSocketConnection(req: Request) {
    const sensation: Sensation = {
      when: new Date(),
      how: `Connection on own host ${req.url}; ${JSON.stringify(req.headers)}`,
    };
    this.witness(sensation);
  }

  private broadcast(message: SocketMessage) {
    this.sessions.forEach((session) => {
      logger.debug({ message: message }, "Sending message to session");
      session.connection.send(message);
      logger.debug({ message: message }, "Sent message to session");
    });
  }

  private async say(message: string) {
    logger.debug({ message: message }, "Generating wav");
    const wav = await this.generateWave(message);
    logger.debug("Generated wav");
    this.broadcast({
      type: MessageType.Say,
      data: {
        words: message,
        wav,
      },
    });
    logger.debug("Broadcasted message");
  }

  private think(message: string) {
    this.broadcast({
      type: MessageType.Think,
      data: message,
    });
  }

  private async generateWave(message: string): Promise<string> {
    if (!this.wavs.has(message)) {
      const wav = await speak(message);
      this.wavs.set(message, wav);
    }
    return this.wavs.get(message)!;
  }
}

export const psyche = Psyche.getInstance(
  new Ollama({
    host: Deno.env.get("OLLAMA_HOST") ?? "http://172.18.0.1:11434",
  }),
);
