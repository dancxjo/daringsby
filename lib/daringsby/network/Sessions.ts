import { SocketConnection } from "./sockets/connection.ts";
import {
    BehaviorSubject,
    map,
    of,
    ReplaySubject,
    Subject,
    Subscription,
} from "npm:rxjs";
import { Message } from "npm:ollama";
import { Sensation } from "../core/interfaces.ts";
import { logger } from "../../../logger.ts";
import { Wit } from "../genii/Wit.ts";
import {
    handleEchoes,
    handleGeolocations,
    handleIncomingTexts,
    setupHeartbeat,
} from "./sockets/handlers.ts";
import { Voice } from "../genii/Voice.ts";

export class Session {
    readonly integration = new Wit(
        "Sensory Integration",
        "This part of the mind receives low-level, fine-grained sensory input and uses it to produce a coherent understanding of the present instant",
    );
    protected latest$: ReplaySubject<string> = new ReplaySubject<string>(1);
    readonly voice = new Voice("Voice", this.latest$, this);

    protected instants: Sensation<string>[] = [];
    protected moments: Sensation<string>[] = [];
    protected context: string = `MERGE (me:Self) return me`; // A Cypher query that represents the current situation

    constructor(
        readonly connection: SocketConnection,
        readonly conversation: ReplaySubject<Message[]>,
        readonly subscriptions: Subscription[],
    ) {
        handleGeolocations(this);
        handleEchoes(this);
        handleIncomingTexts(this);
        setupHeartbeat(this);
        conversation.subscribe((messages) => {
            this.voice.feel({
                when: new Date(),
                content: {
                    explanation: JSON.stringify(messages),
                    content: "The conversation so far",
                },
            });
        });

        setInterval(() => {
            this.tickWits();
        }, 10000);
    }

    feel(sensation: Sensation<unknown>) {
        logger.debug({ sensation }, "Feeling sensation");
        this.integration.feel(sensation);
    }

    tickWits() {
        logger.info("Gathering instant");
        this.subscriptions.push(
            this.integration.consult().subscribe((instant) => {
                logger.info({ instant }, "Received latest instant");
                if (!instant) {
                    return;
                }
                this.instants.push({
                    when: new Date(),
                    content: {
                        explanation: instant,
                        content: instant,
                    },
                });
                this.latest$.next(instant);
            }),
        );

        logger.info("Thinking next thought");

        this.subscriptions.push(
            this.voice.consult().subscribe((narration) => {
                logger.info({ narration }, "Received narration");
            }),
        );
    }

    unsubscribe() {
        this.subscriptions.forEach((subscription) =>
            subscription.unsubscribe()
        );
    }
}

export const sessions = new Map<WebSocket, Session>();

export function addSession(
    socket: WebSocket,
    connection: SocketConnection,
): Session {
    const session = new Session(
        connection,
        new ReplaySubject<Message[]>(1),
        [],
    );

    sessions.set(socket, session);
    return session;
}

export function removeSession(socket: WebSocket) {
    const session = sessions.get(socket);
    if (session) {
        session.connection.hangup();
        session.unsubscribe();
        sessions.delete(socket);
    }
}
