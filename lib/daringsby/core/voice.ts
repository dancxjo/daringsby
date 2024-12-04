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

const logger = newLog(import.meta.url, "debug");

export class Voice implements Sensitive<Message[]> {
  protected neo4jDriver;
  protected qdrantClient = new QdrantClient({
    url: Deno.env.get("QDRANT_URL") || "http://localhost:6333",
  });
  protected static readonly COLLECTION_NAME = "experiences";
  protected conversation: Message[] = [];

  constructor(
    public context: string = "",
    protected connection: SocketConnection,
    protected wit: Wit,
  ) {
    this.neo4jDriver = neo4j.driver(
      Deno.env.get("NEO4J_URL") || "bolt://localhost:7687",
      neo4j.auth.basic(
        Deno.env.get("NEO4J_USER") || "neo4j",
        Deno.env.get("NEO4J_PASSWORD") || "password",
      ),
      {},
    );
    connection.incoming(isValidTextMessage).subscribe(async (message) => {
      // Write the message to the database
      const dbSession = this.neo4jDriver.session({
        defaultAccessMode: neo4j.session.WRITE,
      });
      try {
        const query =
          `CREATE (e:ChatMessage {content: $content, role: $role, when: timestamp($when) }) RETURN e`;
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
        wit.enqueue({
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
      this.feel({
        when: new Date(),
        what: this.conversation,
      }).then((impression) => {
        wit.enqueue(impression);
      });
    });
    connection.incoming(isValidEchoMessage).subscribe(async (message) => {
      // Write the message to the database
      const dbSession = this.neo4jDriver.session({
        defaultAccessMode: neo4j.session.WRITE,
      });
      try {
        const query =
          `CREATE (e:ChatMessage {content: $content, role: $role, when: timestamp($when)}) RETURN e`;
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
        wit.enqueue({
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
    logger.debug("Voice initialized");
    this.loadConversation().then((messages) => {
      logger.debug(`Loaded ${messages.length} messages`);
      this.conversation = messages;
      const sensation: Sensation<Message[]> = {
        when: new Date(),
        what: messages,
      };
      this.feel(sensation).then((impression) => {
        logger.debug("Voice initialized");
        wit.enqueue(impression);
      });
    });
  }

  async loadConversation(): Promise<Message[]> {
    const session = this.neo4jDriver.session({
      defaultAccessMode: neo4j.session.READ,
    });
    try {
      const query =
        `MATCH (e:ChatMessage) RETURN e ORDER BY e.when DESC LIMIT 10`;
      const result = await session.run(query);
      return result.records.map((record) => record.get("e").properties);
    } catch (e) {
      logger.error({ e }, `Failed to load experiences`);
      return [];
    } finally {
      await session.close(); // Added 'await' to ensure proper session closure
    }
  }

  async feel(
    sensation: Sensation<Message[]>,
  ): Promise<Impression<Message[]>> {
    const prompt =
      `You are an artificial being named Pete Daringsby. Your memory is constantly keeping up with the conversation, but it's a smidge slow. The following is a conversation you're currently having. Here is what you know: ${this.context}\n(If the context above is empty, it means you're still groggily waking up. Stall for time while your memory catches up.\nDo not speak excessively in one response. You will be able to continue your response momentarily. Please allow others to speak. You may also execute typescript code and get the results using <function name="typescript">const meaningOfLife = 42; meaningOfLife;</function>. DO NOT SPEAK MORE THAN ONE SENTENCE. DO NOT USE THE ASTERISK SYMBOL OR RESPOND WITH ANY TEXT MEANT NOT TO BE VERBALIZED. NOT MORE THAN A SENTENCE AT A TIME. Spell out numbers and initials always so your TTS can correctly speak for you. Include an emoji in your response and it will become your face. ONE AND ONLY ONE SENTENCE (but as many function calls as you want...anything outside a function call will be spoken aloud or displayed as your face)!`;

    const response = await lm.chat({
      messages: [
        { role: "system", content: prompt },
        ...sensation.what,
      ],
    });
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
          this.wit.enqueue({
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
          text = fc.replace(`Error: ${e.message}`);
          logger.error({ e }, `Error`);
          this.wit.enqueue({
            how:
              `I just attempted to execute the following code: ${fc.content} but encountered an error: ${
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
      } else {
        text = fc.replace(``);
        this.wit.enqueue({
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
    this.connection.send({
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
    this.wit.enqueue(faceChange);
    this.connection.send({
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
    const functionCalls = $("function").toArray().map((fc) => ({
      content: $(fc).text(),
      name: $(fc).attr("name"),
      args: $(fc).attr(),
      replace: (value: string) => {
        const $ = cheerio.load(value);
        const newContent = $(fc).text();
        $(fc).replaceWith(newContent);
        return $.html();
      },
    }));
    return functionCalls;
  }
}
