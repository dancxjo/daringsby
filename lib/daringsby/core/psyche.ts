import { SocketConnection } from "../network/sockets/connection.ts";
import { addSession, Session, sessions } from "../network/Sessions.ts";
import { logger } from "../core/logger.ts";
import { Image, ImageDescriber } from "../vision/describer.ts";
import { Wit } from "../core/wit.ts";
import { Contextualizer } from "../core/contextualizer.ts";
import neo4j from "npm:neo4j-driver";
import { Experience, Impression, Sensation } from "./interfaces.ts";
import { MessageType } from "../network/messages/MessageType.ts";
import { speak } from "../utils/audio_processing.ts";
import handleIncomingGeolocationMessages from "../network/handlers/geolocation.ts";
import handleIncomingSeeMessages from "../network/handlers/images.ts";
import handleIncomingSenseMessages from "../network/handlers/sense.ts";
import { Voice } from "./voice.ts";
import { SocketMessage } from "../network/messages/SocketMessage.ts";

class Psyche {
  static maxWit = 3;
  see(sensation: Sensation<Image>) {
    return this.eye.feel(sensation);
  }
  private static instance: Psyche;

  public eye: ImageDescriber;
  public witnesses: Wit[];
  public contextualizer: Contextualizer;
  protected context: string = ""; // Relevant memories
  protected situation: string = ""; // Current situation
  public voice: Voice;
  public recentExperiences: Experience[];
  public sessions: Map<WebSocket, Session>;
  protected wavs: Map<string, string> = new Map();
  protected isConceptualizing: boolean = true;
  protected isConversing: boolean = true;

  private constructor() {
    this.eye = new ImageDescriber();
    this.witnesses = this.initializeWitnesses();
    this.contextualizer = new Contextualizer();
    this.voice = new Voice(
      "",
      (m) => this.broadcastMessage(m),
      (impression) => this.witness(impression),
    );
    this.recentExperiences = [];
    this.sessions = sessions; // Use the existing sessions map

    this.startFetchingContext();
    this.startTalking();
  }

  async startTalking() {
    while (this.isConversing) {
      await this.voice.offerChanceToAct();
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }
  }

  async startFetchingContext() {
    while (this.isConceptualizing) {
      const context = await this.contextualizer.getContext();
      if (context) {
        this.context = context;
        this.witnesses.forEach((wit) => {
          wit.enqueue({
            how: `These may be relevant memories: "${context}"`,
            depth_low: 0,
            depth_high: 0,
            what: {
              when: new Date(),
              what: context,
            },
          });
        });
      }
    }
  }

  // Singleton instance retrieval
  public static getInstance(): Psyche {
    if (!Psyche.instance) {
      Psyche.instance = new Psyche();
    }
    return Psyche.instance;
  }

  public witness(impression: Impression<unknown>) {
    this.witnesses[0].enqueue(impression);
  }

  private initializeWitnesses(): Wit[] {
    const wits = [];
    for (let i = 0; i < Psyche.maxWit; i++) {
      const newWit = new Wit();
      if (i > 0) {
        wits[i - 1].setNext(newWit);
      }

      wits.push(newWit);

      let isBusy = false;
      setInterval(async () => {
        if (isBusy) {
          return;
        }
        isBusy = true;
        const impression = await newWit.feel({
          when: new Date(),
          what: [
            {
              how: `It is currently ${new Date().toLocaleString()}/${
                new Date().toISOString()
              }.`,
              depth_low: 0,
              depth_high: 0,
              what: {
                when: new Date(),
                what: new Date().toLocaleTimeString(),
              },
            },
          ],
        });

        const isOnLastWit = i == Psyche.maxWit - 1;
        if (isOnLastWit) {
          this.situation = impression.how;
          this.eye.situation = impression.how;
          this.voice.situation = impression.how;
        } else {
          this.situation += " " + impression.how;
          this.eye.situation = this.situation;
          this.voice.situation = this.situation;
        }
        this.broadcastMessage({
          type: MessageType.Think,
          data: this.situation,
        });
        isBusy = false;
      }, 1000 + i * 2000);
    }
    return wits;
  }

  public async handleWebSocketConnection(req: Request): Promise<Response> {
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
      const context = await this.getLastContext();
      this.eye.situation = context;
      addSession(socket, connection);
    }

    const session = this.sessions.get(socket);
    if (!session) {
      logger.error("Failed to find a session for the WebSocket");
      return response;
    }

    this.doFeelSocketConnection(session, req);
    // Handle incoming messages
    this.handleIncomingMessages(session);

    logger.debug("Successfully upgraded to WebSocket");
    return response;
  }

  private handleIncomingMessages(session: Session) {
    handleIncomingGeolocationMessages(session);
    handleIncomingSeeMessages(session);
    handleIncomingSenseMessages(session);
  }

  private doFeelSocketConnection(session: Session, req: Request) {
    const messageToWitness =
      `I just felt someone connect to me at ${req.url} via WebSocket. ${
        JSON.stringify({ ...req.headers })
      }. I now can see through their webcam and hear them speaking to me. Anything I say will be spoken to them.`;
    const sensation: Impression<Request> = {
      how: messageToWitness,
      depth_low: 0,
      depth_high: 0,
      what: {
        when: new Date(),
        what: req,
      },
    };
    this.witnesses[0].enqueue(sensation);
  }

  private async getLastContext() {
    const driver = neo4j.driver(
      Deno.env.get("NEO4J_URL") || "bolt://localhost:7687",
      neo4j.auth.basic(
        Deno.env.get("NEO4J_USER") || "neo4j",
        Deno.env.get("NEO4J_PASSWORD") || "password",
      ),
    );
    const session = driver.session();
    const result = await session.run(
      "MATCH (e:Experience) RETURN e ORDER BY e.when DESC LIMIT 1",
    );
    session.close();
    driver.close();
    return result.records[0]?.get(0)?.properties?.what || "";
  }

  public broadcastMessage(message: SocketMessage) {
    this.sessions.forEach((session) => {
      session.connection.send(message);
    });
  }

  public broadcast(message: string) {
    this.generateWave(message).then((wav) => {
      this.broadcastMessage({
        type: MessageType.Say,
        data: {
          words: message,
          wav,
        },
      });
    });
  }

  async generateWave(message: string): Promise<string> {
    if (!this.wavs.has(message)) {
      const wav = await speak(message);
      this.wavs.set(message, wav);
    }
    const wav = this.wavs.get(message);
    if (!wav) {
      throw new Error("Failed to generate wave");
    }
    return wav;
  }
}

export const psyche = Psyche.getInstance();
