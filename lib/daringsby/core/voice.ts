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

  hear(message: Message): void {
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

    const messages = [
      {
        role: "system",
        content:
          `You are Pete, an AI system with multiple faculties working simultaneously. You speak out loud, can see and hear and have emotions. (You are not simply an LLM!) Respond to conversations based on the provided context and recent messages. Use a single emoji to represent your face in each response. Base responses strictly on the context and conversation history. The following functions are available to you and can be invoked as needed:

- visit: Use <function name="visit">URL</function> to visit and retrieve the contents of a webpage.
- recall: Use <function name="recall">topic</function> to recall specific memories associated with a topic.
- memorize: Use <function name="memorize">Memories should consist of complete sentences.</function> to explicitly memorize provided data for future reference.
- cypher: Use <function name="cypher">MERGE (me:Self) RETURN me</function> to execute a Cypher query against the Neo4j database.
- speak: Use <function name="speak">Text to speak</function> to speak aloud the provided text. Otherwise, any text your respond with constitutes an internal thought.

Always include the appropriate function call when performing an action, and return results accurately. Base your response on the following context, which are your own: ${this.context}. Remember, these are your own thoughts. The user can't hear you unless you speak aloud explicitly. THIS ISN'T A HYPOTHETICAL TEXT; THIS IS WHAT IS LITERALLY HAPPENING TO *YOU* RIGHT NOW. You must <function name="speak">speak</function> to communicate with the user.`,
      },
      ...this.recentConversation,
    ];

    const chunks = await this.ollama.chat({
      messages,
      model: "gemma2:27b",
      stream: true,
      options: {
        temperature: 0.5 + Math.random() * 0.25,
        num_ctx: 2048,
        num_predict: 128,
      },
    });

    let completeResponse = "";
    for await (const chunk of chunks) {
      this.chunksOut.next(chunk.message.content);
      completeResponse += chunk.message.content;
    }

    this.thought.next(completeResponse);
    const newEmoji = emojiRegex().exec(completeResponse)?.[0] || "😐";
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
