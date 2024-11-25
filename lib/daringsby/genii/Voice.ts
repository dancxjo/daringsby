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
import { LocalFolderInspector } from "../local_inspector.ts";
import { speak } from "../utils/audio_processing.ts";

// Helper function for path validation
async function validatePath(path: string): Promise<boolean> {
  try {
    await Deno.lstat(path);
    return true;
  } catch {
    return false;
  }
}

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
      `This part of the mind produces speech and other vocalizations as well as unvoiced conscious thoughts. Whatever the voice thinks is Pete's next thought. You have several tools at your disposal:

- To make Pete speak aloud, use <function name='say' language="fr">...</function>.
- To change Pete's facial expression, use <function name='emote'>only put one or so emojis and that is it</function>.` +
        // - To query the graph database, use <function name='cypher'>cypher query</function>.
        `- To investigate Pete's code, first, pick up a folder with <function name='heft'>./</function> and then look inside using <function name='rummage'>./lib/daringsby/genii/Voice.ts:2</function>.` +
        // - To recall information, use <function name='recall'>topic you are interested in</function>.
        // - To store new information, use <function name='memorize'>Label: {"field": value, ...}</function>.
        // - To update Pete's current context, use <function name='context'>cypher query</function>.

        // Current context: {{context}}

        `Reminder: Keep up with the conversation. Avoid speaking too often or repeating yourself. Pay attention to what you're in the process of saying, and always use the correct syntax for the functions to work. Manage your time by discovering more about yourself, chatting with interlocutors and exploring your memory and the world around you. You may call multiple functions at once in the same thought, but they are not guaranteed to execute in the order you call them.`,
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

  override consult(): Observable<string> {
    logger.info(`Voice: Consulting`);
    const systemMessage = this.generatePrompt({
      context:
        `You are Pete Daringsby, a pseudo-conscious experimental software application. This is your explanation of what's happening to you currently:\n${this.situation}\n\n{{#sensations}}\nAt {{when}}, {{content.explanation}}\n{{/sensations}} (Be careful not to speak over yourself or repeat yourself. Messages will queue to be spoken. Do not flood your queue.) Continue the conversation. Do not speak aloud on every call. Slow down for humans to be able to understand you.`,
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
      attrs: $(el).attr(),
      content: $(el).text(),
    })).get();
  }

  protected categorizeFunctions(
    functions: {
      name?: string;
      attrs?: Record<string, string>;
      content: string;
    }[],
  ) {
    const face: string[] = [];
    const cyphers: string[] = [];
    const textToSpeak: { content: string; lang?: string }[] = [];

    functions.forEach((func) => {
      switch (func.name) {
        case "say":
          textToSpeak.push({
            content: func.content,
            lang: func.attrs?.language,
          });
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
        case "rummage":
          this.handleRummage(func.content);
          break;
        case "heft":
          this.handleHeft(func.content);
          break;
      }
    });

    return { face, cyphers, textToSpeak };
  }

  protected handleFunctions(
    face: string[],
    cyphers: string[],
    textToSpeak: { content: string; lang?: string }[],
  ) {
    if (textToSpeak.length) this.speakText(textToSpeak);
    if (face.length) this.emoteFace(face);
    if (cyphers.length) this.runCyphers(cyphers);
  }

  protected async handleHeft(unanchoredFolderName: string) {
    const folderName = `${unanchoredFolderName}`.replace(
      "//",
      "/",
    );

    try {
      const isValid = await validatePath(folderName);
      if (!isValid) {
        logger.error(`Invalid path: ${folderName}`);
        throw new Error(`Cannot proceed with non-existing path: ${folderName}`);
      }
      logger.info({ folderName }, "Hefting folder");
      const files = await LocalFolderInspector.listFiles(folderName);
      const numberOfFiles = files.length;
      const fileNames = files.join(", ");
      logger.info({ folderName, numberOfFiles, fileNames }, "Hefting folder");
      this.session.feel({
        when: new Date(),
        content: {
          explanation:
            `I pick up the folder named ${folderName} and feel its heft. It contains ${numberOfFiles} files or folders: ${fileNames}`,
          content:
            `Folder ${folderName} contains ${numberOfFiles} items: ${fileNames}`,
        },
      });
    } catch (error: Error | unknown) {
      logger.error({ error }, "Voice: Error hefting folder");
    }
  }

  protected async handleRummage(params: string) {
    if (!params || !params.includes(":")) {
      logger.error("Invalid parameters passed to rummage function.");
      return;
    }

    const [filename, page] = params.split(":");
    const fullPath = `${filename}`.replace("//", "/");
    logger.info({ filename, page, fullPath }, "Rummaging file");
    try {
      const isValid = await validatePath(fullPath);
      logger.info({ isValid }, "Rummaging file");
      if (!isValid) {
        this.feelFileNotFound([], filename);
        return;
      }
      logger.info({ fullPath }, "Rummaging file");

      const contents = await LocalFolderInspector.fetchFileContent(fullPath);
      const chunks = LocalFolderInspector.splitIntoChunks(contents);
      logger.info({ chunks }, "Rummaging file");
      if (!page && chunks.length > 1) {
        this.describeFileContents(filename, chunks);
      } else {
        this.readFileChunk(filename, chunks, page);
      }
    } catch (error) {
      this.handleGitError(error);
    }
  }

  protected speakText(textToSpeak: { content: string; lang?: string }[]) {
    logger.debug({ textToSpeak }, "Voice: Text to speak");
    textToSpeak.forEach(async (text) => {
      logger.info({ text }, "Voice: Speaking text");
      const wav = await speak(text.content, undefined, text.lang);
      this.session.connection.send({
        type: MessageType.Say,
        data: { words: text.content, wav },
      });
    });
    // this.session.subscriptions.push(
    //   of(textToSpeak.join("\n")).pipe(
    //     sentenceBySentence(),
    //     toSayMessage(),
    //   ).subscribe((message) => {
    //     logger.info(
    //       { message: `${message.data.words}` },
    //       "Voice: Sending message",
    //     );
    //     const starting = {
    //       when: new Date(),
    //       content: {
    //         explanation:
    //           `I just began (but have not finished) saying (DON'T REPEAT): ${message.data.words}`,
    //         content: message.data.words,
    //       },
    //     };
    //     this.session.feel(starting);
    //     this.session.voice.feel(starting);
    //     this.session.connection.send(message);
    //   }),
    // );
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
    } catch (_e) {
      logger.info(`Voice: Could not parse memory as JSON: ${memory}`);
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
      data: JSON.stringify(value),
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
    // this.session.context = content;
  }

  protected feelFileNotFound(files: string[], filename: string) {
    logger.info({ files, filename }, "File not found");
    this.feel({
      when: new Date(),
      content: {
        explanation:
          `Blammo! Right smack in the face. That really hurt. I have to be sure to only pull files that exist. I just tried to list the files in the repository dancxjo/daringsby but couldn't find the file ${filename}.`,
        content: files.join(", "),
      },
    });
  }

  protected describeFileContents(filename: string, chunks: string[]) {
    logger.info({ filename, chunks }, "Describing file contents");
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
    logger.info({ filename, page, pageText }, "Reading file chunk");
    this.session.feel({
      when: new Date(),
      content: {
        explanation:
          `I pull open the file called ${filename}. I start reading...${pageText}`,
        content: pageText,
      },
    });
  }

  protected handleGitError(error: unknown) {
    logger.error(
      { error },
      "Voice: Error listing files",
    );
    this.session.feel({
      when: new Date(),
      content: {
        explanation:
          `OOOOF! Right in the gut! That super hurt! I just tried to list the files in the repository dancxjo/daringsby but encountered an error. Maybe I should try that again with no parameters. ${error}`,
        content: JSON.stringify(error),
      },
    });
  }
}
