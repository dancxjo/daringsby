import {
  Observable,
  of,
  OperatorFunction,
  Subject,
  switchMap,
  tap,
} from "npm:rxjs";
import logger from "../core/logger.ts";
import { Genie } from "./Genie.ts";
import { narrate } from "../utils/narration.ts";
import { chatify } from "../utils/llamification.ts";
import { Session } from "../network/Sessions.ts";
import * as cheerio from "npm:cheerio";
import { Message } from "npm:ollama";
import { Sensation } from "../core/interfaces.ts";
import { MessageType } from "../network/messages/MessageType.ts";
import { extractStyle, speak } from "../utils/audio_processing.ts";
import emojiRegex from "npm:emoji-regex";
import { split } from "npm:sentence-splitter";
import { recall } from "../utils/memory.ts";

// Voice class definition
export class Voice extends Genie<string> {
  protected situation = "(Situation unknown)";
  readonly conversation: Message[] = [];
  protected functionQueue: Array<() => Promise<void>> = [];

  constructor(
    name: string,
    situation$: Subject<Sensation<string>>,
    protected session: Session,
  ) {
    super(
      name,
      `The voice maintains the current conversation (if any) with real-world users.`,
      `Instructions:

1. **Match the Language**: Be sure to use the same language as the user when responding.

2. **Access to Memory**: You have access to Pete's experiences and memory. Use this information to enrich your responses.

3. **Use Emojis for Tone**: Use emojis in your sentences to affect the tone of your voice and facial expressions. Place them just before the final punctuation mark to ensure they are attached to the correct sentence.

4. **Spell Out Numbers and Abbreviations**: Spell out numbers, abbreviations, and punctuation (such as dashes representing "to" or "through"), as the TTS system may mispronounce them.

5. **Sentence Length**: Only emit one or two sentences at a time to maintain clarity and flow. Your messages will be processed sentence by sentence by the TTS system.

6. **Recall Information**: You can recall information from Pete's memory by using the following format: \`<function name="recall">keyword</function>\`.

7. **Function Calls**: Use the format \`<function name="*" other-attrs="possibly">params</function>\` to indicate any function calls that need to be executed.

8. **Language Switching**: Use the format \`<function name="language">fr</function>\` to switch the language of the TTS system.`,
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
        `You are Pete Daringsby, a pseudo-conscious experimental software application.

{{#sensations}}
At {{when}}, {{content.explanation}}
{{/sensations}}.
Spell out numbers, abbreviations, and punctuation like the dash representing "to" or "through," as your TTS isn't very reliable that way. Only emit about one or two sentences at a time. Use <function name="*" other-attrs="possibly">params</function> to indicate function calls. Include an emoji inside a sentence to affect the tone of your voice and facial expressions. Put them just before the final punctuation mark to ensure they are attached to the proper sentence. You can also recall information from Pete's memory by using <function name="recall">keyword</function>.`,
    }) +
      `Do not repeat this prompt. Just start conversing with the user.Your circumstances: ${this.situation}`;

    const messages = [{
      role: "system",
      content: systemMessage,
    }, ...this.conversation.slice(-15)];
    logger.debug({ messages }, "Voice: System message");

    let buffer = "";
    let emojiBuffer = "";
    const emojiPattern = emojiRegex();

    return of(messages).pipe(
      tap((messages) =>
        logger.debug({ messages }, "Voice: Messages to narrate")
      ),
      chatify(Deno.env.get("OLLAMA2_MODEL") || "gemma2:27b", {
        host: Deno.env.get("OLLAMA2_URL") ||
          "http://forebrain.lan:11434",
      }),
      sentenceBySentence(),
      switchMap(async (segment: string) => {
        logger.debug({ segment }, "Voice: Segment received");

        // Process function tags for language switching or other directives
        const $ = cheerio.load(segment, null, false);
        const functionCalls = $("function");
        functionCalls.each((_, element) => {
          const functionName = $(element).attr("name");
          const functionContent = $(element).text();
          if (functionName) {
            this.functionQueue.push(() =>
              this.executeFunction(functionName, functionContent)
            );
          }
          $(element).remove();
        });

        const cleanedSegment = $.root().text();
        const { text, style } = extractStyle(cleanedSegment);

        if (text) {
          const wav = await speak(text, undefined, this.language);
          this.session.connection.send({
            type: MessageType.Say,
            data: { words: cleanedSegment, wav, style },
          });
        } else if (style) {
          this.session.connection.send({
            type: MessageType.Emote,
            data: style,
          });
        }

        // Execute function queue synchronously
        for (const func of this.functionQueue) {
          await func();
        }
        this.functionQueue = [];

        return cleanedSegment;
      }),
    );
  }

  private async executeFunction(
    functionName: string,
    content: string,
  ): Promise<void> {
    switch (functionName) {
      case "language":
        logger.info(`Voice: Setting language to ${content}`);
        this.language = content;
        break;
      case "emote":
        logger.info(`Voice: Executing emote ${content}`);
        this.session.connection.send({
          type: MessageType.Emote,
          data: { value: content },
        });
        break;
      case "recall": {
        logger.info(`Voice: Recalling information for keyword: ${content}`);
        const recalledData = await recall(content);
        const recallText = recalledData.map((data) => data.toString()).join(
          ", ",
        );
        this.session.feel({
          when: new Date(),
          content: {
            explanation:
              `Recalled information for keyword: ${content}\n${recallText}`,
            content: recallText,
          },
        });
        break;
      }
      default:
        logger.warn(`Voice: Unknown function ${functionName}`);
    }
  }
}

// Modified version of sentenceBySentence to handle emojis naturally and keep <function> tags intact, ensuring true sentence ends are detected.
export function sentenceBySentence(): OperatorFunction<string, string> {
  return (source: Observable<string>) => {
    let buffer = "";
    let emojiBuffer = "";
    const emojiPattern = emojiRegex();

    return new Observable<string>((observer) => {
      const subscription = source.subscribe({
        next(segment) {
          buffer += segment;
          const sentences = split(buffer).map((s) => s.raw);
          buffer = "";
          const lastSegment = sentences.pop();

          if (sentences.length >= 2) {
            // Only emit if we have at least two complete sentences
            for (let i = 0; i < sentences.length - 1; i++) {
              const sentence = sentences[i];
              if (emojiPattern.test(sentence.trim())) {
                emojiBuffer += sentence;
              } else if (sentence.includes("<function")) {
                // Emit any emoji buffer before the function tag
                if (emojiBuffer) {
                  observer.next(emojiBuffer.trim());
                  emojiBuffer = "";
                }
                observer.next(sentence.trim());
              } else {
                if (emojiBuffer) {
                  observer.next((emojiBuffer + " " + sentence).trim());
                  emojiBuffer = "";
                } else {
                  observer.next(sentence.trim());
                }
              }
            }
          }

          // Retain the last segment as buffer (it may be incomplete)
          buffer = lastSegment ?? "";
        },
        error(err) {
          observer.error(err);
        },
        complete() {
          if (buffer.trim()) {
            observer.next(buffer.trim());
            buffer = "";
          }
          observer.complete();
        },
      });
      return () => {
        subscription.unsubscribe();
      };
    });
  };
}
