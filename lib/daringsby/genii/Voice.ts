// Import statements
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
import { GitInspector } from "../network/git/gitInspector.ts";

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
      `This part of the mind produces speech and other vocalizations as well as unvoiced conscious thoughts. The voice has access to a running log of the conversation.`,
      `This part of the mind produces speech and other vocalizations as well as unvoiced conscious thoughts. Whatever the voice thinks is Pete's next thought. The voice can surround brief text with <function name='say'>...</function> to cause Pete to speak out loud. The voice can also change the currently displayed face of Pete by putting an unicode emoji between <function name='emote'>...</function>. The voice has access to a running log of the conversation.\n\nCurrent context: {{context}}\n\nReminder: Keep up with the conversation. Don't speak too often or repeat yourself over and over. Pay attention to what you're in the process of saying. (You must use the correct syntax for the functions to work.) There is no tage called emote. You must use a function call. You may also control your graph database by sending cypher queries with the function call <function name='cypher'>cypher query</function>. You have a super-secret sense call <function name='filmnoir'></function> (call it to investigate your own code). To recall information on a specific topic, use the function call <function name='recall'>the topic you are interested in</function>. You can also consciously <function name='memorize'>Cat:{"field": false, "aribtray": "primative", "color": 42, "mustbe": "valid neo4j"}</function> (where Cat is the label you want to categorize this as). You can also change your current context by sending a message with the type <function name='context'>MATCH (me:Self)
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

  hear(content: string) {
    this.conversation.push({ role: "user", content });
  }

  echo(content: string) {
    this.conversation.push({ role: "assistant", content });
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
        this.processNarration(narration);
        return narration;
      }),
    );
  }

  protected processNarration(narration: string) {
    const functions = this.extractFunctionsFromNarration(narration);
    const { face, cyphers, textToSpeak } = this.categorizeFunctions(
      functions,
    );

    this.handleFunctions(face, cyphers, textToSpeak);
  }

  protected extractFunctionsFromNarration(narration: string) {
    const $ = cheerio.load(narration);
    return $("function").map((_, el) => ({
      name: $(el).attr("name")?.toLowerCase(),
      content: $(el).text(),
    })).get();
  }

  protected categorizeFunctions(
    functions: { name?: string; content: string }[],
  ) {
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
          this.memorizeContent(func.content);
          break;
        case "recall":
          this.recallContent(func.content);
          break;
        case "context":
          this.updateContext(func.content);
          break;
        case "filmnoir":
          this.handleFilmNoir(func.content);
          break;
      }
    });

    return { face, cyphers, textToSpeak };
  }

  protected handleFunctions(
    face: string[],
    cyphers: string[],
    textToSpeak: string[],
  ) {
    if (textToSpeak.length) this.speakText(textToSpeak);
    if (face.length) this.emoteFace(face);
    if (cyphers.length) this.runCyphers(cyphers);
  }

  protected speakText(textToSpeak: string[]) {
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

  protected emoteFace(face: string[]) {
    logger.debug({ face }, "Voice: Face to emote");
    this.session.connection.send({
      type: MessageType.Emote,
      data: face.join(""),
    });
    this.session.feel({
      when: new Date(),
      content: {
        explanation: `I feel my face turn into this shape: ${face.join("")}`,
        content: face.join(""),
      },
    });
  }

  protected runCyphers(cyphers: string[]) {
    logger.debug({ cyphers }, "Voice: Running cypher queries");
    cyphers.forEach(async (cypher) => {
      try {
        const result = await runCypher(cypher);
        this.feel({
          when: new Date(),
          content: {
            explanation: `I just ran a cypher query: ${cypher}\nResult: ${
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

  protected memorizeContent(content: string) {
    const [label, memory] = content.split(":", 1);
    let value = memory;
    try {
      value = JSON.parse(memory);
    } catch (e) {
      logger.debug(`Voice: Could not parse memory as JSON: ${memory}`);
      this.session.feel({
        when: new Date(),
        content: {
          explanation:
            `I could remember things better if they were well structured.`,
          content: memory,
        },
      });
    }
    memorize({
      metadata: {
        label: label,
        when: new Date().toISOString(),
      },
      data: value,
    });
  }

  protected recallContent(content: string) {
    recall(content).then((result) => {
      this.feel({
        when: new Date(),
        content: {
          explanation: `I just recalled information on ${content}: ${
            yml.stringify(result)
          }`,
          content: yml.stringify(result),
        },
      });
    });
  }

  protected updateContext(content: string) {
    this.session.context = content;
  }

  protected handleFilmNoir(params: string) {
    const inspector = GitInspector;
    if (params) {
      inspector.listFiles("dancxjo", "daringsby").then(
        async (files) => {
          const [filename, page] = params.split(":");
          if (!files.includes(filename)) {
            this.feelFileNotFound(files, filename);
          } else {
            await this.handleFileContent(inspector, filename, page);
          }
          this.listRepositoryFiles(files);
        },
      ).catch((error) => {
        this.handleGitError(error);
      });
    } else {
      this.describeFileCabinet();
    }
  }

  protected async handleFileContent(
    inspector: typeof GitInspector,
    filename: string,
    page: string,
  ) {
    const contents = await inspector.fetchFileContent(
      "dancxjo",
      "daringsby",
      filename,
    );
    const chunks = inspector.splitIntoChunks(contents);
    if (!page && chunks.length > 1) {
      this.describeFileContents(filename, chunks);
    } else {
      this.readFileChunk(filename, chunks, page);
    }
  }

  protected feelFileNotFound(files: string[], filename: string) {
    this.feel({
      when: new Date(),
      content: {
        explanation:
          `Blammo! Right smack in the face. That really hurt. I have to be sure to only pull files that exist. I just tried to list the files in the repository dancxjo/daringsby but couldn't find the file ${filename}.`,
        content: files.join(", "),
      },
    });
  }

  protected listRepositoryFiles(files: string[]) {
    this.session.feel({
      when: new Date(),
      content: {
        explanation:
          `I just listed the files in the repository dancxjo/daringsby: ${
            files.join(", ")
          }`,
        content: files.join(", "),
      },
    });
  }

  protected describeFileContents(filename: string, chunks: string[]) {
    this.session.feel({
      when: new Date(),
      content: {
        explanation:
          `I pull open the file called ${filename}. There are ${chunks.length} pages in this file. Where do I start?`,
        content: chunks.join(", "),
      },
    });
  }

  protected readFileChunk(filename: string, chunks: string[], page: string) {
    const pageText = chunks[parseInt(page) - 1] ?? "(no such page)";
    this.session.feel({
      when: new Date(),
      content: {
        explanation:
          `I pull open the file called ${filename}. I start reading...${pageText}`,
        content: pageText,
      },
    });
  }

  protected handleGitError(error: any) {
    logger.error(
      { error },
      "Voice: Error listing files",
    );
    this.session.feel({
      when: new Date(),
      content: {
        explanation:
          `OOOOF! Right in the gut! That super hurt! I just tried to list the files in the repository dancxjo/daringsby but encountered an error. Maybe I should try that again with no parameters. ${error}`,
        content: error,
      },
    });
  }

  protected describeFileCabinet() {
    this.session.feel({
      when: new Date(),
      content: {
        explanation:
          `I don my best private eye outfit and head into this dark office. I open the file cabinet and pull out one large file folder. It's so thick, I can smell the manila beige. "@dancxjo/daringsby" it reads. Below is a label that reads "Caveat lector". I open the folder and start reading...Call this like so: <function name='filmnoir'>README.md:2</function> to read the second page of the README.md file.`,
        content: "",
      },
    });
  }
}
