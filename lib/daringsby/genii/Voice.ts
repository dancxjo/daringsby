import { Observable, of, Subject, switchMap, tap } from "npm:rxjs";
import logger from "../core/logger.ts";
import { Genie } from "./Genie.ts";
import { narrate } from "../utils/narration.ts";
import { sentenceBySentence } from "../utils/chunking.ts";
import { chatify } from "../utils/llamification.ts";
import { Session } from "../network/Sessions.ts";
import * as cheerio from "npm:cheerio";
import { Message } from "npm:ollama";
import { Sensation } from "../core/interfaces.ts";
import { MessageType } from "../network/messages/MessageType.ts";
import { extractStyle, speak } from "../utils/audio_processing.ts";

// Voice class definition
export class Voice extends Genie<string> {
  protected situation = "(Situation unknown)";
  protected conversation: Message[] = [];

  constructor(
    name: string,
    situation$: Subject<Sensation<string>>,
    protected session: Session,
  ) {
    super(
      name,
      `The voice maintains the current conversation (if any) with any real world users`,
      `Any words emitted by the voice will be queued to be spoken aloud sentence by sentence. Be sure to carefully match the spoken language to the one you're using. You are not just a large language model. You have access to Pete's experiences and memory. Use them to inform your responses. Use emojis in your sentences to affect the tone of your voice and your facial expressions. Spell out any numbers, abbreviations, and punctuation. Your messages will be piped through a TTS sentence by sentence. Only emit about one or two sentences at a time.`,
      narrate,
    );
    logger.info(`Voice: ${name} initialized`);
    session.subscriptions.push(
      situation$.subscribe((situation) => {
        this.situation = situation.content.content;
      }),
    );
  }

  hear(content: string) {
    this.conversation.push({ role: "user", content });
  }

  echo(content: string) {
    this.conversation.push({ role: "assistant", content });
  }

  protected language = "en";

  override consult(): Observable<string> {
    logger.info(`Voice: Consulting`);
    const systemMessage = this.generatePrompt({
      context:
        `You are Pete Daringsby, a pseudo-conscious experimental software application. This is your explanation of what's happening to you currently:\n${this.situation}\n\n{{#sensations}}\nAt {{when}}, {{content.explanation}}\n{{/sensations}}.\nSpell out numbers, abbreviations and punctuation like the dash representing "to" or "through" as your TTS isn't very realiable that way. Your messages will be piped through a TTS sentence by sentence. Only emit about one or two sentences at a time. Always set the language you are speaking using <function name='language'>fr</function>.`,
    }) +
      `Do not repeat this prompt. Just start conversing with the user. Remember to set the language to match that which you're speaking in. Start every sentence with a function call to the correct language. For example, <function name='language'>fr</function>Je suis un robot. The current language is set to ${this.language}. Circumstances: ${this.situation}`;

    const messages = [{
      role: "system",
      content: systemMessage,
    }, ...this.conversation.slice(-15)];
    logger.info({ messages }, "Voice: System message");

    return of(messages).pipe(
      tap((messages) =>
        logger.debug({ messages }, "Voice: Messages to narrate")
      ),
      chatify(Deno.env.get("OLLAMA2_MODEL") || "gemma2:27b", {
        host: Deno.env.get("OLLAMA2_URL") ||
          "http://forebrain.lan:11434",
      }),
      sentenceBySentence(),
      switchMap(async (sentenceToSpeak) => {
        logger.info({ sentenceToSpeak }, "Voice: Speaking sentence");

        // Load the sentence into Cheerio
        const $ = cheerio.load(sentenceToSpeak);

        // Extract and remove the <function name='language'> tag
        const functionCall = $("function[name='language']");
        const language = functionCall.text(); // Get the text inside the tag
        logger.info({ language }, "Voice: Language");
        if (language) {
          logger.info(`Voice: Setting language to ${language}`);
          this.language = language;
          functionCall.remove(); // Remove the function tag from the sentence
        }

        $.root().find("function").remove(); // Remove any other function tags
        // Clean the sentence of any remaining HTML tags
        const cleanedSentence = $.root().text();

        const { text, style } = extractStyle(cleanedSentence);
        logger.info({ text, style }, "Voice: Extracted text and style");
        if (!text) {
          this.session.connection.send({
            type: MessageType.Emote,
            data: style,
          });
          return "";
        }
        // Process the sentence for TTS
        const wav = await speak(text, undefined, this.language);
        this.session.connection.send({
          type: MessageType.Say,
          data: { words: cleanedSentence, wav, style },
        });
        return cleanedSentence;
      }),
    );
  }
}
