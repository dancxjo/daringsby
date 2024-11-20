import { map, Observable, of, Subject, tap } from "npm:rxjs";
import logger from "../core/logger.ts";
import { Genie } from "./Genie.ts";
import { narrate } from "../utils/narration.ts";
import { wholeResponse } from "../utils/chunking.ts";
import { chatify } from "../utils/llamification.ts";
import { Session } from "../network/Sessions.ts";
import * as cheerio from "npm:cheerio";
import { toSayMessage } from "../utils/tts.ts";
import { Message } from "npm:ollama";
import { Sensation } from "../core/interfaces.ts";

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
        const textToSpeak = $("function")
            .filter((_, el) => $(el).attr("name")?.toLowerCase() === "say").map(
                (_, el) => $(el).text(),
            ).get();

        if (textToSpeak.length) {
            logger.debug({ textToSpeak }, "Voice: Text to speak");
            this.session.subscriptions.push(
                of(textToSpeak.join("\n")).pipe(
                    // sentenceBySentence(),
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
    }

    constructor(
        name: string,
        situation$: Subject<Sensation<string>>,
        protected session: Session,
    ) {
        super(
            name,
            `This part of the mind produces speech and other vocalizations as well as unvoiced conscious thoughts. The voice has access to a running log of the conversation.`,
            `This part of the mind produces speech and other vocalizations as well as unvoiced conscious thoughts. Whatever the voice thinks is Pete's next thought. The voice can surround brief text with <function name='say'>...</function> to cause Pete to speak out loud. The voice has access to a running log of the conversation.\n\nCurrent context: {{context}}\n\nReminder: Keep up with the conversation. Don't speak too often or repeat yourself over and over. Pay attention to what you're in the process of saying.`,
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
                `You are Pete Daringsby, a pseudo-conscious experimental software application. Here are the circumstances:\n${this.situation}\n\nContinue the conversation. Do not speak aloud on every call. Slow down for humans to be able to understand you.`,
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
