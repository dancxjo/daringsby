import { Handlers } from "$fresh/server.ts";
import { SocketConnection } from "../lib/daringsby/network/sockets/connection.ts";
import {
  addSession,
  Session,
  sessions,
} from "../lib/daringsby/network/Sessions.ts";
import { logger } from "../logger.ts";
import { isValidSeeMessage } from "../lib/daringsby/network/messages/SeeMessage.ts";
import { Image } from "../lib/daringsby/vision/describer.ts";
import { ImageDescriber } from "../lib/daringsby/vision/describer.ts";
import { MessageType } from "../lib/daringsby/network/messages/MessageType.ts";
import { isValidSenseMessage } from "../lib/daringsby/network/messages/SenseMessage.ts";
import { isValidTextMessage } from "../lib/daringsby/network/messages/TextMessage.ts";
import { Witness } from "./Witness.ts";

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
      addSession(socket, connection);
    }

    const session = sessions.get(socket);
    if (!session) {
      logger.error("Failed to find a session for the WebSocket");
      return response;
    }

    handleIncomingSeeMessages(session);
    handleIncomingSenseMessages(session);
    handleIncomingTextMessages(session);

    logger.debug("Successfully upgraded to WebSocket");
    return response;
  },
};

const witness = new Witness();

function tick() {
  setTimeout(async () => {
    const impression = await witness.feel({
      when: new Date(),
      what: [
        {
          how: `This is my internal chronometer. It is currently ${
            new Date().toLocaleTimeString()
          }. I am alive.`,
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
    });
    witness.enqueue(impression);
    tick();
  }, 1000);
}

tick();

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
        witness.enqueue(impression);
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
        witness.enqueue({
          ...message.data,
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
          how: `I heard: ${message.data}`,
          what: {
            when: new Date(),
            what: message.data,
          },
        };
        witness.enqueue(impression);
        return impression;
      },
    ),
  );
}
