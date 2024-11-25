import { Handlers } from "$fresh/server.ts";
import { SocketConnection } from "../lib/daringsby/network/sockets/connection.ts";
import { addSession, sessions } from "../lib/daringsby/network/Sessions.ts";
import { logger } from "../lib/daringsby/core/logger.ts";

export const handler: Handlers = {
  GET(req, _ctx) {
    logger.info("Received GET request");
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
      logger.info("Creating new SocketToClient for WebSocket");
      const connection = new SocketConnection(socket);
      addSession(socket, connection);
    }

    const session = sessions.get(socket);
    if (!session) {
      logger.error("Failed to find a session for the WebSocket");
      return response;
    }

    logger.info("Successfully upgraded to WebSocket");
    return response;
  },
};
