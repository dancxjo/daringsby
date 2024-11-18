import { Handlers } from "$fresh/server.ts";
import { Message } from "npm:ollama";
import { isValidGeolocateMessage } from "../lib/daringsby/messages/GeolocateMessage.ts";
import { SocketConnection } from "../lib/daringsby/messages/SocketConnection.ts";
import {
    isValidTextMessage,
    TextMessage,
} from "../lib/daringsby/messages/TextMessage.ts";
import {
    EchoMessage,
    isValidEchoMessage,
} from "../lib/daringsby/messages/EchoMessage.ts";
import { logger } from "../logger.ts";
import { BehaviorSubject, map, Observable, Subscription, tap } from "npm:rxjs";
import { Heart, Session } from "../lib/daringsby/newts.ts";

const heart = new Heart();
const sessions = heart.sessions;

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

        // Manage sessions
        if (!sessions.has(socket)) {
            logger.debug("Creating new SocketToClient for WebSocket");
            const connection = new SocketConnection(socket);
            const session: Session = {
                connection,
                conversation: new BehaviorSubject<Message[]>([]),
                subscriptions: [],
            };
            sessions.set(socket, session);
        }
        logger.debug("Found existing SocketToClient for WebSocket");

        const session = sessions.get(socket);
        if (!session) {
            logger.error("Failed to find a session for the WebSocket");
            return response;
        }

        handleGeolocation(session);
        handleHearingVoices(session);
        handleEchoMessages(session);
        heart.consult().subscribe((sensations) => {
            logger.info({ sensations }, "Consulted heart");
        });

        logger.debug("Successfully upgraded to WebSocket");

        return response;
    },
};

function handleGeolocation(
    session: Session,
) {
    const geolocation$ = session.connection.incoming(isValidGeolocateMessage)
        .pipe(
            map((message) => {
                logger.debug({ message }, "Received geolocation");
                return {
                    when: new Date(message.at ?? new Date()),
                    content: {
                        explanation:
                            `I am at ${message.data.latitude}, ${message.data.longitude}`,
                        content: message.data,
                    },
                };
            }),
        );
    session.subscriptions.push(geolocation$.subscribe((sensation) => {
        // Send to the quick of the heart
        heart.quick.next({
            when: sensation.when,
            content: {
                explanation: sensation.content.explanation,
                content: JSON.stringify(sensation.content.content),
            },
        });
        logger.info({ sensation }, "Processed geolocation sensation");
    }));
}

function handleHearingVoices(
    session: Session,
) {
    const incomingTextMessages$: Observable<TextMessage> = session.connection
        .incoming(
            isValidTextMessage,
        );

    const incomingText$ = incomingTextMessages$.pipe(
        map((message) => ({
            when: new Date(message.at ?? new Date()),
            content: {
                explanation: `I hear my interlocutor say: ${message.data}`,
                content: message.data,
            },
        })),
        tap((sensation) => {
            logger.debug({ sensation }, "Received text sensation");
        }),
    );

    session.subscriptions.push(incomingText$.subscribe((sensation) => {
        // Add to conversation
        session.conversation.next([...session.conversation.value, {
            role: "interlocutor",
            content: sensation.content.content,
        }]);
        // Send to the quick of the heart
        heart.quick.next(sensation);
        logger.info({ sensation }, "Processed hearing sensation");
    }));
}

function handleEchoMessages(
    session: Session,
) {
    const incomingEchoMessages$: Observable<EchoMessage> = session.connection
        .incoming(
            isValidEchoMessage,
        );

    const incomingEcho$ = incomingEchoMessages$.pipe(
        map((message) => ({
            when: new Date(message.at ?? new Date()),
            content: {
                explanation: `I just heard myself say: ${message.data}`,
                content: message.data,
            },
        })),
        tap((sensation) => {
            logger.debug({ sensation }, "Received echo sensation");
        }),
    );

    session.subscriptions.push(incomingEcho$.subscribe((sensation) => {
        // Add to conversation
        session.conversation.next([...session.conversation.value, {
            role: "self",
            content: sensation.content.content,
        }]);
        // Send to the quick of the heart
        heart.quick.next(sensation);
        logger.info({ sensation }, "Processed echo sensation");
    }));
}
