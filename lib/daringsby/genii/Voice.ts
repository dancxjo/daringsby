import { map, Observable, of, Subject, tap } from "npm:rxjs";
import * as yml from "npm:yaml";
import logger from "../core/logger.ts";
import { Genie } from "./Genie.ts";
import { narrate } from "../utils/narration.ts";
import { sentenceBySentence, wholeResponse } from "../utils/chunking.ts";
import { chatify } from "../utils/llamification.ts";
import { Session } from "../network/Sessions.ts";
import * as cheerio from "npm:cheerio";
import { toSayMessage } from "../utils/tts.ts";
import { Message } from "npm:ollama";
import { Sensation } from "../core/interfaces.ts";
import { MessageType } from "../network/messages/MessageType.ts";
import { memorize, recall, runCypher } from "../utils/memory.ts";

export class Voice extends Genie<string> {
    hear(content: string) {
        this.conversation.push({ role: "user", content });
    }

    echo(content: string) {
        this.conversation.push({ role: "assistant", content });
    }
    protected situation = "(Situation unknown)";
    protected conversation: Message[] = [];

    protected runFunctions(narration: string) {
        const $ = cheerio.load(narration);

        const functions = $("function").map((_, el) => ({
            name: $(el).attr("name")?.toLowerCase(),
            content: $(el).text(),
        })).get();

        const face: string[] = [];
        const cyphers: string[] = [];
        const textToSpeak: string[] = [];

        functions.forEach((func) => {
            switch (func.name) {
                case "say":
                    textToSpeak.push(func.content);
                    break;
                case "emote":
                    face.push(func.content);
                    break;
                case "cypher":
                    cyphers.push(func.content);
                    break;
                case "memorize":
                    memorize({
                        metadata: {
                            label: "Memory",
                            when: new Date().toISOString(),
                        },
                        data: func.content,
                    });
                    break;
                case "recall":
                    recall(func.content).then((result) => {
                        this.feel({
                            when: new Date(),
                            content: {
                                explanation:
                                    `I just recalled information on ${func.content}: ${
                                        yml.stringify(result)
                                    }`,
                                content: yml.stringify(result),
                            },
                        });
                    });
                    break;
                case "context":
                    this.session.context = func.content;
                    break;
            }
        });

        if (textToSpeak.length) {
            logger.debug({ textToSpeak }, "Voice: Text to speak");
            this.session.subscriptions.push(
                of(textToSpeak.join("\n")).pipe(
                    sentenceBySentence(),
                    toSayMessage(),
                ).subscribe((message) => {
                    logger.debug(
                        { message: `${message.data.words}` },
                        "Voice: Sending message",
                    );
                    this.session.connection.send(message);
                }),
            );
        }
        if (face.length) {
            logger.debug({ face }, "Voice: Face to emote");
            this.session.connection.send({
                type: MessageType.Emote,
                data: face.join(""),
            });
            this.session.feel({
                when: new Date(),
                content: {
                    explanation: `I feel my face turn into this shape: ${
                        face.join("")
                    }`,
                    content: face.join(""),
                },
            });
        }
        if (cyphers.length) {
            logger.debug({ cyphers }, "Voice: Face to emote");
            cyphers.forEach(async (cypher) => {
                try {
                    const result = await runCypher(cypher);
                    this.feel({
                        when: new Date(),
                        content: {
                            explanation:
                                `I just ran a cypher query: ${cypher}\nResult: ${
                                    yml.stringify(result)
                                }`,
                            content: yml.stringify(result),
                        },
                    });
                } catch (error) {
                    logger.error({ error }, "Voice: Error running cypher");
                }
            });
        }
    }

    constructor(
        name: string,
        situation$: Subject<Sensation<string>>,
        protected session: Session,
    ) {
        super(
            name,
            `This part of the mind produces speech and other vocalizations as well as unvoiced conscious thoughts. The voice has access to a running log of the conversation.`,
            `This part of the mind produces speech and other vocalizations as well as unvoiced conscious thoughts. Whatever the voice thinks is Pete's next thought. The voice can surround brief text with <function name='say'>...</function> to cause Pete to speak out loud. The voice can also change the currently displayed face of Pete by putting an unicode emoji between <function name='emote'>...</function>. The voice has access to a running log of the conversation.\n\nCurrent context: {{context}}\n\nReminder: Keep up with the conversation. Don't speak too often or repeat yourself over and over. Pay attention to what you're in the process of saying. (You must use the correct syntax for the functions to work.) There is no tage called emote. You must use a function call. You may also control your graph database by sending cypher queries with the function call <function name='cypher'>cypher query</function>. To recall information on a specific topic, use the function call <function name='recall'>the topic you are interested in</function>. You can also consciously <function name='memorize'>Anything you want to remember</function>. You can also change your current context by sending a message with the type <function name='context'>MATCH (me:Self)
        OPTIONAL MATCH (latest:Situation)
        RETURN me, latest
        ORDER BY latest.timestamp DESC
        LIMIT 2</function>. Your context is the set of memories that you constantly receive.`,
            narrate,
        );
        logger.debug(`Voice: ${name} initialized`);
        session.subscriptions.push(
            situation$.subscribe((situation) => {
                this.situation = situation.content.content;
            }),
        );
    }

    override consult(): Observable<string> {
        logger.debug(`Voice: Consulting`);
        const systemMessage = this.generatePrompt({
            context:
                `You are Pete Daringsby, a pseudo-conscious experimental software application. This is your explanation of what's happening to you currently:\n${this.situation}\n\nContinue the conversation. Do not speak aloud on every call. Slow down for humans to be able to understand you.`,
        });

        logger.debug({ systemMessage }, "Voice: System message");
        const messages = [{
            role: "system",
            content: systemMessage,
        }, ...this.conversation.slice(-5)];
        return of(messages).pipe(
            tap((messages) =>
                logger.debug({ messages }, "Voice: Messages to narrate")
            ),
            chatify(Deno.env.get("OLLAMA_MODEL") || "gemma2:27b", {
                host: Deno.env.get("OLLAMA2_URL") ||
                    "http://forebrain.lan:11434",
            }),
            wholeResponse(),
            tap((narration) => {
                logger.debug({ narration }, "Voice: Narration received");
                this.session.feel({
                    when: new Date(),
                    content: {
                        explanation: `I just thought to myself: ${narration}`,
                        content: narration,
                    },
                });
            }),
            map((narration) => {
                this.runFunctions(narration);
                return narration;
            }),
        );
    }
}
