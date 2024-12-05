import { Handlers } from "$fresh/server.ts";
import { SocketConnection } from "../lib/daringsby/network/sockets/connection.ts";
import {
  addSession,
  Session,
  sessions,
} from "../lib/daringsby/network/Sessions.ts";
import { logger } from "../lib/daringsby/core/logger.ts";
import { isValidSeeMessage } from "../lib/daringsby/network/messages/SeeMessage.ts";
import { Image } from "../lib/daringsby/vision/describer.ts";
import { ImageDescriber } from "../lib/daringsby/vision/describer.ts";
import { MessageType } from "../lib/daringsby/network/messages/MessageType.ts";
import { isValidSenseMessage } from "../lib/daringsby/network/messages/SenseMessage.ts";
import { isValidTextMessage } from "../lib/daringsby/network/messages/TextMessage.ts";
import { isValidGeolocateMessage } from "../lib/daringsby/network/messages/GeolocateMessage.ts";
import { Wit } from "../lib/daringsby/core/wit.ts";
import { Contextualizer } from "../lib/daringsby/core/contextualizer.ts";
import { Experience, Impression } from "../lib/daringsby/core/interfaces.ts";
import { Voice } from "../lib/daringsby/core/voice.ts";
import neo4j from "npm:neo4j-driver";

const eye = new ImageDescriber();
const baseWitness = new Wit();
const witnesses = [baseWitness];
const contextualizer = new Contextualizer();
let recentExperiences: Experience[] = [];

export const handler: Handlers = {
  async GET(req, _ctx) {
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

    if (!sessions.has(socket)) {
      logger.debug("Creating new SocketToClient for WebSocket");
      const connection = new SocketConnection(socket);
      const context = await getLastContext();
      eye.context = context;
      const voice = new Voice(context, connection, baseWitness);
      addSession(socket, connection, voice);
    }

    const session = sessions.get(socket);
    if (!session) {
      logger.error("Failed to find a session for the WebSocket");
      return response;
    }

    doFeelSocketConnection(session, req);
    handleIncomingSeeMessages(session);
    handleIncomingSenseMessages(session);
    handleIncomingTextMessages(session);
    handleIncomingGeolocationMessages(session);

    logger.debug("Successfully upgraded to WebSocket");
    return response;
  },
};

// Create a chain of witnesses, each feeding into the next one
for (let i = 1; i < 3; i++) {
  const newWitness = new Wit();
  witnesses[i - 1].setNext(newWitness);
  witnesses.push(newWitness);
}

// Set the last witness to feed back into the base witness
witnesses[witnesses.length - 1].setNext(baseWitness);

function tick() {
  const interval = 1000;

  const tickWithDelay = async (witnessIndex: number) => {
    if (witnessIndex >= witnesses.length) {
      tick(); // Restart the loop once all witnesses have processed
      return;
    }

    const witness = witnesses[witnessIndex];

    if (witness.impressions?.length < 3) { // Defer processing if there are not enough sensations queued
      setTimeout(
        () => tickWithDelay(witnessIndex + 1),
        interval / witnesses.length,
      );
      return;
    }

    const impression = await witness.feel({
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

    sessions.forEach((session) => {
      session.connection.send({
        type: MessageType.Think,
        data: impression.how,
      });
      if (session.voice) {
        session.voice.context = impression.how;
      }
      eye.context = impression.how;
    });

    witness.next?.enqueue(impression);
    recentExperiences = [impression, ...recentExperiences].slice(0, 10);

    setTimeout(
      () => tickWithDelay(witnessIndex + 1),
      interval / witnesses.length,
    );
  };

  setTimeout(() => tickWithDelay(0), interval);
}

tick();

function tock() {
  setTimeout(async () => {
    await contextualizer.feel({
      when: new Date(),
      what: recentExperiences,
    });
    const willHaveContext = contextualizer.getContext();
    willHaveContext.then((context) => {
      if (context.match(/^Error/)) {
        return;
      }
      witnesses.forEach((witness) =>
        witness.enqueue({
          how: `Possibly relevant memories: ${context}`,
          depth_low: 1,
          depth_high: 1,
          what: {
            when: new Date(),
            what: context,
          },
        })
      );
    });
  }, 1000);
}

tock();
function handleIncomingSeeMessages(session: Session) {
  session.subscriptions.push(
    session.connection.incoming(isValidSeeMessage).subscribe(
      async (message) => {
        logger.debug("Received a valid SeeMessage");
        const image: Image = { base64: message.data };
        const impression = await eye.feel({
          when: new Date(message.at),
          what: image,
        });
        baseWitness.enqueue(impression);
        return impression;
      },
    ),
  );
}

function handleIncomingSenseMessages(session: Session) {
  session.subscriptions.push(
    session.connection.incoming(isValidSenseMessage).subscribe(
      async (message) => {
        logger.debug({ data: message.data }, "Received a valid SenseMessage");
        baseWitness.enqueue({
          how: `I sense: ${message.data.how}`,
          depth_low: message.data.depth_low,
          depth_high: message.data.depth_high,
          what: {
            ...message.data.what,
            when: new Date(message.data.what.when),
          },
        });
      },
    ),
  );
}

function handleIncomingTextMessages(session: Session) {
  session.subscriptions.push(
    session.connection.incoming(isValidTextMessage).subscribe(
      async (message) => {
        logger.debug({ data: message.data }, "Received a TextMessage");
        const impression = {
          how: `I just heard someone say to me: ${message.data}`,
          depth_low: 0,
          depth_high: 0,
          what: {
            when: new Date(),
            what: message.data,
          },
        };
        baseWitness.enqueue(impression);
        return impression;
      },
    ),
  );
}

function handleIncomingGeolocationMessages(session: Session) {
  session.subscriptions.push(
    session.connection.incoming(isValidGeolocateMessage).subscribe(
      async (message) => {
        logger.debug({ data: message.data }, "Received a GeolocationMessage");
        const impression = {
          how:
            `I am geolocated at ${message.data.latitude}, ${message.data.longitude}`,
          depth_low: 0,
          depth_high: 0,
          what: {
            when: new Date(),
            what: message.data,
          },
        };
        baseWitness.enqueue(impression);
        return impression;
      },
    ),
  );
}

function doFeelSocketConnection(session: Session, req: Request) {
  // Create a message with lots of useful information about the request and our context
  const messageToWitness = `I am connected to ${req.url} via WebSocket. ${
    JSON.stringify({ ...req.headers })
  }`;
  const sensation: Impression<Request> = {
    how: messageToWitness,
    depth_low: 0,
    depth_high: 0,
    what: {
      when: new Date(),
      what: req,
    },
  };
  baseWitness.enqueue(sensation);
}

async function getLastContext() {
  // Open the neo4j db and fetch the latest experience and return its content
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
  return result.records[0].get(0).properties.what;
}
