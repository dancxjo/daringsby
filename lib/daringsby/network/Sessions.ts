import { SocketConnection } from "./sockets/connection.ts";
import {
    BehaviorSubject,
    bufferTime,
    map,
    of,
    ReplaySubject,
    Subject,
    Subscription,
    tap,
} from "npm:rxjs";
import { Message } from "npm:ollama";
import { Sensation } from "../core/interfaces.ts";
import { logger } from "../core/logger.ts";
import { Wits } from "../genii/Wit.ts";
import {
    handleEchoes,
    handleGeolocations,
    handleIncomingTexts,
    setupHeartbeat,
} from "./sockets/handlers.ts";
import { Voice } from "../genii/Voice.ts";
import { MessageType } from "./messages/MessageType.ts";
import * as yaml from "npm:yaml";

import {
    establishMemory,
    memorize,
    queryMemory,
    recall,
} from "../utils/memory.ts";

export class Session {
    readonly integration = new Wits(
        "Sensory Integration",
        "This part of the mind receives low-level, fine-grained sensory input and uses it to produce a coherent understanding of the present instant. Somewhat terse, rather 'just the facts, ma'am.'",
    );
    readonly combobulation = new Wits(
        "Combobulation",
        "This part of the mind combines several instants together to produce a more coherent understanding of the present moment and current situation. The combobulator figures out what's up and what's what. The combobulator is very intuitive and makes good inferences based on the information it can access. It does not invent information, though. It sticks with the facts that it receives.",
    );
    protected latestInstants$: ReplaySubject<Sensation<string>> =
        new ReplaySubject<Sensation<string>>(1);
    protected latestSituation$: ReplaySubject<Sensation<string>> =
        new ReplaySubject<Sensation<string>>(1);
    readonly voice = new Voice("Voice", this.latestSituation$, this);

    protected instants: Sensation<string>[] = [];
    protected moments: Sensation<string>[] = [];
    protected context: string = `MERGE (n:Self) RETURN n`; // A Cypher query that represents the current situation
    protected contextValue: string = ""; // The current situation as a string

    constructor(
        readonly connection: SocketConnection,
        readonly subscriptions: Subscription[],
    ) {
        // establishMemory();
        handleGeolocations(this);
        handleEchoes(this);
        handleIncomingTexts(this);
        setupHeartbeat(this);

        this.tickWits();
        this.tock();
        setInterval(() => this.tock(), 15000);
        this.latestInstants$.pipe(
            bufferTime(10000, 5000),
            tap((latest) => {
                latest.forEach((latest) => {
                    this.instants.push(latest);
                    this.combobulation.feel(latest);
                });
                this.tickWits();
            }),
            bufferTime(60000, 30000),
        ).subscribe((latest) => {
            latest.forEach((latest) => {
                subscriptions.push(
                    this.combobulation.consult().subscribe((narration) => {
                        if (!narration) {
                            return;
                        }
                        this.tickWits();
                        latest.sort((a, b) =>
                            b.when.getTime() - a.when.getTime()
                        );
                        const asOf = new Date(latest[latest.length - 1]?.when);
                        const newSituation = {
                            when: asOf,
                            content: {
                                explanation:
                                    `The situation as of ${asOf.toLocaleString()} is as follows: ${narration}`,
                                content: JSON.stringify(narration),
                            },
                        };
                        this.latestSituation$.next(newSituation);
                        this.moments.push(newSituation);
                        this.combobulation.feel(newSituation);
                        this.connection.send({
                            type: MessageType.Think,
                            data: narration,
                        });
                        let when = new Date().toISOString();
                        try {
                            when = new Date(newSituation.when).toISOString();
                        } catch (error) {
                            logger.error(
                                { error },
                                "Failed to memorize situation",
                            );
                        }

                        memorize({
                            metadata: {
                                label: "Situation",
                                when,
                            },
                            data: {
                                explanation: newSituation.content.explanation,
                                content: newSituation.content.content,
                            },
                        });
                        recall(newSituation.content.explanation, 5)
                            .then((memories) => {
                                logger.debug({ memories }, "Recalled memories");
                                this.feel({
                                    when: new Date(),
                                    content: {
                                        explanation: `Recalled memories: ${
                                            JSON.stringify(memories)
                                        }`,
                                        content: JSON.stringify(memories),
                                    },
                                });
                            })
                            .catch((error) => {
                                logger.error(
                                    { error },
                                    "Failed to recall memories",
                                );
                            });
                    }),
                );
            });
        });
    }

    tock() {
        logger.debug({ context: this.context }, "Gathering instant");
        queryMemory(this.context).then((context) => {
            logger.debug({ context }, "Gathered instant");
            this.contextValue = JSON.stringify(context).replace(
                /"embedding":\s*\[\.*](,|$|\n)/gm,
                "",
            );
            const newSituation: Sensation<string> = {
                when: new Date(),
                content: {
                    explanation: `From your memory: \n` +
                        yaml.stringify(context),
                    content: yaml.stringify(context),
                },
            };

            logger.debug({ newSituation }, "New situation in YAML format");
            const glueSensation = { ...newSituation, embedding: undefined };
            logger.debug({ glueSensation }, "Gathered instant");
            this.integration.feel(glueSensation);
        });
    }

    feel(sensation: Sensation<unknown>) {
        logger.debug({ sensation }, "Feeling sensation");
        this.integration.feel(sensation);
    }

    tickWits() {
        const startedAt = new Date();
        this.subscriptions.push(
            this.integration.consult().subscribe((instant) => {
                logger.debug({ instant }, "Received latest instant");
                if (!instant) {
                    return;
                }
                const newInstant: Sensation<string> = {
                    when: startedAt,
                    content: {
                        explanation: `The current instant is: ${instant}`,
                        content: instant,
                    },
                };
                this.instants.push(newInstant);
                this.instants.sort((a, b) =>
                    b.when.getTime() - a.when.getTime()
                );
                this.instants.push(newInstant);
                this.combobulation.feel(newInstant);

                this.connection.send({
                    type: MessageType.Think,
                    data: instant,
                });
                this.latestInstants$.next(newInstant);
            }),
        );

        logger.debug("Thinking next thought");

        this.subscriptions.push(
            this.voice.consult().subscribe((narration) => {
                logger.debug({ narration }, "Received narration");
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
