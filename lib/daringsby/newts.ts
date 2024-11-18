import {
    BehaviorSubject,
    catchError,
    combineLatest,
    debounceTime,
    filter,
    interval,
    map,
    mergeMap,
    Observable,
    of,
    ReplaySubject,
    Subject,
    Subscription,
    switchMap,
    tap,
    throttleTime,
    toArray,
} from "npm:rxjs";
import Handlebars from "npm:handlebars";
import { narrate as defaultNarration } from "./narration.ts";
import { pino } from "npm:pino";
import { SocketConnection } from "./messages/SocketConnection.ts";
import * as cheerio from "npm:cheerio";
import { speak } from "./audio_processing.ts";
import { MessageType } from "./messages/MessageType.ts";
import { Message } from "npm:ollama";
const logger = pino({ level: "info" });

export interface Session {
    connection: SocketConnection;
    conversation: BehaviorSubject<Message[]>;
    subscriptions: Subscription[];
}

export interface Stamped<I> {
    when: Date;
    content: I;
}

export interface Described<I> {
    explanation: string; // This is what the text-processing model receives
    content: I;
}

export type Sensation<I> = Stamped<Described<string>>;
export type Sensitive<I> = Subject<Sensation<I>>;

export interface Faculty<I = unknown, O = unknown> {
    quick?: Sensitive<I>;
    consult(): Observable<O>;
}

export class Genie<I> implements Faculty<I, string> {
    readonly quick: Sensitive<I> = new ReplaySubject<Sensation<I>>(1);
    protected sensations: Sensation<I>[] = [];

    constructor(
        protected name: string,
        protected description: string,
        protected instruction: string,
        protected narrate: (prompt: string) => Observable<string>,
    ) {
        logger.info(`Initializing Genie: ${name}`);
        this.initializeQuickSubscription();
        this.quick.subscribe((sensation) => {
            logger.info({ sensation }, `${name} received sensation`);
        });
        this.quick.next({
            when: new Date(),
            content: {
                explanation: `Initialized ${name}`,
                content: `Poke the quick to start the ${name}`,
            },
        });
    }

    protected initializeQuickSubscription() {
        logger.info(`${this.name}: Initializing quick subscription`);
        this.quick.pipe(
            tap((v) => logger.info({ v }, `${this.name} quick`)),
            throttleTime(500),
            toArray(),
            switchMap((sensations) => {
                logger.info(`${this.name}: Received sensations`);
                this.sensations = sensations;
                return this.consult();
            }),
        ).subscribe({
            next: (narration) =>
                logger.info({ narration }, `${this.name} narration`),
            error: (err) =>
                logger.error(err, `${this.name} encountered an error`),
        });
    }

    protected formatInput(
        input: Sensation<unknown>[],
    ): Record<string, unknown> {
        logger.info(`${this.name}: Formatting input`);
        return { sensations: input };
    }

    consult(): Observable<string> {
        logger.info(`${this.name}: Consulting`);
        const template = Handlebars.compile(
            `You are playing the role of {{name}}. {{description}} {{instruction}}`,
        );
        logger.info(`${this.name}: Compiling template`);
        const input = {
            name: this.name,
            description: this.description,
            instruction: this.instruction,
            sensations: this.formatInput(this.sensations),
        };

        // Adding introspective layer to enhance awareness
        const introspectionTemplate = Handlebars.compile(
            `Reflect on Pete's current state:
            {{#each sensations}}
            At {{when}}, Pete felt '{{explanation}}'.
            {{/each}}
            How do these sensations connect, and what do they tell us about Pete's emotional state?`,
        );
        const introspectivePrompt = introspectionTemplate(input);
        logger.info(
            { introspectivePrompt },
            `${this.name}: Introspective prompt generated`,
        );

        return this.narrate(introspectivePrompt).pipe(
            tap((response) =>
                logger.info(`${this.name}: LLM response received`, { response })
            ),
            catchError((err) => {
                logger.error(`${this.name}: Error invoking LLM`, err);
                return of("");
            }),
        );
    }
}

export class FondDuCoeur extends Genie<string> {
    protected subscriptionStack: Subscription[] = [];
    protected wits: Wit[] = [];

    constructor() {
        super(
            "Fond du Coeur",
            `Fond du Coeur is Pete's emotional core. It integrates all the Wits, serves as the seat of his feelings, and provides a coherent and accurate account of the emotional data.`,
            `Narrate the emotional state and experience of Pete.
{{#sensations}}At {{when}}, {{explanation}}.
{{/sensations}}
`,
            defaultNarration,
        );
        logger.info("Fond du Coeur: Initializing wits");
        this.initializeWits();
    }

    private initializeWits() {
        this.wits = [
            new Wit("The Living Daylights", "Wit of the Instant", this),
            new Wit("Fourth Nonblond", "Wit of the Moment", this),
            new Wit("Contextualizer", "Broader Context Integration", this),
        ];
    }

    override consult(): Observable<string> {
        logger.info("Fond du Coeur: Consulting");
        return combineLatest(this.wits.map((wit) => wit.consult())).pipe(
            map((narrations) => narrations.join("\n")),
            tap((combinedNarration) => {
                logger.info("Fond du Coeur: Combined narration received");
                this.quick.next({
                    when: new Date(),
                    content: {
                        explanation: combinedNarration,
                        content: combinedNarration,
                    },
                });
            }),
            switchMap(() => super.consult()),
        );
    }

    unshift(intervalMs: number): void {
        logger.info(`Fond du Coeur: Unshifting with interval ${intervalMs}ms`);
        this.subscriptionStack.push(
            interval(intervalMs).pipe(
                switchMap(() => this.consult()),
                switchMap((narration) => {
                    this.quick.next({
                        when: new Date(),
                        content: {
                            explanation: narration,
                            content: narration,
                        },
                    });
                    return super.consult();
                }),
            ).subscribe({
                next: (narration) =>
                    logger.info({ narration }, "Fond du Coeur narration"),
                error: (err) =>
                    logger.error(err, "Fond du Coeur encountered an error"),
            }),
        );
    }
}

export class Wit extends Genie<string> {
    constructor(
        name: string,
        description: string,
        protected fondDuCoeur: FondDuCoeur,
    ) {
        super(
            name,
            `${description}: Integrates input from Fond du Coeur and provides a refined perspective on Pete's experiences.`,
            `Narrate the sensory data and refine its meaning in the context of ${name}.
{{#sensations}}At {{when}}, {{explanation}}.
{{/sensations}}
`,
            defaultNarration,
        );
        logger.info(`Wit: ${name} initialized`);
        this.initializeQuickSubscription();
    }

    override consult(): Observable<string> {
        logger.info(`Wit: ${this.name} consulting`);
        return this.quick.pipe(
            toArray(),
            debounceTime(100),
            switchMap((sensations) => {
                logger.info(`Wit: ${this.name} received sensations`);
                this.sensations = sensations;
                return super.consult();
            }),
        );
    }
}

export class Heart extends Genie<string> {
    protected bottom = new FondDuCoeur();
    readonly sessions = new Map<WebSocket, Session>();

    constructor() {
        super(
            "Heart",
            `Pete's heart is the kernel of his psyche, integrating data from all his Wits and making sense of it in a central place. It synthesizes and commands actions based on the accumulated experiences.`,
            `Provide Pete's inward thoughts or initiate an action based on the accumulated input from all wits and Fond du Coeur.
{{#sensations}}At {{when}}, {{explanation}}.
{{/sensations}} To speak out loud, include brief text in <function name="say">...</function> tags. Do not send any asterisks or the TTS will read them out loud. Also, spell out all numbers, including dates, etc., and convert initialisms to words. Be careful not to speak too often and interrupt yourself, and allow your interlocutors time to speak and understand.`,
            defaultNarration,
        );
        logger.info("Heart: Initializing");
        this.initializeQuickSubscription();
        setInterval(() => {
            this.consult().subscribe((narration) => {
                this.quick.next({
                    when: new Date(),
                    content: {
                        explanation: narration,
                        content: narration,
                    },
                });
                // Parse the narration with cheerio and extract all <function/> calls
                const $ = cheerio.load(narration);
                const functionCalls = $("function").map((i, el) => ({
                    content: $(el).text(),
                    name: $(el).attr("name"),
                })).get();

                logger.info(
                    { functionCalls },
                    "Extracted function calls from narration",
                );
                for (const call of functionCalls) {
                    logger.debug({ call }, "Executing function call");
                    switch (call.name?.toLowerCase()) {
                        case "say":
                        case "speak":
                            this.sessions.forEach(async (session) => {
                                const wav = await speak(call.content);
                                session.connection.send({
                                    type: MessageType.Say,
                                    data: {
                                        words: call.content,
                                        wav,
                                    },
                                });
                            });
                            break;
                    }
                }
                this.sessions.forEach((session) => {
                    session.connection.send({
                        type: MessageType.Think,
                        data: narration,
                    });
                });
            });
            this.bottom.consult().subscribe((narration) => {
                this.quick.next({
                    when: new Date(),
                    content: {
                        explanation: narration,
                        content: narration,
                    },
                });
            });
        }, 5000);
    }

    protected cleanupSession(socket: WebSocket) {
        const session = this.sessions.get(socket);
        if (session) {
            session.subscriptions.forEach((subscription) =>
                subscription.unsubscribe()
            );
            this.sessions.delete(socket);
            logger.info(
                "Cleaned up session and unsubscribed from all observables",
            );
        }
    }
}
