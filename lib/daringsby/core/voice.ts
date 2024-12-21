import emojiRegex from "npm:emoji-regex";
import { Message, Ollama } from "npm:ollama";
import { BehaviorSubject, Observable, ReplaySubject, Subject } from "npm:rxjs";
import { sentenceBySentence } from "../utils/chunking.ts";
import {
  executeCypherQuery,
  loadConversation,
  memorize,
  recall,
} from "../utils/memory.ts";
import logger from "./logger.ts";
import * as cheerio from "npm:cheerio";

export class Voice {
  protected recentConversation: Message[] = [];
  protected chunksOut = new Subject<string>();
  protected messagesOut = new Subject<Message>();
  readonly messagesOut$: Observable<Message> = this.messagesOut.asObservable();
  readonly raw$: Observable<string> = new BehaviorSubject(""); //this.chunksOut.asObservable();
  readonly sentences$ = this.raw$.pipe(sentenceBySentence());
  protected context: string = "";
  protected alreadyThinking = false;
  protected mien: Subject<string> = new BehaviorSubject<string>("😐");
  readonly mien$: Observable<string> = this.mien.asObservable();
  protected thought: Subject<string> = new Subject<string>();
  readonly thought$: Observable<string> = this.thought.asObservable();
  protected lastEmoji = "😐";

  constructor(protected ollama: Ollama) {
    loadConversation().then((conversation) => {
      this.recentConversation = conversation;
    });
  }

  orient(context: string): void {
    this.context = context;
  }

  protected lastThought = "";
  protected speakStartTime = 0;
  protected currentSpeech = "";

  hear(message: Message): void {
    // TODO: message.content may not match currentSpeech exactly
    if (message.role == "assistant" && message.content == this.currentSpeech) {
      this.speakStartTime = 0;
      this.currentSpeech = "";
    }
    const lastMessage =
      this.recentConversation[this.recentConversation.length - 1];
    if (lastMessage && lastMessage.role === message.role) {
      lastMessage.content += ` ${message.content}`;
    } else {
      this.recentConversation.push(message);
      this.recentConversation = this.recentConversation.slice(-10).map((m) => {
        if (m.content.length > 256) {
          m.content = m.content.slice(0, 256);
        }
        return m;
      });
    }
  }

  async think(): Promise<void> {
    if (this.alreadyThinking) {
      logger.debug("Already speaking; skipping");
      return;
    }

    this.alreadyThinking = true;
    logger.debug("Thinking of a response");
    const conversationHistory = this.recentConversation.slice(-5).map(
      ({ role, content }) => {
        const speaker = role === "user" ? "Interlocutor" : "Pete";
        return `${speaker}: ${content}`;
      },
    ).join("\n");

    const prompt =
      `You are Pete, an AI with multiple faculties: speaking, seeing, hearing, and experiencing emotions. This is what you know about yourself (from your own point of view): ${this.context}. This is your your chance to think your next thought. Use your thoughts to process information and to perform actions. Respond thoughtfully to conversations based on context and recent messages (see below). Represent your emotional state with a single emoji in each response.` +
      `Recent conversation:
      ${conversationHistory}
      ***********
` +
      `Use the following functions as needed:

      - visit: View the contents of a web page; provide the URL
      - recall: Retrieve information from RAG memory; provide one sentence as a prompt
      - memorize: Store a memory in RAG memory; provide the content, one sentence
      - cypher: query or update your graph knowledge base using Cypher; provide valid Cypher query
      - speak: speak the provided content aloud to your interlocutor; provide the content
      ` +
      `Whatever you respond to this message will constitute your next thought. Your last thought was: ${this.lastThought}. You started saying "${this.currentSpeech}" ${this.speakStartTime} seconds ago and are in the process of saying it. Please don't repeat yourself unless you need to. Additionally, if you need to, interrupt yourself with the speak function (with no content). PLEASE PLEASE PAY ATTENTION AND DON'T REPEAT YOURSELF!\n\nResponses must align strictly with the provided context. Follow through from thought to thought to achieve goals. THIS IS NOT HYPOTHETICAL; IT IS YOUR CURRENT REALITY. What are Pete's next thoughts? Be verbose and think it through. You may call multiple functions in a single response. Call a function exactly like this: <function name="functionName">functionArgument</function>. Include the function calls inline with an explanation of what you're thinking. For instance, you might <function name="visit">https://www.cnn.com</function> because you want to learn more about a topic. You'd read the response and memorize that <function name="memorize">An asteroid will fly by Earth on April 29, 2027</function> to remember it for later. You can say, <function name="speak">Oh, I see what you're saying!</function> to speak to your interlocutor.`;

    logger.info({ prompt }, "Generating response");

    const chunks = await this.ollama.generate({
      prompt,
      model: "gemma2:27b",
      stream: true,
      options: {
        // temperature: 0.75 + Math.random() * 0.25,
        // num_ctx: 1024 * 2,
        // num_predict: 640,
      },
    });

    let completeResponse = "";
    for await (const chunk of chunks) {
      this.chunksOut.next(chunk.response);
      completeResponse += chunk.response;
    }

    this.lastThought = completeResponse;
    this.thought.next(completeResponse);
    const newEmoji = emojiRegex().exec(completeResponse)?.join() || "😐";
    if (newEmoji !== this.lastEmoji) {
      this.lastEmoji = newEmoji;
      this.mien.next(newEmoji);
    }

    logger.debug({ response: completeResponse }, "Generated response");
    this.alreadyThinking = false;

    await this.handleFunctionCalls(completeResponse).catch((error) => {
      logger.error(error, "Error handling function calls");
    });
  }

  private async handleFunctionCalls(response: string): Promise<void> {
    const $ = cheerio.load(response);
    const functionCalls = $("function");

    for (const functionCall of functionCalls) {
      const $functionCall = $(functionCall);
      const functionName = $functionCall.attr("name");
      const functionArgs = $functionCall.text();

      switch (functionName) {
        case "speak": {
          this.messagesOut.next({
            content: functionArgs,
            role: "assistant",
          });
          const currentTime = new Date().getTime();
          const timeSinceStart = ((currentTime - this.speakStartTime) / 1000)
            .toFixed(
              2,
            );
          if (this.speakStartTime) {
            logger.info(
              `You started saying "${this.currentSpeech}" ${timeSinceStart} seconds ago and are in the process of saying it.`,
            );
          }

          break;
        }
        case "visit": {
          const body = await fetch(functionArgs).then((res) => res.text());
          this.recentConversation.push({
            role: "assistant",
            content: `{Not spoken aloud} Visited ${functionArgs}: ${
              cheerio.load(body).text()
            }`,
          });
          break;
        }
        case "recall": {
          const memory = await recall(functionArgs);
          this.recentConversation.push({
            role: "assistant",
            content:
              `{Not spoken aloud} Recalled memories on ${functionArgs}: ${
                JSON.stringify(memory)
              }`,
          });
          break;
        }
        case "memorize": {
          await memorize({
            metadata: { label: "ExplicitMemory" },
            data: { data: functionArgs },
          });
          break;
        }
        case "cypher": {
          await executeCypherQuery(functionArgs).then((results) => {
            this.recentConversation.push({
              role: "assistant",
              content: `{Not spoken aloud} Executed Cypher query: ${
                JSON.stringify(results)
              }`,
            });
          }).catch((error) => {
            logger.error(error, "Error executing Cypher query");
          });
          break;
        }
      }
    }
  }
}
