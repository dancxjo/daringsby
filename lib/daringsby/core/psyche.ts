import { Observable, ReplaySubject } from "npm:rxjs";
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
import { Sensation, Voice, Wit } from "./newt.ts";
import handleIncomingEchoMessages from "../network/handlers/echo.ts";

const logger = newLog("Psyche", "debug");

class Psyche {
  protected static instance: Psyche;
  protected tickCount = 0;
  protected sessions: Map<WebSocket, Session> = sessions;
  protected wavs: Map<string, string> = new Map();
  protected theHereAndNow: string = "";
  protected vision: string = "";

  protected wits: Wit[] = [];
  protected voice = new Voice(
    new Ollama({
      host: "http://forebrain.local:11434",
    }),
  );
  isAwake = true;

  private constructor(protected ollama: Ollama) {
    this.initializeWits(5, [2, 7, 11, 17, 23]);
    // this.voice.raw$.subscribe((message) => {
    //   logger.info({ message: message }, "Received raw message");
    // });
    this.voice.sentences$.subscribe((message) => {
      this.witness({
        when: new Date(),
        how:
          `I feel the impulse to say the following message and I start speaking: ${message}`,
      });
      logger.info({ message: message }, "Saying sentence");
      this.say(message);
    });
    this.run();
  }

  hear(message: Message): void {
    this.voice.hear(message);
    if (message.role === "user") {
      this.witness({
        when: new Date(),
        how: `I just heard my interlocuter say: ${message.content}`,
      });
    } else {
      this.witness({
        when: new Date(),
        how: `I just heard myself finish saying: ${message.content}`,
      });
    }
  }

  private initializeWits(layers: number, primes: number[]): void {
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
  }

  protected async run() {
    let lastSent = "";
    while (this.isAwake) {
      await this.tick();
      if (this.theHereAndNow !== lastSent) {
        this.think(this.theHereAndNow);
        this.voice.orient(this.theHereAndNow);
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
      const modPrime = [2, 7, 11, 17, 23][i];

      if (this.tickCount % modPrime === 0) {
        if (!wit.canSample) {
          logger.trace(`Not enough data in layer ${i} to process.`);
          return;
        }

        await wit.sample();

        if (i === this.wits.length - 1) {
          wit.experience$.subscribe((experience) => {
            logger.info(
              { experience: experience.how },
              "Processed top-level experience",
            );
            this.theHereAndNow = experience.how;
          });
        }
      }
    }

    if (this.tickCount % 3 === 0) {
      await this.voice.thinkOfResponse();
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
      how: `Connection from ${req.url}`,
    };
    this.witness(sensation);
  }

  private broadcast(message: SocketMessage) {
    this.sessions.forEach((session) => {
      logger.info({ message: message }, "Sending message to session");
      session.connection.send(message);
      logger.info({ message: message }, "Sent message to session");
    });
  }

  private async say(message: string) {
    logger.info({ message: message }, "Generating wav");
    const wav = await this.generateWave(message);
    logger.info("Generated wav");
    this.broadcast({
      type: MessageType.Say,
      data: {
        words: message,
        wav,
      },
    });
    logger.info("Broadcasted message");
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
    host: Deno.env.get("OLLAMA_HOST") ?? "http://forebrain.local:11434",
  }),
);
