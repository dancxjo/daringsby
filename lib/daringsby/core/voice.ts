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
    connection.incoming(isValidTextMessage).subscribe((message) => {
      // Write the message to the database
      const dbSession = this.neo4jDriver.session({
        defaultAccessMode: neo4j.session.WRITE,
      });
      try {
        const query =
          `CREATE (e:ChatMessage {content: $content, role: $role, when: datetime($when)}) RETURN e`;
        const result = dbSession.run(query, {
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
    connection.incoming(isValidEchoMessage).subscribe((message) => {
      // Write the message to the database
      const dbSession = this.neo4jDriver.session({
        defaultAccessMode: neo4j.session.WRITE,
      });
      try {
        const query =
          `CREATE (e:ChatMessage {content: $content, role: $role, when: datetime($when)}) RETURN e`;
        const result = dbSession.run(query, {
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
      `You are an artificial being. The following is a conversation you're currently having. Here is what you know: ${this.context}\n\nDo not speak excessively in one response. You will be able to continue your response momentarily. Please allow others to speak.`;

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

    this.connection.send({
      type: MessageType.Say,
      data: {
        words: response.content,
        wav: await speak(response.content),
      },
    });

    return aboutToSpeak;
  }
}
