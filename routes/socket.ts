import { Handlers } from "$fresh/server.ts";
import * as cheerio from "npm:cheerio";
import { speak } from "../lib/daringsby/audio_processing.ts";
import { isValidEchoMessage } from "../lib/daringsby/messages/EchoMessage.ts";
import { isValidGeolocateMessage } from "../lib/daringsby/messages/GeolocateMessage.ts";
import { MessageType } from "../lib/daringsby/messages/MessageType.ts";
import { isValidSeeMessage } from "../lib/daringsby/messages/SeeMessage.ts";
import { SocketConnection } from "../lib/daringsby/messages/SocketConnection.ts";
import { isValidSocketMessage } from "../lib/daringsby/messages/SocketMessage.ts";
import { isValidTextMessage } from "../lib/daringsby/messages/TextMessage.ts";
import { logger } from "../logger.ts";

interface Session {
    connection: SocketConnection;
}

const sessions = new Map<WebSocket, Session>();

export const handler: Handlers = {
    async GET(req, _ctx) {
        logger.debug("Received GET request");
        if (!req.headers.get("upgrade")?.toLowerCase().includes("websocket")) {
            logger.error("Received non-WebSocket request");
            return new Response("Expected WebSocket request", {
                status: 400,
            });
        }
        logger.info("Received GET request to upgrade to WebSocket");
        const { socket, response } = Deno.upgradeWebSocket(req);

        logger.info("Upgrading to WebSocket");
        if (!socket) {
            logger.error("Failed to upgrade to WebSocket");
            return response;
        }

        if (!sessions.has(socket)) {
            logger.info("Creating new SocketToClient for WebSocket");
            const connection = new SocketConnection(socket);
            const session = {
                connection,
            };
            sessions.set(socket, session);
        }
        logger.info("Found existing SocketToClient for WebSocket");

        const session = sessions.get(socket);
        if (!session) {
            logger.error("Failed to find a session for the WebSocket");
            return response;
        }
        const { connection } = session;

        connection.incoming(isValidTextMessage).subscribe((message) => {
        });

        logger.info("Successfully upgraded to WebSocket");

        return response;
    },
};
