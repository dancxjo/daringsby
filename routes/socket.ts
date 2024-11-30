import { Handlers } from "$fresh/server.ts";
import { SocketConnection } from "../lib/daringsby/network/sockets/connection.ts";
import { addSession, sessions } from "../lib/daringsby/network/Sessions.ts";
import { logger } from "../logger.ts";
import { isValidSeeMessage } from "../lib/daringsby/network/messages/SeeMessage.ts";
import { Image } from "../lib/daringsby/vision/describer.ts";
import { ImageDescriber } from "../lib/daringsby/vision/describer.ts";
import { MessageType } from "../lib/daringsby/network/messages/MessageType.ts";

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

    const eye = new ImageDescriber();

    session.subscriptions.push(
      session.connection.incoming(isValidSeeMessage).subscribe(
        async (message) => {
          logger.debug("Received a valid SeeMessage");
          const image: Image = { base64: message.data };
          const description = await eye.feel({
            when: new Date(message.at),
            what: image,
          });
          session.connection.send({
            type: MessageType.Think,
            data: description.how,
            at: new Date().toISOString(),
          });
          return description;
        },
      ),
    );

    logger.debug("Successfully upgraded to WebSocket");
    return response;
  },
};
