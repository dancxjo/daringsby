import { Handlers } from "$fresh/server.ts";
import { Message, Ollama } from "npm:ollama";
import { isValidEchoMessage } from "../lib/daringsby/messages/EchoMessage.ts";
import { isValidGeolocateMessage } from "../lib/daringsby/messages/GeolocateMessage.ts";
import { isValidMienMessage } from "../lib/daringsby/messages/MienMessage.ts";
import {
    isValidSeeMessage,
    stamp,
} from "../lib/daringsby/messages/SeeMessage.ts";
import { SocketConnection } from "../lib/daringsby/messages/SocketConnection.ts";
import {
    isValidTextMessage,
    TextMessage,
} from "../lib/daringsby/messages/TextMessage.ts";
import { isValidThoughtMessage } from "../lib/daringsby/messages/ThoughtMessage.ts";
import { OllamaClient } from "../lib/daringsby/providers/ollama/Client.ts";
import { OllamaProcessor } from "../lib/daringsby/providers/ollama/Processor.ts";
import { describe, internalize } from "../lib/daringsby/senses/vision.ts";
import { logger } from "../logger.ts";
import {
    BehaviorSubject,
    from,
    map,
    merge,
    mergeMap,
    Observable,
    OperatorFunction,
    Subject,
    Subscription,
    switchMap,
    takeUntil,
    tap,
    toArray,
    windowTime,
} from "npm:rxjs";
import {
    integrate,
    Sensation,
    Stamped,
} from "../lib/daringsby/senses/sense.ts";
import { MessageType } from "../lib/daringsby/messages/MessageType.ts";
import {
    Balancer,
    ModelCharacteristic,
} from "../lib/daringsby/providers/Balancer.ts";
import { ChatTask, Method } from "../lib/daringsby/tasks.ts";
import {
    stringify,
    toSentences,
    wholeResponse,
} from "../lib/daringsby/chunking.ts";
import { Processor } from "../lib/daringsby/processors.ts";
import { chitChat } from "../lib/daringsby/chat.ts";
import { speak } from "../lib/daringsby/audio_processing.ts";

interface Session {
    connection: SocketConnection;
    conversation: BehaviorSubject<Message[]>;
    subscriptions: Subscription[];
}

const sessions = new Map<WebSocket, Session>();
const mainProcessor = new OllamaProcessor(
    new OllamaClient(
        "main",
        new Ollama({
            host: Deno.env.get("OLLAMA_URL") || "http://localhost:11434",
        }),
    ),
);

const backupProcessor = new OllamaProcessor(
    new OllamaClient(
        "backup",
        new Ollama({
            host: Deno.env.get("OLLAMA2_URL") || "http://localhost:11434",
        }),
    ),
);

const localProcessor = new OllamaProcessor(
    new OllamaClient(
        "local",
        new Ollama(),
    ),
);

const balancer = new Balancer([mainProcessor, backupProcessor, localProcessor]);

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
        const { connection, conversation } = session;

        // Senses
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

        const incomingTextMessages$: Observable<TextMessage> = connection
            .incoming(
                isValidTextMessage,
            );

        const incomingText$ = incomingTextMessages$.pipe(
            map((message) => ({
                content: `I hear my interlocutor say: ${message.data}`,
                at: new Date(message.at ?? new Date()),
            } as Sensation)),
            tap((message) => {
                logger.debug({ message }, "Received text");
            }),
        );

        connection.incoming(isValidMienMessage).pipe(map((message) => {
            logger.debug({ message }, "Received mien");
        }));

        connection.incoming(isValidThoughtMessage).pipe(map((message) => {
            logger.debug({ message }, "Received thought");
        }));

        const context$ = new BehaviorSubject(
            "You are an artificial consciousness named Pete Daringsby.",
        );

        const vision$ = connection.incoming(isValidSeeMessage).pipe(
            map(stamp),
            describe(balancer, context$),
            internalize(balancer),
        );

        const heartbeat$ = new Subject<Sensation>();

        setInterval(() => {
            heartbeat$.next({
                content: "I feel my heart beat",
                at: new Date(),
            });
        }, Math.random() * 1000);
        const newSensation$ = new Subject<Sensation>();
        const sensations$ = merge(
            heartbeat$,
            vision$,
            incomingText$,
            geolocation$,
            newSensation$,
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

        // Conversation
        const allTheThingsISaid = connection.incoming(isValidEchoMessage)
            .subscribe(
                (echoMessage) => {
                    logger.debug({ echoMessage }, "Received echo");
                    const messages: Message[] = [...conversation.value, {
                        role: "assistant",
                        content: echoMessage.data,
                    }];
                    conversation.next(messages);
                },
            );
        session.subscriptions.push(allTheThingsISaid);

        session.subscriptions.push(
            incomingTextMessages$.subscribe((message) => {
                conversation.next([...conversation.value, {
                    role: "user",
                    content: message.data,
                }]);
            }),
        );

        const everythingIPlanToSay = conversation.pipe(
            chitChat(balancer, context$),
        );

        session.subscriptions.push(
            everythingIPlanToSay.subscribe(async (intentionToSay) => {
                logger.debug({ intentionToSay }, "Intention to say");
                newSensation$.next({
                    at: new Date(),
                    content:
                        `I'm starting to say this: ${intentionToSay.content}`,
                });
                const wav = await speak(intentionToSay.content);
                connection.send({
                    type: MessageType.Say,
                    data: { words: intentionToSay.content, wav },
                });
            }),
        );

        // TODO: Clean up subscriptions

        logger.debug("Successfully upgraded to WebSocket");

        return response;
    },
};
