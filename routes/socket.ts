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
import { Experience } from "../lib/daringsby/core/interfaces.ts";
import { Voice } from "../lib/daringsby/core/voice.ts";

export const handler: Handlers = {
  GET(req, _ctx) {
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
      const voice = new Voice("", connection, baseWitness);
      addSession(socket, connection, voice);
    }

    const session = sessions.get(socket);
    if (!session) {
      logger.error("Failed to find a session for the WebSocket");
      return response;
    }

    handleIncomingSeeMessages(session);
    handleIncomingSenseMessages(session);
    handleIncomingTextMessages(session);
    handleIncomingGeolocationMessages(session);

    logger.debug("Successfully upgraded to WebSocket");
    return response;
  },
};

const baseWitness = new Wit();
const witnesses = [baseWitness];
const contextualizer = new Contextualizer();
let recentExperiences: Experience[] = [];

// Create a chain of witnesses, each feeding into the next one
for (let i = 1; i < 5; i++) {
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
          how: `It is currently ${new Date().toLocaleTimeString()}.`,
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
  const eye = new ImageDescriber();

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
