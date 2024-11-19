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
            logger.info({ textToSpeak }, "Voice: Text to speak");
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
        situation$: Subject<string>,
        protected session: Session,
    ) {
        super(
            name,
            `This part of the mind produces speech and other vocalizations as well as unvoiced conscious thoughts. The voice has access to a running log of the conversation.`,
            `This part of the mind produces speech and other vocalizations as well as unvoiced conscious thoughts. Whatever the voice thinks is Pete's next thought. The voice can surround brief text with <function name='say'>...</function> to cause Pete to speak out loud. The voice has access to a running log of the conversation.\n\nCurrent context: {{context}}`,
            narrate,
        );
        logger.info(`Voice: ${name} initialized`);
        session.subscriptions.push(
            situation$.subscribe((situation) => {
                this.situation = situation;
            }),
        );
    }

    override consult(): Observable<string> {
        logger.info(`Voice: Consulting`);
        const systemMessage = this.generatePrompt({ context: this.situation });
        const messages = [{
            role: "system",
            content: systemMessage,
        }, ...this.conversation];
        return of(messages).pipe(
            tap((messages) =>
                logger.debug({ messages }, "Voice: Messages to narrate")
            ),
            chatify(Deno.env.get("OLLAMA_MODEL") || "gemma2:27b", {
                host: Deno.env.get("OLLAMA2_URL") || "http://localhost:11434",
            }),
            wholeResponse(),
            map((narration) => {
                this.runFunctions(narration);
                return narration;
            }),
        );
    }
}
