import { Handlers } from "$fresh/server.ts";
import { Ollama } from "npm:ollama";
import { isValidEchoMessage } from "../lib/daringsby/messages/EchoMessage.ts";
import { isValidGeolocateMessage } from "../lib/daringsby/messages/GeolocateMessage.ts";
import { isValidMienMessage } from "../lib/daringsby/messages/MienMessage.ts";
import {
    isValidSeeMessage,
    stamp,
} from "../lib/daringsby/messages/SeeMessage.ts";
import { SocketConnection } from "../lib/daringsby/messages/SocketConnection.ts";
import { isValidTextMessage } from "../lib/daringsby/messages/TextMessage.ts";
import { isValidThoughtMessage } from "../lib/daringsby/messages/ThoughtMessage.ts";
import { OllamaClient } from "../lib/daringsby/providers/ollama/Client.ts";
import { OllamaProcessor } from "../lib/daringsby/providers/ollama/Processor.ts";
import { describe, internalize } from "../lib/daringsby/senses/vision.ts";
import { logger } from "../logger.ts";
import {
    from,
    map,
    merge,
    mergeMap,
    Observable,
    Subject,
    tap,
    toArray,
    windowTime,
} from "npm:rxjs";
import { integrate, Sensation } from "../lib/daringsby/senses/sense.ts";
import { MessageType } from "../lib/daringsby/messages/MessageType.ts";

interface Session {
    connection: SocketConnection;
}

const sessions = new Map<WebSocket, Session>();
const mainProcessor = new OllamaProcessor(
    new OllamaClient(
        new Ollama({
            host: Deno.env.get("OLLAMA_URL") || "http://localhost:11434",
        }),
    ),
);

const backupProcessor = new OllamaProcessor(
    new OllamaClient(
        new Ollama({
            host: Deno.env.get("OLLAMA2_URL") || "http://localhost:11434",
        }),
    ),
);

const localProcessor = new OllamaProcessor(
    new OllamaClient(
        new Ollama(),
    ),
);

export const handler: Handlers = {
    GET(req, _ctx) {
        logger.debug("Received GET request");
        if (!req.headers.get("upgrade")?.toLowerCase().includes("websocket")) {
            logger.error("Received non-WebSocket request");
            return new Response("Expected WebSocket request", {
                status: 400,
            });
        }
        logger.debug("Received GET request to upgrade to WebSocket");
        const { socket, response } = Deno.upgradeWebSocket(req);

        logger.debug("Upgrading to WebSocket");
        if (!socket) {
            logger.error("Failed to upgrade to WebSocket");
            return response;
        }

        if (!sessions.has(socket)) {
            logger.debug("Creating new SocketToClient for WebSocket");
            const connection = new SocketConnection(socket);
            const session = {
                connection,
            };
            sessions.set(socket, session);
        }
        logger.debug("Found existing SocketToClient for WebSocket");

        const session = sessions.get(socket);
        if (!session) {
            logger.error("Failed to find a session for the WebSocket");
            return response;
        }
        const { connection } = session;

        const geolocation$ = connection.incoming(isValidGeolocateMessage).pipe(
            map((message) => {
                logger.debug({ message }, "Received geolocation");
                return {
                    at: new Date(message.at ?? new Date()),
                    content:
                        `I am at ${message.data.latitude}, ${message.data.longitude}`,
                };
            }),
        );

        connection.incoming(isValidEchoMessage).pipe(map((message) => {
            logger.debug({ message }, "Received echo");
        }));

        connection.incoming(isValidMienMessage).pipe(map((message) => {
            logger.debug({ message }, "Received mien");
        }));

        connection.incoming(isValidThoughtMessage).pipe(map((message) => {
            logger.debug({ message }, "Received thought");
        }));

        const context$ = from(
            "You are an artificial consciousness named Pete Daringsby.",
        );

        const vision$ = connection.incoming(isValidSeeMessage).pipe(
            map(stamp),
            describe(mainProcessor, context$),
            internalize(backupProcessor),
        );

        const incomingText$: Observable<Sensation> = connection.incoming(
            isValidTextMessage,
        ).pipe(
            map((message) => ({
                content: `I hear my interlocutor say: ${message.data}`,
                at: new Date(message.at ?? new Date()),
            } as Sensation)),
            tap((message) => {
                logger.debug({ message }, "Received text");
            }),
        );

        const heartbeat$ = new Subject<Sensation>();

        setInterval(() => {
            heartbeat$.next({
                content: "I feel my heart beat",
                at: new Date(),
            });
        }, Math.random() * 1000);

        const sensations$ = merge(
            heartbeat$,
            vision$,
            incomingText$,
            geolocation$,
        );

        const instants$ = sensations$.pipe(
            windowTime(15000),
            mergeMap((window$) =>
                window$.pipe(
                    toArray(),
                    tap((sensations) => {
                        logger.debug(
                            `Received ${sensations.length} sensations`,
                        );
                    }),
                )
            ),
            integrate(backupProcessor),
        );

        const moments$ = instants$.pipe(
            windowTime(60000),
            mergeMap((window$) =>
                window$.pipe(
                    toArray(),
                    tap((instants) => {
                        logger.debug(
                            `Received ${instants.length} instants`,
                        );
                    }),
                )
            ),
            integrate(localProcessor),
        );

        const frames$ = moments$.pipe(
            windowTime(300000),
            mergeMap((window$) =>
                window$.pipe(
                    toArray(),
                    tap((moments) => {
                        logger.debug(
                            `Received ${moments.length} moments`,
                        );
                    }),
                )
            ),
            integrate(localProcessor),
        );

        merge(instants$, moments$, frames$).subscribe((moment) => {
            logger.debug({ moment }, "Latest moment");
            connection.send({
                type: MessageType.Think,
                data: moment.content,
            });
        });

        // TODO: Clean up subscriptions

        logger.debug("Successfully upgraded to WebSocket");

        return response;
    },
};
