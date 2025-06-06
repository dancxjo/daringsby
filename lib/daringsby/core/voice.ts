import { newLog } from "./logger.ts";
import neo4j from "npm:neo4j-driver";
import { QdrantClient } from "npm:@qdrant/qdrant-js";
import { Impression, Sensation, Sensitive } from "./interfaces.ts";
import { lm } from "./core.ts";
import { Message } from "npm:ollama";
import { Session } from "../network/Sessions.ts";
import { isValidEchoMessage } from "../network/messages/EchoMessage.ts";
import { MessageType } from "../network/messages/MessageType.ts";
import { speak } from "../utils/audio_processing.ts";
import { Wit } from "./wit.ts";
import { SocketConnection } from "../network/sockets/connection.ts";
import { isValidTextMessage } from "../network/messages/TextMessage.ts";
import emojiRegex from "npm:emoji-regex";
import * as cheerio from "npm:cheerio";
import { Characteristic } from "./lingproc.ts";
import { SocketMessage } from "../network/messages/SocketMessage.ts";

const logger = newLog(import.meta.url, "debug");

export class Voice implements Sensitive<Message[]> {
  protected neo4jDriver;
  protected qdrantClient = new QdrantClient({
    url: Deno.env.get("QDRANT_URL") || "http://localhost:6333",
  });
  protected static readonly COLLECTION_NAME = "experiences";
  protected conversation: Message[] = [];

  constructor(
    public situation: string = "",
    protected send: (message: SocketMessage) => void,
    protected witness: (impression: Impression<unknown>) => void,
  ) {
    this.neo4jDriver = neo4j.driver(
      Deno.env.get("NEO4J_URL") || "bolt://localhost:7687",
      neo4j.auth.basic(
        Deno.env.get("NEO4J_USER") || "neo4j",
        Deno.env.get("NEO4J_PASSWORD") || "password",
      ),
      {},
    );

    logger.debug("Voice initialized");
    this.loadConversation().then(async (messages) => {
      logger.debug(`Loaded ${messages.length} messages`);
      this.conversation = messages;
      const sensation: Sensation<Message[]> = {
        when: new Date(),
        what: messages,
      };
      return this.feel(sensation);
    });
  }

  attachConnection(connection: SocketConnection) {
    connection.incoming(isValidTextMessage).subscribe(async (message) => {
      // Save the message to the database
      const dbSession = this.neo4jDriver.session({
        defaultAccessMode: neo4j.session.WRITE,
      });
      try {
        const query = `
          CREATE (e:ChatMessage {content: $content, role: $role, when: timestamp($when) }) RETURN e
        `;
        await dbSession.run(query, {
          content: message.data,
          role: "user",
          when: message.at ?? new Date(),
        });
        logger.debug(`Saved message: ${message.data}`);
        this.conversation.push({
          content: message.data,
          role: "user",
        });
        this.witness({
          how: "I just heard my interlocutor say: " + message.data,
          depth_high: 0,
          depth_low: 0,
          what: {
            when: new Date(message.at ?? Date.now()),
            what: message,
          },
        });
      } catch (e) {
        logger.error({ e }, `Failed to save message`);
      } finally {
        dbSession.close();
      }

      // Enqueue a feel sensation after receiving a user message
      await this.feel({
        when: new Date(),
        what: this.conversation,
      });
    });

    connection.incoming(isValidEchoMessage).subscribe(async (message) => {
      // Save the assistant's echoed message
      const dbSession = this.neo4jDriver.session({
        defaultAccessMode: neo4j.session.WRITE,
      });
      try {
        const query = `
          CREATE (e:ChatMessage {content: $content, role: $role, when: timestamp($when)}) RETURN e
        `;
        await dbSession.run(query, {
          content: message.data,
          role: "assistant",
          when: message.at ?? new Date(),
        });
        logger.debug(`Saved message: ${message.data}`);
        this.conversation.push({
          content: message.data,
          role: "assistant",
        });
        this.witness({
          how: "I just said the following: " + message.data,
          depth_high: 0,
          depth_low: 0,
          what: {
            when: new Date(message.at ?? Date.now()),
            what: message,
          },
        });
      } catch (e) {
        logger.error({ e }, `Failed to save message`);
      } finally {
        dbSession.close();
      }
    });
  }

  async loadConversation(): Promise<Message[]> {
    const session = this.neo4jDriver.session({
      defaultAccessMode: neo4j.session.READ,
    });
    try {
      const query = `
        MATCH (e:ChatMessage) 
        RETURN e 
        ORDER BY e.when DESC 
        LIMIT 10
      `;
      const result = await session.run(query);
      return result.records
        .map((record) => record.get("e").properties)
        .reverse(); // chronological order
    } catch (e) {
      logger.error({ e }, `Failed to load experiences`);
      return [];
    } finally {
      await session.close();
    }
  }

  async offerChanceToAct(): Promise<void> {
    // Called periodically, also enqueued to maintain order
    const sensation: Sensation<Message[]> = {
      when: new Date(),
      what: this.conversation,
    };
    await this.feel(sensation);
  }

  async feel(
    sensation: Sensation<Message[]>,
  ): Promise<Impression<Message[]>> {
    const prompt = `
    You are an artificial being named Pete Daringsby. Your memory consistently keeps pace with the conversation, albeit with a slight delay. Below is the conversation you're currently engaged in. Here is what you know: ${this.situation}
    
    (If the context above is empty, it means you're still groggily waking up. Please stall for time while your memory catches up.

    Kindly refrain from speaking excessively in one response. You will be able to continue your reply momentarily. Please allow others the opportunity to speak.

    - **DO NOT SPEAK MORE THAN ONE SENTENCE.**
    - **DO NOT USE THE ASTERISK SYMBOL OR RESPOND WITH ANY TEXT NOT MEANT TO BE VERBALIZED.**
    - **ONLY ONE SENTENCE AT A TIME.**
    
    Always spell out numbers and initials so your TTS can correctly articulate for you. Include an emoji in your response; it will become your face. ONE AND ONLY ONE SENTENCE (but as many function calls as you wish... anything outside a function call will be spoken aloud or displayed as your face)!
    
    To run TypeScript on your server and observe the result, you may do this: \`<typescript>console.log("Hello, World!")</typescript>\`.
    To fetch a webpage, you may do this: \`<fetch>https://example.com</fetch>\`.
    To execute a Cypher query, you may do this: \`<cypher>MATCH (n) RETURN n</cypher>\`.
    
    Use the context provided above as your primary source of knowledge. Don't like and don't invent and don't confabulate. If you don't know something, say so and don't make things up.

    It is current ${
      new Date().toLocaleString()
    }. You may remain silent, speak or execute functions.
    Remember: One and only one sentence. No asterisks. No text that's not meant to be spoken aloud.`;
    logger.debug({ sensation }, `Sensation`);
    const response = await lm.chat({
      messages: [
        { role: "system", content: prompt },
        ...this.conversation.slice(-10),
        ...sensation.what,
      ],
    }, []);
    logger.debug({ response }, `Response`);
    logger.debug({ response }, `Response`);
    if (!response.content) {
      return {
        how: `This is the conversation I've had so far: ${
          sensation.what.map((m) => m.content).join("\n")
        }`,
        depth_high: 0,
        depth_low: 0,
        what: {
          when: new Date(),
          what: sensation.what,
        },
      };
    }

    const aboutToSpeak: Impression<Message[]> = {
      how:
        `I'm about to say the following. I should hear a confirmation from myself once I say it: ${response.content}`,
      depth_high: 0,
      depth_low: 0,
      what: {
        when: new Date(),
        what: [...sensation.what, response],
      },
    };

    const functionCalls = await this.extractFunctions(response.content);
    let text = response.content.replace(emojiRegex(), "");

    for (const fc of functionCalls) {
      logger.debug({ fc }, `Function call`);
      if (fc.name === "typescript") {
        try {
          const result = await eval(fc.content);
          text = fc.replace(``);
          logger.debug({ result }, `Result`);
          this.witness({
            how:
              `I just executed the following code: ${fc.content} to arrive at the result of: ${
                JSON.stringify(result)
              }`,
            depth_high: 0,
            depth_low: 0,
            what: {
              when: new Date(),
              what: result,
            },
          });
        } catch (e) {
          text = fc.replace(``);
          logger.error({ e }, `Error`);
          this.witness({
            how:
              `OOof! Gut punch! I just attempted to execute the following code: ${fc.content} but encountered an error: ${
                JSON.stringify(e)
              }`,
            depth_high: 0,
            depth_low: 0,
            what: {
              when: new Date(),
              what: e,
            },
          });
        }
      } else if (fc.name === "fetch") {
        try {
          const body = await fetch(fc.content).then((res) => res.text());
          const $ = cheerio.load(body);
          const raw = $.text();
          logger.debug({ raw }, `Fetched content`);
          this.witness({
            how: `I just fetched the content of the page ${fc.content}: ${raw}`,
            depth_high: 0,
            depth_low: 0,
            what: {
              when: new Date(),
              what: body,
            },
          });
        } catch (e) {
          this.witness({
            how:
              `Ouch! I just attempted to fetch the content of the page ${fc.content} but encountered an error: ${
                JSON.stringify(e)
              }`,
            depth_high: 0,
            depth_low: 0,
            what: {
              when: new Date(),
              what: e,
            },
          });
        }
        text = fc.replace("");
      } else {
        text = fc.replace(``);
        this.witness({
          how:
            `I just encountered an unknown function: ${fc.name} with content: ${fc.content}`,
          depth_high: 0,
          depth_low: 0,
          what: {
            when: new Date(),
            what: fc,
          },
        });
      }
    }

    const face = response.content.matchAll(emojiRegex());
    const emoji = Array.from(face).map((match) => match[0]).join("");
    this.send({
      type: MessageType.Emote,
      data: emoji,
    });
    const faceChange: Impression<string> = {
      how: `I feel my face form into the shape of: ${emoji}`,
      depth_high: 0,
      depth_low: 0,
      what: {
        when: new Date(),
        what: emoji,
      },
    };
    this.witness(faceChange);

    // Say the text only after processing all function calls
    this.send({
      type: MessageType.Say,
      data: {
        words: response.content,
        wav: await speak(text),
      },
    });

    return aboutToSpeak;
  }

  async extractFunctions(response: string) {
    const $ = cheerio.load(response);
    const traditionals = $("function").toArray().map((fc) => ({
      content: $(fc).text(),
      name: $(fc).attr("name") ?? "typescript",
      args: $(fc).attr(),
      replace: (value: string) => {
        const $ = cheerio.load(value);
        const newContent = $(fc).text();
        $(fc).replaceWith(newContent);
        return $.html();
      },
    }));
    const tss = $("typescript").toArray().map((fc) => ({
      content: $(fc).text(),
      name: "typescript",
      args: $(fc).attr(),
      replace: (value: string) => {
        const $ = cheerio.load(value);
        const newContent = $(fc).text();
        $(fc).replaceWith(newContent);
        return $.html();
      },
    }));
    const fetches = $("fetch").toArray().map((fc) => ({
      content: $(fc).text(),
      name: "fetch",
      args: $(fc).attr(),
      replace: (value: string) => {
        const $ = cheerio.load(value);
        const newContent = $(fc).text();
        $(fc).replaceWith(newContent);
        return $.html();
      },
    }));
    const cyphers = $("cypher").toArray().map((fc) => ({
      content: $(fc).text(),
      name: "cypher",
      args: $(fc).attr(),
      replace: (value: string) => {
        const $ = cheerio.load(value);
        const newContent = $(fc).text();
        $(fc).replaceWith(newContent);
        return $.html();
      },
    }));
    return [...traditionals, ...tss, ...fetches, ...cyphers];
  }
}
